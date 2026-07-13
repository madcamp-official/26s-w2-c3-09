import 'dart:async';

import 'package:dio/dio.dart';
import 'package:drift/native.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:mousekeeper/core/sync/mutation_queue.dart';
import 'package:mousekeeper/storage/app_database.dart';

void main() {
  late AppDatabase database;
  late String mode;
  late String? currentUid;
  late MutationQueue queue;
  late List<String> postedPaths;
  late List<String?> apiInvocationOwnerUids;
  late Completer<void> accountAFlushStarted;
  late Completer<void> releaseAccountAFlush;

  Future<Map<String, dynamic>> post(
    String path,
    Map<String, dynamic> body, {
    String? idempotencyKey,
    String? expectedOwnerUid,
  }) async {
    expect(expectedOwnerUid, currentUid);
    postedPaths.add(path);
    apiInvocationOwnerUids.add(currentUid);
    final request = RequestOptions(path: path);
    if (mode == 'offline') {
      throw DioException(
        requestOptions: request,
        type: DioExceptionType.connectionError,
      );
    }
    if (mode == 'rejected') {
      throw DioException(
        requestOptions: request,
        type: DioExceptionType.badResponse,
        response: Response<void>(requestOptions: request, statusCode: 400),
      );
    }
    if (mode == 'switch-account-then-succeed') {
      currentUid = 'firebase-user-b';
      await Future<void>.delayed(Duration.zero);
      return {'id': 'server-result'};
    }
    if (mode == 'switch-account-then-reject') {
      currentUid = 'firebase-user-b';
      await Future<void>.delayed(Duration.zero);
      throw DioException(
        requestOptions: request,
        type: DioExceptionType.badResponse,
        response: Response<void>(requestOptions: request, statusCode: 400),
      );
    }
    if (mode == 'switch-account-then-offline') {
      currentUid = 'firebase-user-b';
      await Future<void>.delayed(Duration.zero);
      throw DioException(
        requestOptions: request,
        type: DioExceptionType.connectionError,
      );
    }
    if (mode == 'hold-account-a' && expectedOwnerUid == 'firebase-user-a') {
      accountAFlushStarted.complete();
      await releaseAccountAFlush.future;
    }
    return {'id': 'server-result'};
  }

  setUp(() {
    database = AppDatabase.forTesting(NativeDatabase.memory());
    mode = 'offline';
    currentUid = 'firebase-user-a';
    postedPaths = [];
    apiInvocationOwnerUids = [];
    accountAFlushStarted = Completer<void>();
    releaseAccountAFlush = Completer<void>();
    queue = MutationQueue(database, post, () => currentUid);
  });

  tearDown(() => database.close());

  test('네트워크 실패는 보관하고 서버 ACK 뒤에만 삭제한다', () async {
    await _cacheRoom(database, 'room');
    final result = await queue.postOrQueue(
      mutationType: 'CREATE_COMMAND',
      path: '/v1/rooms/room/commands',
      body: {'intent': 'ANALYZE'},
      idempotencyKey: 'idempotency-key-1',
      roomId: 'room',
    );
    expect(result.queued, isTrue);
    expect((await queue.summary()).pending, 1);

    mode = 'success';
    await queue.flush();
    final summary = await queue.summary();
    expect(summary.pending, 0);
    expect(summary.failed, 0);
  });

  test('terminal 4xx는 무한 재시도하지 않고 FAILED로 남긴다', () async {
    await queue.postOrQueue(
      mutationType: 'CREATE_DECISION',
      path: '/v1/proposals/proposal/decisions',
      body: {'decisionType': 'APPROVE'},
      idempotencyKey: 'idempotency-key-2',
    );

    mode = 'rejected';
    await queue.flush();

    final summary = await queue.summary();
    expect(summary.pending, 0);
    expect(summary.failed, 1);
    final row = await database.select(database.mutationOutbox).getSingle();
    expect(row.lastErrorCode, 'HTTP_400');
  });

  test(
    'removed room mutation cannot be queued after lifecycle purge',
    () async {
      await expectLater(
        queue.postOrQueue(
          mutationType: 'CREATE_COMMAND',
          path: '/v1/rooms/removed-room/commands',
          body: {'intent': 'ANALYZE'},
          idempotencyKey: 'idempotency-key-3',
          roomId: 'removed-room',
        ),
        throwsA(isA<StateError>()),
      );
      expect(await database.select(database.mutationOutbox).get(), isEmpty);
    },
  );

  test(
    'flush drops a queued room mutation if its room is no longer active',
    () async {
      await _cacheRoom(database, 'room');
      await queue.postOrQueue(
        mutationType: 'CREATE_COMMAND',
        path: '/v1/rooms/room/commands',
        body: {'intent': 'ANALYZE'},
        idempotencyKey: 'idempotency-key-4',
        roomId: 'room',
      );
      await database.delete(database.cachedRooms).go();
      postedPaths.clear();
      mode = 'success';

      await queue.flush();

      expect(postedPaths, isEmpty);
      expect(await database.select(database.mutationOutbox).get(), isEmpty);
    },
  );

  test(
    'account switch during ACK leaves the original owner outbox untouched',
    () async {
      await queue.postOrQueue(
        mutationType: 'CREATE_DECISION',
        path: '/v1/proposals/proposal-a/decisions',
        body: {'decisionType': 'APPROVE'},
        idempotencyKey: 'account-switch-success',
      );
      postedPaths.clear();
      apiInvocationOwnerUids.clear();
      mode = 'switch-account-then-succeed';

      await expectLater(
        queue.flush(),
        throwsA(
          isA<StateError>().having(
            (error) => error.message,
            'message',
            'ACCOUNT_CHANGED',
          ),
        ),
      );

      expect(apiInvocationOwnerUids, ['firebase-user-a']);
      final row = await database.select(database.mutationOutbox).getSingle();
      expect(row.ownerUid, 'firebase-user-a');
      expect(row.status, 'PENDING');
      expect(row.attemptCount, 0);
      expect(row.lastErrorCode, isNull);
    },
  );

  test(
    'account switch during a rejected request does not mark A outbox failed',
    () async {
      await queue.postOrQueue(
        mutationType: 'CREATE_DECISION',
        path: '/v1/proposals/proposal-a/decisions',
        body: {'decisionType': 'APPROVE'},
        idempotencyKey: 'account-switch-rejected',
      );
      postedPaths.clear();
      apiInvocationOwnerUids.clear();
      mode = 'switch-account-then-reject';

      await expectLater(
        queue.flush(),
        throwsA(
          isA<StateError>().having(
            (error) => error.message,
            'message',
            'ACCOUNT_CHANGED',
          ),
        ),
      );

      expect(apiInvocationOwnerUids, ['firebase-user-a']);
      final row = await database.select(database.mutationOutbox).getSingle();
      expect(row.ownerUid, 'firebase-user-a');
      expect(row.status, 'PENDING');
      expect(row.attemptCount, 0);
      expect(row.lastErrorCode, isNull);
    },
  );

  test(
    'postOrQueue does not enqueue for A or B when account changes in flight',
    () async {
      mode = 'switch-account-then-offline';

      await expectLater(
        queue.postOrQueue(
          mutationType: 'CREATE_DECISION',
          path: '/v1/proposals/proposal-a/decisions',
          body: {'decisionType': 'APPROVE'},
          idempotencyKey: 'account-switch-before-enqueue',
        ),
        throwsA(
          isA<StateError>().having(
            (error) => error.message,
            'message',
            'ACCOUNT_CHANGED',
          ),
        ),
      );

      expect(apiInvocationOwnerUids, ['firebase-user-a']);
      expect(await database.select(database.mutationOutbox).get(), isEmpty);
    },
  );

  test(
    'account B flush starts while account A flush is still in flight',
    () async {
      await queue.postOrQueue(
        mutationType: 'CREATE_DECISION',
        path: '/v1/proposals/proposal-a/decisions',
        body: {'decisionType': 'APPROVE'},
        idempotencyKey: 'parallel-owner-a',
      );
      currentUid = 'firebase-user-b';
      await queue.postOrQueue(
        mutationType: 'CREATE_DECISION',
        path: '/v1/proposals/proposal-b/decisions',
        body: {'decisionType': 'APPROVE'},
        idempotencyKey: 'parallel-owner-b',
      );

      mode = 'hold-account-a';
      currentUid = 'firebase-user-a';
      final accountAFlush = queue.flush();
      await accountAFlushStarted.future;

      currentUid = 'firebase-user-b';
      await queue.flush();

      final rowsBeforeACompletes = await database
          .select(database.mutationOutbox)
          .get();
      expect(rowsBeforeACompletes, hasLength(1));
      expect(rowsBeforeACompletes.single.ownerUid, 'firebase-user-a');
      expect(rowsBeforeACompletes.single.status, 'PENDING');

      releaseAccountAFlush.complete();
      await expectLater(
        accountAFlush,
        throwsA(
          isA<StateError>().having(
            (error) => error.message,
            'message',
            'ACCOUNT_CHANGED',
          ),
        ),
      );

      final finalRows = await database.select(database.mutationOutbox).get();
      expect(finalRows, hasLength(1));
      expect(finalRows.single.ownerUid, 'firebase-user-a');
    },
  );
}

Future<void> _cacheRoom(AppDatabase database, String roomId) {
  return database
      .into(database.cachedRooms)
      .insert(
        CachedRoomsCompanion.insert(
          ownerUid: 'firebase-user-a',
          id: roomId,
          payloadJson: '{"id":"$roomId","status":"ACTIVE"}',
          updatedAt: DateTime.now(),
        ),
      );
}
