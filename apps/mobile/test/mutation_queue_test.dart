import 'package:dio/dio.dart';
import 'package:drift/native.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:mousekeeper/core/sync/mutation_queue.dart';
import 'package:mousekeeper/storage/app_database.dart';

void main() {
  late AppDatabase database;
  late String mode;
  late MutationQueue queue;

  Future<Map<String, dynamic>> post(
    String path,
    Map<String, dynamic> body, {
    String? idempotencyKey,
  }) async {
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
    return {'id': 'server-result'};
  }

  setUp(() {
    database = AppDatabase.forTesting(NativeDatabase.memory());
    mode = 'offline';
    queue = MutationQueue(database, post, () => 'firebase-user-a');
  });

  tearDown(() => database.close());

  test('네트워크 실패는 보관하고 서버 ACK 뒤에만 삭제한다', () async {
    final result = await queue.postOrQueue(
      mutationType: 'CREATE_COMMAND',
      path: '/v1/rooms/room/commands',
      body: {'intent': 'ANALYZE'},
      idempotencyKey: 'idempotency-key-1',
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
}
