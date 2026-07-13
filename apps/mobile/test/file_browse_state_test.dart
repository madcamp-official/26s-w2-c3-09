import 'package:dio/dio.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:mousekeeper/features/files/files_page.dart';

void main() {
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
}
