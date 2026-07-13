import 'package:dio/dio.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:mousekeeper/core/sync/realtime_controller.dart';
import 'package:mousekeeper/features/files/files_page.dart';

void main() {
  test('file browse status waits on websocket before slow REST fallback', () {
    expect(fileBrowseStatusFallbackInterval, const Duration(seconds: 5));
  });

  test('다음 페이지 실패 전에는 기존 READY page를 지우지 않는다', () {
    final existing = <Map<String, dynamic>>[
      {'relativePath': 'first.pdf'},
    ];
    expect(shouldClearBrowseEntries(append: true), isFalse);
    expect(
      mergeBrowseEntries(
        existing: existing,
        received: [
          {'relativePath': 'second.pdf'},
        ],
        append: true,
      ).map((entry) => entry['relativePath']),
      ['first.pdf', 'second.pdf'],
    );
  });

  test('Dio 오류 body의 명시적 code를 사용자 상태로 보존한다', () {
    final offline = DioException(
      requestOptions: RequestOptions(path: '/browse'),
      response: Response<Map<String, dynamic>>(
        requestOptions: RequestOptions(path: '/browse'),
        statusCode: 409,
        data: const {'code': 'DEVICE_OFFLINE'},
      ),
      type: DioExceptionType.badResponse,
    );
    expect(fileOperationErrorCode(offline), 'DEVICE_OFFLINE');
    expect(fileOperationErrorMessage(offline), contains('이전에 받은 목록'));
    expect(
      fileOperationErrorMessage(StateError('CURSOR_INVALIDATED')),
      contains('첫 페이지'),
    );
    expect(
      fileOperationErrorMessage(StateError('SOURCE_CHANGED')),
      contains('원본 파일이 변경'),
    );
  });
  test('file transfer realtime update patches only the matching transfer', () {
    final current = {
      'id': 'transfer-a',
      'status': 'REQUESTED',
      'failureCode': 'OLD',
    };

    final unrelated = patchFileTransferStateForRealtimeUpdate(
      current: current,
      update: const RealtimeFileTransferUpdate(
        transferId: 'transfer-b',
        status: 'READY',
      ),
    );
    expect(identical(unrelated, current), isTrue);

    final ready = patchFileTransferStateForRealtimeUpdate(
      current: current,
      update: const RealtimeFileTransferUpdate(
        transferId: 'transfer-a',
        status: 'READY',
      ),
    );
    expect(ready['status'], 'READY');
    expect(ready.containsKey('failureCode'), isFalse);

    final failed = patchFileTransferStateForRealtimeUpdate(
      current: ready,
      update: const RealtimeFileTransferUpdate(
        transferId: 'transfer-a',
        status: 'FAILED',
        failureCode: 'SOURCE_CHANGED',
      ),
    );
    expect(failed['status'], 'FAILED');
    expect(failed['failureCode'], 'SOURCE_CHANGED');
  });
}
