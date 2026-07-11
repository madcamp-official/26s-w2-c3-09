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
  bool _flushing = false;

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
  }) async {
    try {
      final response = await _post(path, body, idempotencyKey: idempotencyKey);
      return MutationResult(queued: false, response: response);
    } on DioException catch (error) {
      if (!_isRetryable(error)) rethrow;
      await _database
          .into(_database.mutationOutbox)
          .insert(
            MutationOutboxCompanion.insert(
              ownerUid: _ownerUid,
              id: const Uuid().v4(),
              mutationType: mutationType,
              payloadJson: jsonEncode({
                'path': path,
                'body': body,
                'idempotencyKey': idempotencyKey,
              }),
              nextRetryAt: DateTime.now(),
              createdAt: DateTime.now(),
            ),
          );
      return const MutationResult(queued: true);
    }
  }

  Future<void> flush() async {
    if (_flushing) return;
    _flushing = true;
    try {
      final pending =
          await (_database.select(_database.mutationOutbox)
                ..where(
                  (row) =>
                      row.ownerUid.equals(_ownerUid) &
                      row.status.equals('PENDING') &
                      row.nextRetryAt.isSmallerOrEqualValue(DateTime.now()),
                )
                ..orderBy([(row) => OrderingTerm.asc(row.createdAt)]))
              .get();
      for (final mutation in pending) {
        final payload =
            jsonDecode(mutation.payloadJson) as Map<String, dynamic>;
        try {
          await _post(
            payload['path'] as String,
            Map<String, dynamic>.from(payload['body'] as Map),
            idempotencyKey: payload['idempotencyKey'] as String,
          );
          await (_database.delete(
            _database.mutationOutbox,
          )..where((row) => row.id.equals(mutation.id))).go();
        } on DioException catch (error) {
          if (!_isRetryable(error)) {
            await (_database.update(
              _database.mutationOutbox,
            )..where((row) => row.id.equals(mutation.id))).write(
              MutationOutboxCompanion(
                status: const Value('FAILED'),
                lastErrorCode: Value(
                  'HTTP_${error.response?.statusCode ?? 'UNKNOWN'}',
                ),
              ),
            );
            continue;
          }
          final attempts = mutation.attemptCount + 1;
          final seconds = (1 << attempts.clamp(0, 8)) * 5;
          await (_database.update(
            _database.mutationOutbox,
          )..where((row) => row.id.equals(mutation.id))).write(
            MutationOutboxCompanion(
              attemptCount: Value(attempts),
              nextRetryAt: Value(
                DateTime.now().add(Duration(seconds: seconds)),
              ),
              lastErrorCode: const Value('NETWORK_UNAVAILABLE'),
            ),
          );
        }
      }
    } finally {
      _flushing = false;
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
