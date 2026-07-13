import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:mousekeeper/features/files/files_page.dart';

void main() {
  testWidgets('browse displays metadata and navigable breadcrumb', (
    tester,
  ) async {
    final gateway = _QueuedBrowseGateway()
      ..enqueue(
        _ready([
          {
            'name': 'docs',
            'relativePath': 'docs',
            'type': 'DIRECTORY',
            'sizeBytes': null,
            'modifiedAt': '2026-07-13T01:02:00.000Z',
          },
          {
            'name': 'report.pdf',
            'relativePath': 'report.pdf',
            'type': 'FILE',
            'sizeBytes': 2048,
            'modifiedAt': '2026-07-13T01:02:00.000Z',
          },
        ]),
      )
      ..enqueue(_ready(const []));
    await _pumpFiles(tester, gateway);

    expect(find.text('report.pdf'), findsOneWidget);
    expect(find.textContaining('2.0 KB'), findsOneWidget);
    expect(find.textContaining('2026.'), findsWidgets);
    expect(find.text('관리 폴더'), findsOneWidget);

    await tester.tap(find.text('docs'));
    await tester.pumpAndSettle();

    expect(gateway.bodies.last['relativeDirectory'], 'docs');
    expect(find.text('docs'), findsOneWidget);
  });

  testWidgets('search waits 300ms, requires 2 chars, and sends both scopes', (
    tester,
  ) async {
    final gateway = _QueuedBrowseGateway()
      ..enqueue(_ready(const []))
      ..enqueue(_ready(const []))
      ..enqueue(_ready(const []));
    await _pumpFiles(tester, gateway);
    expect(gateway.bodies, hasLength(1));

    await tester.enterText(
      find.byKey(const ValueKey('file-search-field')),
      'a',
    );
    await tester.pump(const Duration(milliseconds: 400));
    expect(gateway.bodies, hasLength(1));

    await tester.enterText(
      find.byKey(const ValueKey('file-search-field')),
      'ab',
    );
    await tester.pump(const Duration(milliseconds: 299));
    expect(gateway.bodies, hasLength(1));
    await tester.pump(const Duration(milliseconds: 1));
    await tester.pumpAndSettle();
    expect(gateway.bodies, hasLength(2));
    expect(gateway.bodies.last['query'], 'ab');
    expect(gateway.bodies.last['searchScope'], fileSearchScopeDirectory);

    await tester.tap(find.byKey(const ValueKey('file-search-scope')));
    await tester.pumpAndSettle();
    await tester.tap(find.text('전체 폴더').last);
    await tester.pumpAndSettle();

    expect(gateway.bodies, hasLength(3));
    expect(gateway.bodies.last['query'], 'ab');
    expect(gateway.bodies.last['searchScope'], fileSearchScopeManagedRoot);
  });

  testWidgets('search length uses Unicode code points and blocks over 100', (
    tester,
  ) async {
    final gateway = _QueuedBrowseGateway()
      ..enqueue(_ready(const []))
      ..enqueue(_ready(const []));
    await _pumpFiles(tester, gateway);
    final field = find.byKey(const ValueKey('file-search-field'));

    await tester.enterText(field, '😀');
    await tester.pump(const Duration(milliseconds: 400));
    expect(gateway.bodies, hasLength(1));
    expect(find.text('검색어를 2자 이상 입력해 주세요.'), findsOneWidget);

    await tester.enterText(field, '😀a');
    await tester.pump(const Duration(milliseconds: 300));
    await tester.pumpAndSettle();
    expect(gateway.bodies, hasLength(2));
    expect(gateway.bodies.last['query'], '😀a');

    await tester.enterText(field, List.filled(101, 'a').join());
    await tester.pump(const Duration(milliseconds: 400));
    expect(gateway.bodies, hasLength(2));
    expect(find.text('검색어를 100자 이하로 입력해 주세요.'), findsOneWidget);
  });

  testWidgets(
    'late response from an older search cannot replace latest results',
    (tester) async {
      final older = Completer<Map<String, dynamic>>();
      final gateway = _QueuedBrowseGateway()
        ..enqueue(_ready(const []))
        ..enqueueFuture(older.future)
        ..enqueue(
          _ready([
            {
              'name': 'latest.txt',
              'relativePath': 'latest.txt',
              'type': 'FILE',
              'sizeBytes': 10,
              'modifiedAt': '2026-07-13T01:02:00.000Z',
            },
          ]),
        );
      await _pumpFiles(tester, gateway);

      await tester.enterText(
        find.byKey(const ValueKey('file-search-field')),
        'ab',
      );
      await tester.pump(const Duration(milliseconds: 300));
      await tester.pump();
      expect(gateway.bodies, hasLength(2));

      await tester.enterText(
        find.byKey(const ValueKey('file-search-field')),
        'abc',
      );
      await tester.pump(const Duration(milliseconds: 300));
      await tester.pumpAndSettle();
      expect(find.text('latest.txt'), findsOneWidget);

      older.complete(
        _ready([
          {
            'name': 'stale.txt',
            'relativePath': 'stale.txt',
            'type': 'FILE',
            'sizeBytes': 10,
            'modifiedAt': '2026-07-13T01:02:00.000Z',
          },
        ]),
      );
      await tester.pumpAndSettle();

      expect(find.text('latest.txt'), findsOneWidget);
      expect(find.text('stale.txt'), findsNothing);
    },
  );

  testWidgets('cursor invalidation drops stale pages and restarts once', (
    tester,
  ) async {
    final gateway = _QueuedBrowseGateway()
      ..enqueue(
        _ready([
          {
            'name': 'stale.txt',
            'relativePath': 'stale.txt',
            'type': 'FILE',
            'sizeBytes': 1,
            'modifiedAt': '2026-07-13T01:02:00.000Z',
          },
        ], nextCursor: 'cursor-1'),
      )
      ..enqueue(_failed('CURSOR_INVALIDATED'))
      ..enqueue(
        _ready([
          {
            'name': 'fresh.txt',
            'relativePath': 'fresh.txt',
            'type': 'FILE',
            'sizeBytes': 2,
            'modifiedAt': '2026-07-13T01:02:00.000Z',
          },
        ]),
      );
    await _pumpFiles(tester, gateway);

    await tester.tap(find.text('다음 파일 불러오기'));
    await tester.pumpAndSettle();

    expect(gateway.bodies, hasLength(3));
    expect(gateway.bodies[1]['cursor'], 'cursor-1');
    expect(gateway.bodies[2]['cursor'], isNull);
    expect(find.text('stale.txt'), findsNothing);
    expect(find.text('fresh.txt'), findsOneWidget);
  });
}

Future<void> _pumpFiles(WidgetTester tester, FileBrowseGateway gateway) async {
  await tester.pumpWidget(
    ProviderScope(
      child: MaterialApp(
        home: FilesPage(
          roomId: 'room-a',
          roomName: '문서',
          browseGateway: gateway,
          enforceConnectionGuard: false,
        ),
      ),
    ),
  );
  await tester.pumpAndSettle();
}

Map<String, dynamic> _ready(
  List<Map<String, dynamic>> entries, {
  String? nextCursor,
}) => {
  'status': 'READY',
  'desktopGeneration': 'generation-1',
  'resultPage': {'entries': entries, 'nextCursor': nextCursor},
};

Map<String, dynamic> _failed(String code) => {
  'status': 'FAILED',
  'failureCode': code,
};

class _QueuedBrowseGateway implements FileBrowseGateway {
  final List<Map<String, dynamic>> bodies = [];
  final List<Future<Map<String, dynamic>>> _queued = [];
  final Map<String, Future<Map<String, dynamic>>> _responses = {};

  void enqueue(Map<String, dynamic> response) {
    _queued.add(Future.value(response));
  }

  void enqueueFuture(Future<Map<String, dynamic>> response) {
    _queued.add(response);
  }

  @override
  Future<Map<String, dynamic>> createRequest(
    String roomId,
    Map<String, dynamic> body,
  ) async {
    bodies.add(Map<String, dynamic>.from(body));
    final id = 'request-${bodies.length}';
    if (_queued.isEmpty) throw StateError('NO_QUEUED_BROWSE_RESPONSE');
    _responses[id] = _queued.removeAt(0);
    return {'id': id};
  }

  @override
  Future<Map<String, dynamic>> getRequest(String requestId) async =>
      _responses[requestId] ??
      (throw StateError('UNKNOWN_BROWSE_REQUEST: $requestId'));
}
