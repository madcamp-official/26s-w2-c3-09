import 'dart:async';
import 'package:drift/drift.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:socket_io_client/socket_io_client.dart' as io;
import '../../storage/app_database.dart';
import '../../storage/display_cache.dart';
import '../../features/auth/auth_controller.dart';
import '../config/app_config.dart';
import '../models/character_state.dart';
import '../network/api_client.dart';
import 'connection_lifecycle.dart';
import 'mutation_queue.dart';
import 'realtime_account_session.dart';

final realtimeOwnerUidProvider = Provider<String?>((ref) {
  return ref.watch(authControllerProvider).asData?.value?.uid;
});

typedef RealtimeEventFetcher =
    Future<List<Map<String, dynamic>>> Function(String path);

final realtimeEventFetcherProvider = Provider<RealtimeEventFetcher>(
  (ref) => ref.watch(apiClientProvider).getList,
);

final realtimeAutoConnectProvider = Provider<bool>((_) => true);

final realtimeRevisionProvider =
    NotifierProvider.autoDispose<RealtimeController, int>(
      RealtimeController.new,
    );

final realtimeCharacterKindProvider =
    NotifierProvider<RealtimeCharacterKindController, CharacterState?>(
      RealtimeCharacterKindController.new,
    );

enum RealtimeHomeUpdateKind {
  presence,
  deviceRemoved,
  roomRemoved,
  proposalCreated,
  commandStatus,
  executionStatus,
  refreshSummary,
}

class RealtimeHomeUpdate {
  const RealtimeHomeUpdate({
    required this.kind,
    required this.eventType,
    this.deviceId,
    this.roomId,
    this.proposalId,
    this.commandId,
    this.executionId,
    this.proposalStatus,
    this.proposalSummary,
    this.proposalItemCount,
    this.pendingProposalCount,
    this.presence,
    this.commandStatus,
    this.executionStatus,
  });

  final RealtimeHomeUpdateKind kind;
  final String eventType;
  final String? deviceId;
  final String? roomId;
  final String? proposalId;
  final String? commandId;
  final String? executionId;
  final String? proposalStatus;
  final Map<String, dynamic>? proposalSummary;
  final int? proposalItemCount;
  final int? pendingProposalCount;
  final String? presence;
  final String? commandStatus;
  final String? executionStatus;
}

final realtimeHomeUpdateProvider =
    NotifierProvider<RealtimeHomeUpdateController, RealtimeHomeUpdate?>(
      RealtimeHomeUpdateController.new,
    );

class RealtimeHomeUpdateController extends Notifier<RealtimeHomeUpdate?> {
  @override
  RealtimeHomeUpdate? build() {
    ref.watch(realtimeOwnerUidProvider);
    return null;
  }

  void emit(RealtimeHomeUpdate update) => state = update;
}

RealtimeHomeUpdate? realtimeHomeUpdateFor(String event, Object? data) {
  if (data is! Map) return null;
  final value = Map<String, dynamic>.from(data);
  final nestedPayload = value['payload'];
  final payload = nestedPayload is Map
      ? Map<String, dynamic>.from(nestedPayload)
      : value;
  final deviceId = switch (payload['deviceId'] ?? value['deviceId']) {
    final String id when id.isNotEmpty => id,
    _ => null,
  };
  final roomId = switch (payload['roomId'] ?? value['roomId']) {
    final String id when id.isNotEmpty => id,
    _ => null,
  };
  final commandId = switch (payload['commandId'] ??
      value['commandId'] ??
      value['aggregateId']) {
    final String id when id.isNotEmpty => id,
    _ => null,
  };
  final proposalId = switch (payload['proposalId'] ??
      value['proposalId'] ??
      value['aggregateId']) {
    final String id when id.isNotEmpty => id,
    _ => null,
  };
  final executionId = switch (payload['executionId'] ??
      value['executionId'] ??
      value['aggregateId']) {
    final String id when id.isNotEmpty => id,
    _ => null,
  };

  if (event == 'presence.updated') {
    final presence = payload['presence'];
    if (deviceId == null ||
        presence is! String ||
        !_validPresences.contains(presence)) {
      return null;
    }
    return RealtimeHomeUpdate(
      kind: RealtimeHomeUpdateKind.presence,
      eventType: event,
      deviceId: deviceId,
      presence: presence,
    );
  }
  if (event == 'device.revoked' && deviceId != null) {
    return RealtimeHomeUpdate(
      kind: RealtimeHomeUpdateKind.deviceRemoved,
      eventType: event,
      deviceId: deviceId,
    );
  }
  if (event == 'room.removed' && roomId != null) {
    return RealtimeHomeUpdate(
      kind: RealtimeHomeUpdateKind.roomRemoved,
      eventType: event,
      roomId: roomId,
    );
  }
  if (event == 'proposal.created') {
    final status = payload['status'];
    final summary = payload['summary'];
    final itemCount = payload['itemCount'];
    final pendingProposalCount = payload['pendingProposalCount'];
    if (roomId != null &&
        proposalId != null &&
        status is String &&
        status.isNotEmpty &&
        pendingProposalCount is int) {
      return RealtimeHomeUpdate(
        kind: RealtimeHomeUpdateKind.proposalCreated,
        eventType: event,
        roomId: roomId,
        proposalId: proposalId,
        commandId: commandId,
        proposalStatus: status,
        proposalSummary: summary is Map
            ? Map<String, dynamic>.from(summary)
            : null,
        proposalItemCount: itemCount is int ? itemCount : null,
        pendingProposalCount: pendingProposalCount,
      );
    }
    return RealtimeHomeUpdate(
      kind: RealtimeHomeUpdateKind.refreshSummary,
      eventType: event,
    );
  }
  if (event == 'command.updated') {
    final status = payload['status'];
    if (roomId != null &&
        commandId != null &&
        status is String &&
        status.isNotEmpty) {
      return RealtimeHomeUpdate(
        kind: RealtimeHomeUpdateKind.commandStatus,
        eventType: event,
        roomId: roomId,
        commandId: commandId,
        commandStatus: status,
      );
    }
    return null;
  }
  if (event == 'execution.updated') {
    final status = payload['status'];
    if (roomId != null && status is String && status.isNotEmpty) {
      return RealtimeHomeUpdate(
        kind: RealtimeHomeUpdateKind.executionStatus,
        eventType: event,
        roomId: roomId,
        executionId: executionId,
        executionStatus: status,
      );
    }
    return RealtimeHomeUpdate(
      kind: RealtimeHomeUpdateKind.refreshSummary,
      eventType: event,
    );
  }
  if (_summaryRefreshEvents.contains(event)) {
    return RealtimeHomeUpdate(
      kind: RealtimeHomeUpdateKind.refreshSummary,
      eventType: event,
    );
  }
  if (_homeIrrelevantEvents.contains(event)) return null;

  // Unknown validated domain envelopes may gain a home projection later.
  // One summary read is safer than silently displaying stale aggregate data.
  if (value['eventId'] is String || value['eventType'] is String) {
    return RealtimeHomeUpdate(
      kind: RealtimeHomeUpdateKind.refreshSummary,
      eventType: event,
    );
  }
  return null;
}

const _validPresences = <String>{
  'OFFLINE',
  'ONLINE_IDLE',
  'ONLINE_SCANNING',
  'ONLINE_EXECUTING',
  'DEGRADED',
};

const _summaryRefreshEvents = <String>{
  'device.paired',
  'decision.created',
  'room.snapshot.updated',
};

const _homeIrrelevantEvents = <String>{
  'character.event',
  'chat.message.created',
  'command.available',
  'command.updated',
  'file.browse.failed',
  'file.transfer.requested',
  'file.transfer.updated',
  'rule.created',
  'rule.updated',
  'smart-cache.updated',
};

class RealtimeCharacterKindController extends Notifier<CharacterState?> {
  @override
  CharacterState? build() {
    ref.watch(realtimeOwnerUidProvider);
    return null;
  }

  void emit(CharacterState kind) => state = kind;
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
  RealtimeNotice? build() {
    ref.watch(realtimeOwnerUidProvider);
    return null;
  }

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
    'device.revoked' => 'PC 연결이 해제되었습니다.',
    'room.removed' => '관리 폴더 연결이 해제되었습니다.',
    _ => null,
  };
  return message == null
      ? null
      : RealtimeNotice(eventId: eventId, eventType: event, message: message);
}

CharacterState? realtimeCharacterKindFor(String event, Object? data) {
  if (data is! Map) return null;
  final value = Map<String, dynamic>.from(data);
  if (event == 'character.event') {
    return parseCharacterState(value['kind']);
  }
  if (event != 'presence.updated') return null;
  final nestedPayload = value['payload'];
  final payload = nestedPayload is Map
      ? Map<String, dynamic>.from(nestedPayload)
      : value;
  return switch (payload['presence']) {
    'OFFLINE' => CharacterState.offline,
    'ONLINE_IDLE' => CharacterState.idle,
    'ONLINE_SCANNING' => CharacterState.analyzing,
    'ONLINE_EXECUTING' => CharacterState.working,
    'DEGRADED' => CharacterState.error,
    _ => null,
  };
}

class RealtimeController extends Notifier<int> {
  io.Socket? _socket;
  Future<void>? _connecting;
  final Set<int> _replayingGenerations = <int>{};
  final RealtimeAccountSessionGuard _account = RealtimeAccountSessionGuard();
  bool _disposed = false;

  @override
  int build() {
    _disposed = false;
    final ownerUid = ref.watch(realtimeOwnerUidProvider);
    if (_account.bind(ownerUid)) {
      _disposeTransport();
    }
    ref.onDispose(() {
      _disposed = true;
      _account.invalidate();
      _disposeTransport();
    });
    if (ownerUid != null && ref.watch(realtimeAutoConnectProvider)) {
      unawaited(connect());
    }
    return 0;
  }

  Future<void> connect() async {
    if (_socket != null || _connecting != null || _disposed) return;
    final ownerUid = _account.ownerUid;
    if (ownerUid == null || ref.read(realtimeOwnerUidProvider) != ownerUid) {
      return;
    }
    final session = _account.beginConnection();
    if (session == null) return;
    late final Future<void> attempt;
    attempt = _connect(session).whenComplete(() {
      if (identical(_connecting, attempt)) _connecting = null;
    });
    _connecting = attempt;
    await attempt;
  }

  void disconnect() {
    _account.invalidate();
    _disposeTransport();
  }

  void _disposeTransport() {
    _socket?.dispose();
    _socket = null;
    _connecting = null;
  }

  Future<void> _connect(RealtimeAccountSession session) async {
    final user = ref.read(firebaseAuthProvider).currentUser;
    if (user?.uid != session.ownerUid) return;
    final token = await user?.getIdToken();
    if (token == null || !_isCurrent(session)) return;
    final socket = io.io(
      '${AppConfig.apiBaseUrl}/realtime',
      io.OptionBuilder()
          .setTransports(['websocket'])
          .disableAutoConnect()
          .setAuth({'token': 'Bearer $token'})
          .enableReconnection()
          .build(),
    );
    if (!_isCurrent(session)) {
      socket.dispose();
      return;
    }
    _socket = socket;
    socket.onConnect((_) async {
      if (!_isActiveSocket(session, socket)) return;
      await _replay(session);
      if (!_isActiveSocket(session, socket)) return;
      // Lifecycle tombstones must purge removed-room mutations before any
      // remaining outbox request is allowed to leave the device.
      await ref.read(mutationQueueProvider).flush();
    });
    socket.onAny((event, data) async {
      await _handleSocketEvent(session, socket, event, data);
    });
    if (!_isCurrent(session)) {
      socket.dispose();
      if (identical(_socket, socket)) _socket = null;
      return;
    }
    socket.connect();
  }

  Future<void> _handleSocketEvent(
    RealtimeAccountSession session,
    io.Socket socket,
    String event,
    Object? data,
  ) async {
    if (!_isActiveSocket(session, socket)) return;
    var shouldRefresh = true;
    await _applyConnectionLifecycle(session, event, data);
    if (!_isActiveSocket(session, socket)) return;
    if (data is Map) {
      final sequence = data['sequence'];
      if (sequence is int && sequence >= 0) {
        shouldRefresh = await _advanceCursor(session, sequence);
      }
    }
    if (!_isActiveSocket(session, socket)) return;
    final notice = realtimeNoticeFor(event, data);
    if (notice != null && _isActiveSocket(session, socket)) {
      ref.read(realtimeNoticeProvider.notifier).emit(notice);
    }
    final characterKind = realtimeCharacterKindFor(event, data);
    if (characterKind != null && _isActiveSocket(session, socket)) {
      ref.read(realtimeCharacterKindProvider.notifier).emit(characterKind);
    }
    final homeUpdate = realtimeHomeUpdateFor(event, data);
    if (shouldRefresh &&
        homeUpdate != null &&
        _isActiveSocket(session, socket)) {
      ref.read(realtimeHomeUpdateProvider.notifier).emit(homeUpdate);
    }
    // Presence has a complete, device-scoped payload and is patched directly
    // by HomeController. It must not fan out through generic invalidation.
    if (shouldRefresh &&
        event != 'presence.updated' &&
        _isActiveSocket(session, socket)) {
      state++;
    }
  }

  Future<void> _replay(RealtimeAccountSession session) async {
    if (!_isCurrent(session) ||
        !_replayingGenerations.add(session.generation)) {
      return;
    }
    try {
      final database = ref.read(appDatabaseProvider);
      final current =
          await (database.select(database.syncCursors)..where(
                (row) =>
                    row.ownerUid.equals(session.ownerUid) &
                    row.stream.equals('user'),
              ))
              .getSingleOrNull();
      if (!_isCurrent(session)) return;
      var cursor = current?.lastSequence ?? 0;
      while (true) {
        final events = await ref.read(realtimeEventFetcherProvider)(
          '/v1/sync/events?after=$cursor&limit=200',
        );
        if (!_isCurrent(session)) return;
        if (events.isEmpty) break;
        for (final event in events) {
          if (!_isCurrent(session)) return;
          final eventType = event['eventType'];
          if (eventType is String) {
            await _applyConnectionLifecycle(session, eventType, event);
            if (!_isCurrent(session)) return;
            final homeUpdate = realtimeHomeUpdateFor(eventType, event);
            if (homeUpdate != null) {
              ref.read(realtimeHomeUpdateProvider.notifier).emit(homeUpdate);
            }
          }
          final sequence = event['sequence'];
          if (sequence is int && sequence > cursor) cursor = sequence;
        }
        await _advanceCursor(session, cursor);
        if (!_isCurrent(session)) return;
        state++;
        if (events.length < 200) break;
      }
    } finally {
      _replayingGenerations.remove(session.generation);
    }
  }

  Future<bool> _advanceCursor(
    RealtimeAccountSession session,
    int sequence,
  ) async {
    if (!_isCurrent(session)) return false;
    final database = ref.read(appDatabaseProvider);
    final advanced = await database.transaction(() async {
      if (!_isCurrent(session)) return false;
      final current =
          await (database.select(database.syncCursors)..where(
                (row) =>
                    row.ownerUid.equals(session.ownerUid) &
                    row.stream.equals('user'),
              ))
              .getSingleOrNull();
      if (!_isCurrent(session)) return false;
      if (current != null && current.lastSequence >= sequence) return false;
      await database
          .into(database.syncCursors)
          .insertOnConflictUpdate(
            SyncCursorsCompanion.insert(
              ownerUid: session.ownerUid,
              stream: 'user',
              lastSequence: Value(sequence),
            ),
          );
      return true;
    });
    return advanced && _isCurrent(session);
  }

  Future<void> _applyConnectionLifecycle(
    RealtimeAccountSession session,
    String event,
    Object? data,
  ) async {
    if (!_isCurrent(session)) return;
    final lifecycle = connectionLifecycleEventFor(event, data);
    if (lifecycle == null) return;
    final cache = DisplayCache(ref.read(appDatabaseProvider), session.ownerUid);
    if (lifecycle.eventType == 'device.revoked' && lifecycle.deviceId != null) {
      await cache.removeDeviceCascade(lifecycle.deviceId!);
      if (!_isCurrent(session)) return;
    } else if (lifecycle.eventType == 'room.removed' &&
        lifecycle.roomId != null) {
      await cache.removeRoomCascade(lifecycle.roomId!);
      if (!_isCurrent(session)) return;
    }
    if (_isCurrent(session)) {
      ref.read(connectionLifecycleEventProvider.notifier).emit(lifecycle);
    }
  }

  bool _isCurrent(RealtimeAccountSession session) =>
      !_disposed &&
      _account.isCurrent(session, ref.read(realtimeOwnerUidProvider));

  bool _isActiveSocket(RealtimeAccountSession session, io.Socket socket) =>
      identical(_socket, socket) && _isCurrent(session);

  /// Test hook for a delayed replay without opening a real socket.
  Future<void> replayCurrentAccountForTesting() async {
    final session = _account.current;
    if (session != null) await _replay(session);
  }
}
