import 'package:dio/dio.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:mousekeeper/core/sync/realtime_controller.dart';
import 'package:mousekeeper/features/files/files_page.dart';

void main() {
  test('file directory state keeps entries and cursor metadata together', () {
    final first = const FileDirectoryState.empty().withPage(
      received: [
        {'relativePath': 'first.pdf'},
      ],
      append: false,
      nextCursor: 'cursor-1',
      generation: 'generation-1',
    );

    expect(first.entries.map((entry) => entry['relativePath']), ['first.pdf']);
    expect(first.nextCursor, 'cursor-1');
    expect(first.generation, 'generation-1');
    expect(
      () => first.entries.add({'relativePath': 'mutate.pdf'}),
      throwsA(isA<UnsupportedError>()),
    );

    final second = first.withPage(
      received: [
        {'relativePath': 'second.pdf'},
      ],
      append: true,
      nextCursor: null,
      generation: 'generation-1',
    );

    expect(second.entries.map((entry) => entry['relativePath']), [
      'first.pdf',
      'second.pdf',
    ]);
    expect(second.nextCursor, isNull);
    expect(second.generation, 'generation-1');
  });

  test('file directory update patches only the visible directory entries', () {
    final current = FileDirectoryState(
      entries: [
        _entry('reports/old.pdf', name: 'old.pdf'),
        _entry('reports/work', name: 'work', type: 'DIRECTORY'),
      ],
      nextCursor: null,
      generation: 'generation-1',
    );

    final added = current.applyUpdate(
      currentRelativeDirectory: 'reports',
      update: FileDirectoryUpdate(
        kind: FileDirectoryUpdateKind.added,
        parentRelativePath: 'reports',
        entry: _entry('reports/a-new.pdf', name: 'a-new.pdf'),
      ),
    );
    expect(added.entries.map((entry) => entry['name']), [
      'work',
      'a-new.pdf',
      'old.pdf',
    ]);

    final unrelated = added.applyUpdate(
      currentRelativeDirectory: 'reports',
      update: FileDirectoryUpdate(
        kind: FileDirectoryUpdateKind.added,
        parentRelativePath: 'elsewhere',
        entry: _entry('elsewhere/ignored.pdf', name: 'ignored.pdf'),
      ),
    );
    expect(identical(unrelated, added), isTrue);

    final updated = added.applyUpdate(
      currentRelativeDirectory: 'reports',
      update: FileDirectoryUpdate(
        kind: FileDirectoryUpdateKind.updated,
        parentRelativePath: 'reports',
        entry: _entry('reports/old.pdf', name: 'old.pdf', sizeBytes: 99),
      ),
    );
    expect(
      updated.entries.singleWhere(
        (entry) => entry['relativePath'] == 'reports/old.pdf',
      )['sizeBytes'],
      99,
    );

    final removed = updated.applyUpdate(
      currentRelativeDirectory: 'reports',
      update: const FileDirectoryUpdate(
        kind: FileDirectoryUpdateKind.removed,
        parentRelativePath: 'reports',
        relativePath: 'reports/a-new.pdf',
      ),
    );
    expect(removed.entries.map((entry) => entry['relativePath']), [
      'reports/work',
      'reports/old.pdf',
    ]);
  });

  test(
    'file directory move updates source and destination rows without reload',
    () {
      final current = FileDirectoryState(
        entries: [
          _entry('reports/old.pdf', name: 'old.pdf'),
          _entry('reports/work', name: 'work', type: 'DIRECTORY'),
        ],
        nextCursor: null,
        generation: 'generation-1',
      );

      final renamed = current.applyUpdate(
        currentRelativeDirectory: 'reports',
        update: FileDirectoryUpdate(
          kind: FileDirectoryUpdateKind.moved,
          previousRelativePath: 'reports/old.pdf',
          parentRelativePath: 'reports',
          entry: _entry('reports/final.pdf', name: 'final.pdf'),
        ),
      );

      expect(renamed.entries.map((entry) => entry['relativePath']), [
        'reports/work',
        'reports/final.pdf',
      ]);

      final movedAway = renamed.applyUpdate(
        currentRelativeDirectory: 'reports',
        update: FileDirectoryUpdate(
          kind: FileDirectoryUpdateKind.moved,
          previousRelativePath: 'reports/final.pdf',
          parentRelativePath: 'archive',
          entry: _entry('archive/final.pdf', name: 'final.pdf'),
        ),
      );

      expect(movedAway.entries.map((entry) => entry['relativePath']), [
        'reports/work',
      ]);
    },
  );

  test(
    'file directory update marks paginated directory stale when range is uncertain',
    () {
      final current = FileDirectoryState(
        entries: [_entry('reports/middle.pdf', name: 'middle.pdf')],
        nextCursor: 'offset:200',
        generation: 'generation-1',
      );

      final patched = current.applyUpdate(
        currentRelativeDirectory: 'reports',
        update: FileDirectoryUpdate(
          kind: FileDirectoryUpdateKind.added,
          parentRelativePath: 'reports',
          entry: _entry(
            'reports/unknown-position.pdf',
            name: 'unknown-position.pdf',
          ),
        ),
      );

      expect(patched.entries.map((entry) => entry['relativePath']), [
        'reports/middle.pdf',
      ]);
      expect(patched.isStale, isTrue);
      expect(patched.nextCursor, 'offset:200');
    },
  );

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

Map<String, dynamic> _entry(
  String relativePath, {
  required String name,
  String type = 'FILE',
  int sizeBytes = 10,
}) => {
  'type': type,
  'name': name,
  'relativePath': relativePath,
  'sizeBytes': type == 'FILE' ? sizeBytes : null,
  'modifiedAt': '2026-07-13T00:00:00.000Z',
};
