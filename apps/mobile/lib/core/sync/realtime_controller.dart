import 'dart:async';
import 'package:drift/drift.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:socket_io_client/socket_io_client.dart' as io;
import '../../storage/app_database.dart';
import '../../features/auth/auth_controller.dart';
import '../config/app_config.dart';
import '../network/api_client.dart';
import 'mutation_queue.dart';

final realtimeRevisionProvider = NotifierProvider<RealtimeController, int>(
  RealtimeController.new,
);

final realtimeCharacterKindProvider =
    NotifierProvider<RealtimeCharacterKindController, String?>(
      RealtimeCharacterKindController.new,
    );

class RealtimeCharacterKindController extends Notifier<String?> {
  @override
  String? build() => null;

  void emit(String kind) => state = kind;
}

class RealtimeNotice {
  const RealtimeNotice({
    required this.eventId,
    required this.eventType,
    required this.message,
  });
  final String eventId;
  final String eventType;
  final String message;
}

final realtimeNoticeProvider =
    NotifierProvider<RealtimeNoticeController, RealtimeNotice?>(
      RealtimeNoticeController.new,
    );

class RealtimeNoticeController extends Notifier<RealtimeNotice?> {
  @override
  RealtimeNotice? build() => null;

  void emit(RealtimeNotice notice) {
    if (state?.eventId == notice.eventId) return;
    state = notice;
  }
}

RealtimeNotice? realtimeNoticeFor(String event, Object? data) {
  if (data is! Map) return null;
  final envelope = Map<String, dynamic>.from(data);
  final eventId = envelope['eventId'] as String?;
  if (eventId == null) return null;
  final payload = Map<String, dynamic>.from(
    envelope['payload'] as Map? ?? const {},
  );
  final message = switch (event) {
    'proposal.created' => '새 정리 제안이 도착했습니다. 승인 전에 내용을 확인하세요.',
    'execution.updated' => switch (payload['status']) {
      'SUCCEEDED' => '승인한 정리 작업이 완료됐습니다.',
      'PARTIALLY_SUCCEEDED' => '정리 작업 일부가 완료되지 않았습니다. 결과를 확인하세요.',
      'FAILED' => '정리 작업이 실패했습니다. 파일은 임의로 성공 처리되지 않았습니다.',
      'STALE' => '승인 뒤 파일이 변경되어 작업을 중단했습니다.',
      'ROLLED_BACK' => '정리 작업이 되돌려졌습니다.',
      _ => null,
    },
    'file.transfer.updated' when payload['status'] == 'READY' =>
      '요청한 파일이 다운로드 준비됐습니다.',
    _ => null,
  };
  return message == null
      ? null
      : RealtimeNotice(eventId: eventId, eventType: event, message: message);
}

String? realtimeCharacterKindFor(String event, Object? data) {
  if (data is! Map) return null;
  final value = Map<String, dynamic>.from(data);
  if (event == 'character.event') {
    return _validatedCharacterKind(value['kind']);
  }
  if (event != 'presence.updated') return null;
  final payload = Map<String, dynamic>.from(
    value['payload'] as Map? ?? const {},
  );
  return switch (payload['presence']) {
    'OFFLINE' => 'OFFLINE',
    'ONLINE_IDLE' => 'IDLE',
    'ONLINE_SCANNING' => 'ANALYZING',
    'ONLINE_EXECUTING' => 'WORKING',
    'DEGRADED' => 'ERROR',
    _ => null,
  };
}

String? _validatedCharacterKind(Object? value) =>
    value is String &&
        const {
          'IDLE',
          'ANALYZING',
          'WAITING_APPROVAL',
          'WORKING',
          'SUCCESS',
          'ERROR',
          'USER_WORKING',
          'OFFLINE',
        }.contains(value)
    ? value
    : null;

class RealtimeController extends Notifier<int> {
  io.Socket? _socket;
  bool _replaying = false;

  @override
  int build() {
    ref.onDispose(() => _socket?.dispose());
    unawaited(connect());
    return 0;
  }

  Future<void> connect() async {
    if (_socket != null) return;
    await _connect();
  }

  void disconnect() {
    _socket?.dispose();
    _socket = null;
  }

  Future<void> _connect() async {
    final user = ref.read(firebaseAuthProvider).currentUser;
    final token = await user?.getIdToken();
    if (token == null) return;
    final socket = io.io(
      '${AppConfig.apiBaseUrl}/realtime',
      io.OptionBuilder()
          .setTransports(['websocket'])
          .disableAutoConnect()
          .setAuth({'token': 'Bearer $token'})
          .enableReconnection()
          .build(),
    );
    _socket = socket;
    socket.onConnect((_) async {
      await ref.read(mutationQueueProvider).flush();
      await _replay();
    });
    socket.onAny((event, data) async {
      var shouldRefresh = true;
      if (data is Map) {
        final payload = Map<String, dynamic>.from(data);
        final sequence = payload['sequence'];
        if (sequence is int) shouldRefresh = await _advanceCursor(sequence);
      }
      final notice = realtimeNoticeFor(event, data);
      if (notice != null) {
        ref.read(realtimeNoticeProvider.notifier).emit(notice);
      }
      final characterKind = realtimeCharacterKindFor(event, data);
      if (characterKind != null) {
        ref.read(realtimeCharacterKindProvider.notifier).emit(characterKind);
      }
      if (shouldRefresh) state++;
    });
    socket.connect();
  }

  Future<void> _replay() async {
    if (_replaying) return;
    _replaying = true;
    try {
      final database = ref.read(appDatabaseProvider);
      final ownerUid = _ownerUid;
      final current =
          await (database.select(database.syncCursors)..where(
                (row) =>
                    row.ownerUid.equals(ownerUid) & row.stream.equals('user'),
              ))
              .getSingleOrNull();
      var cursor = current?.lastSequence ?? 0;
      while (true) {
        final events = await ref
            .read(apiClientProvider)
            .getList('/v1/sync/events?after=$cursor&limit=200');
        if (events.isEmpty) break;
        for (final event in events) {
          final sequence = event['sequence'];
          if (sequence is int && sequence > cursor) cursor = sequence;
        }
        await _advanceCursor(cursor);
        state++;
        if (events.length < 200) break;
      }
    } finally {
      _replaying = false;
    }
  }

  Future<bool> _advanceCursor(int sequence) async {
    final database = ref.read(appDatabaseProvider);
    final ownerUid = _ownerUid;
    return database.transaction(() async {
      final current =
          await (database.select(database.syncCursors)..where(
                (row) =>
                    row.ownerUid.equals(ownerUid) & row.stream.equals('user'),
              ))
              .getSingleOrNull();
      if (current != null && current.lastSequence >= sequence) return false;
      await database
          .into(database.syncCursors)
          .insertOnConflictUpdate(
            SyncCursorsCompanion.insert(
              ownerUid: ownerUid,
              stream: 'user',
              lastSequence: Value(sequence),
            ),
          );
      return true;
    });
  }

  String get _ownerUid {
    final uid = ref.read(firebaseAuthProvider).currentUser?.uid;
    if (uid == null) throw StateError('UNAUTHENTICATED');
    return uid;
  }
}
