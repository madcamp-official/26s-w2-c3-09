import 'dart:convert';
import 'package:dio/dio.dart';
import 'package:drift/drift.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:uuid/uuid.dart';
import '../../features/auth/auth_controller.dart';
import '../../storage/app_database.dart';
import '../network/api_client.dart';

final mutationQueueProvider = Provider<MutationQueue>((ref) {
  final auth = ref.watch(firebaseAuthProvider);
  return MutationQueue(
    ref.watch(appDatabaseProvider),
    ref.watch(apiClientProvider).post,
    () => auth.currentUser?.uid,
  );
});

typedef MutationPost =
    Future<Map<String, dynamic>> Function(
      String path,
      Map<String, dynamic> body, {
      String? idempotencyKey,
      String? expectedOwnerUid,
    });

class MutationResult {
  const MutationResult({required this.queued, this.response});
  final bool queued;
  final Map<String, dynamic>? response;
}

class MutationQueueSummary {
  const MutationQueueSummary({required this.pending, required this.failed});
  final int pending;
  final int failed;
}

class MutationQueue {
  MutationQueue(this._database, this._post, this._readOwnerUid);
  final AppDatabase _database;
  final MutationPost _post;
  final String? Function() _readOwnerUid;
  final Set<String> _flushingOwnerUids = <String>{};

  String get _ownerUid {
    final uid = _readOwnerUid();
    if (uid == null) throw StateError('UNAUTHENTICATED');
    return uid;
  }

  Future<MutationResult> postOrQueue({
    required String mutationType,
    required String path,
    required Map<String, dynamic> body,
    required String idempotencyKey,
    String? roomId,
  }) async {
    final ownerUid = _ownerUid;
    try {
      final response = await _post(
        path,
        body,
        idempotencyKey: idempotencyKey,
        expectedOwnerUid: ownerUid,
      );
      _assertOwnerUid(ownerUid);
      return MutationResult(queued: false, response: response);
    } on DioException catch (error) {
      _assertOwnerUid(ownerUid);
      if (!_isRetryable(error)) rethrow;
      await _database.transaction(() async {
        if (roomId != null) {
          final hasCachedRoom = await _hasCachedRoom(ownerUid, roomId);
          _assertOwnerUid(ownerUid);
          if (!hasCachedRoom) throw StateError('ROOM_REMOVED');
        }
        _assertOwnerUid(ownerUid);
        await _database
            .into(_database.mutationOutbox)
            .insert(
              MutationOutboxCompanion.insert(
                ownerUid: ownerUid,
                id: const Uuid().v4(),
                mutationType: mutationType,
                payloadJson: jsonEncode({
                  'path': path,
                  'body': body,
                  'idempotencyKey': idempotencyKey,
                  'roomId': ?roomId,
                }),
                nextRetryAt: DateTime.now(),
                createdAt: DateTime.now(),
              ),
            );
        _assertOwnerUid(ownerUid);
      });
      _assertOwnerUid(ownerUid);
      return const MutationResult(queued: true);
    }
  }

  Future<void> flush() async {
    final ownerUid = _ownerUid;
    if (!_flushingOwnerUids.add(ownerUid)) return;
    try {
      final pending =
          await (_database.select(_database.mutationOutbox)
                ..where(
                  (row) =>
                      row.ownerUid.equals(ownerUid) &
                      row.status.equals('PENDING') &
                      row.nextRetryAt.isSmallerOrEqualValue(DateTime.now()),
                )
                ..orderBy([(row) => OrderingTerm.asc(row.createdAt)]))
              .get();
      _assertOwnerUid(ownerUid);
      for (final mutation in pending) {
        _assertOwnerUid(ownerUid);
        final payload =
            jsonDecode(mutation.payloadJson) as Map<String, dynamic>;
        final roomId = _payloadRoomId(payload);
        if (roomId != null) {
          final hasCachedRoom = await _hasCachedRoom(ownerUid, roomId);
          _assertOwnerUid(ownerUid);
          if (!hasCachedRoom) {
            await (_database.delete(_database.mutationOutbox)..where(
                  (row) =>
                      row.ownerUid.equals(ownerUid) &
                      row.id.equals(mutation.id),
                ))
                .go();
            _assertOwnerUid(ownerUid);
            continue;
          }
        }
        try {
          _assertOwnerUid(ownerUid);
          await _post(
            payload['path'] as String,
            Map<String, dynamic>.from(payload['body'] as Map),
            idempotencyKey: payload['idempotencyKey'] as String,
            expectedOwnerUid: ownerUid,
          );
          _assertOwnerUid(ownerUid);
          await (_database.delete(_database.mutationOutbox)..where(
                (row) =>
                    row.ownerUid.equals(ownerUid) & row.id.equals(mutation.id),
              ))
              .go();
          _assertOwnerUid(ownerUid);
        } on DioException catch (error) {
          _assertOwnerUid(ownerUid);
          if (!_isRetryable(error)) {
            await (_database.update(_database.mutationOutbox)..where(
                  (row) =>
                      row.ownerUid.equals(ownerUid) &
                      row.id.equals(mutation.id),
                ))
                .write(
                  MutationOutboxCompanion(
                    status: const Value('FAILED'),
                    lastErrorCode: Value(
                      'HTTP_${error.response?.statusCode ?? 'UNKNOWN'}',
                    ),
                  ),
                );
            _assertOwnerUid(ownerUid);
            continue;
          }
          final attempts = mutation.attemptCount + 1;
          final seconds = (1 << attempts.clamp(0, 8)) * 5;
          await (_database.update(_database.mutationOutbox)..where(
                (row) =>
                    row.ownerUid.equals(ownerUid) & row.id.equals(mutation.id),
              ))
              .write(
                MutationOutboxCompanion(
                  attemptCount: Value(attempts),
                  nextRetryAt: Value(
                    DateTime.now().add(Duration(seconds: seconds)),
                  ),
                  lastErrorCode: const Value('NETWORK_UNAVAILABLE'),
                ),
              );
          _assertOwnerUid(ownerUid);
        }
      }
    } finally {
      _flushingOwnerUids.remove(ownerUid);
    }
  }

  Future<MutationQueueSummary> summary() async {
    final rows = await (_database.select(
      _database.mutationOutbox,
    )..where((row) => row.ownerUid.equals(_ownerUid))).get();
    return MutationQueueSummary(
      pending: rows.where((row) => row.status == 'PENDING').length,
      failed: rows.where((row) => row.status == 'FAILED').length,
    );
  }

  Future<void> discardFailed() async {
    await (_database.delete(_database.mutationOutbox)..where(
          (row) => row.ownerUid.equals(_ownerUid) & row.status.equals('FAILED'),
        ))
        .go();
  }

  Future<bool> _hasCachedRoom(String ownerUid, String roomId) async {
    final room =
        await (_database.select(_database.cachedRooms)..where(
              (row) => row.ownerUid.equals(ownerUid) & row.id.equals(roomId),
            ))
            .getSingleOrNull();
    return room != null;
  }

  void _assertOwnerUid(String expectedOwnerUid) {
    if (_readOwnerUid() != expectedOwnerUid) {
      throw StateError('ACCOUNT_CHANGED');
    }
  }

  String? _payloadRoomId(Map<String, dynamic> payload) {
    final explicit = payload['roomId'];
    if (explicit is String && explicit.isNotEmpty) return explicit;
    final path = payload['path'];
    if (path is! String) return null;
    final segments = Uri.tryParse(path)?.pathSegments ?? const <String>[];
    for (var index = 0; index + 1 < segments.length; index++) {
      if (segments[index] == 'rooms' && segments[index + 1].isNotEmpty) {
        return segments[index + 1];
      }
    }
    return null;
  }

  bool _isRetryable(DioException error) {
    return switch (error.type) {
      DioExceptionType.connectionError ||
      DioExceptionType.connectionTimeout ||
      DioExceptionType.sendTimeout ||
      DioExceptionType.receiveTimeout => true,
      DioExceptionType.badResponse => (error.response?.statusCode ?? 0) >= 500,
      _ => false,
    };
  }
}
