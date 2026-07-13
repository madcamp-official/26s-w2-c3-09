import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:mousekeeper/features/chat/chat_page.dart';

void main() {
  testWidgets('loads chat sessions and switches selected session', (
    tester,
  ) async {
    final gateway = _FakeChatGateway(
      sessions: [_session('s1', '첫 대화'), _session('s2', '둘째 대화')],
      messagesBySession: {
        's1': [_message('m1', 's1', 'USER', '첫 메시지')],
        's2': [_message('m2', 's2', 'USER', '둘째 메시지')],
      },
    );

    await _pumpChat(tester, gateway);

    expect(find.text('첫 대화'), findsOneWidget);
    expect(find.text('첫 메시지'), findsOneWidget);
    expect(gateway.messageLoads, {'s1': 1});

    await tester.tap(find.byKey(const ValueKey('chat-session-picker')));
    await tester.pumpAndSettle();
    await tester.tap(find.text('둘째 대화').last);
    await tester.pumpAndSettle();

    expect(find.text('둘째 메시지'), findsOneWidget);
    expect(find.text('첫 메시지'), findsNothing);
    expect(gateway.messageLoads, {'s1': 1, 's2': 1});
  });

  testWidgets('sending appends user and assistant messages without reload', (
    tester,
  ) async {
    final gateway = _FakeChatGateway(
      sessions: [_session('s1', '첫 대화')],
      messagesBySession: {'s1': []},
      sendResult: {
        'message': _message('m-new', 's1', 'USER', '정리해줘'),
        'assistant': _message(
          'm-draft',
          's1',
          'ASSISTANT',
          '제가 이해한 내용: 정리 명령 초안입니다.',
          messageType: 'COMMAND_DRAFT',
        ),
        'aiStatus': 'READY',
        'ai': {
          'status': 'READY',
          'kind': 'COMMAND_DRAFT',
          'commandDraftId': 'draft-1',
        },
      },
    );

    await _pumpChat(tester, gateway);
    await tester.enterText(
      find.byKey(const ValueKey('chat-message-field')),
      '정리해줘',
    );
    await tester.tap(find.byKey(const ValueKey('chat-send-button')));
    await tester.pumpAndSettle();

    expect(find.text('정리해줘'), findsOneWidget);
    expect(find.text('제가 이해한 내용: 정리 명령 초안입니다.'), findsOneWidget);
    expect(find.text('확인 카드'), findsOneWidget);
    expect(gateway.messageLoads, {'s1': 1});
    expect(gateway.sentMessages, ['정리해줘']);
  });

  testWidgets('new session limit is shown without pretending success', (
    tester,
  ) async {
    final gateway = _FakeChatGateway(
      sessions: [_session('s1', '첫 대화')],
      messagesBySession: {'s1': []},
      createError: StateError('CHAT_SESSION_LIMIT_REACHED'),
    );

    await _pumpChat(tester, gateway);
    await tester.tap(find.byKey(const ValueKey('chat-create-session')));
    await tester.pumpAndSettle();

    expect(find.textContaining('최대 5개'), findsOneWidget);
    expect(gateway.createdSessions, 1);
  });
}

Future<void> _pumpChat(WidgetTester tester, _FakeChatGateway gateway) async {
  await tester.pumpWidget(
    ProviderScope(
      child: MaterialApp(
        home: ChatPage(roomId: 'room-1', gateway: gateway),
      ),
    ),
  );
  await tester.pumpAndSettle();
}

Map<String, dynamic> _session(String id, String title) => {
  'id': id,
  'roomId': 'room-1',
  'title': title,
  'summary': null,
  'status': 'ACTIVE',
  'createdAt': '2026-07-14T00:00:00.000Z',
  'updatedAt': '2026-07-14T00:00:00.000Z',
  'deletedAt': null,
  'messagePreview': '',
};

Map<String, dynamic> _message(
  String id,
  String sessionId,
  String senderType,
  String content, {
  String messageType = 'TEXT',
}) => {
  'id': id,
  'roomId': 'room-1',
  'sessionId': sessionId,
  'senderType': senderType,
  'messageType': messageType,
  'content': content,
  'structuredPayload': null,
  'commandId': null,
  'createdAt': '2026-07-14T00:00:00.000Z',
};

class _FakeChatGateway implements ChatGateway {
  _FakeChatGateway({
    required this.sessions,
    required this.messagesBySession,
    this.sendResult,
    this.createError,
  });

  final List<Map<String, dynamic>> sessions;
  final Map<String, List<Map<String, dynamic>>> messagesBySession;
  final Map<String, dynamic>? sendResult;
  final Object? createError;
  final Map<String, int> messageLoads = {};
  final List<String> sentMessages = [];
  int createdSessions = 0;

  @override
  Future<List<Map<String, dynamic>>> listSessions(String roomId) async => [
    ...sessions,
  ];

  @override
  Future<Map<String, dynamic>> createSession(String roomId) async {
    createdSessions += 1;
    final error = createError;
    if (error != null) throw error;
    final created = _session('created-$createdSessions', '새 대화');
    sessions.insert(0, created);
    messagesBySession[created['id'] as String] = [];
    return created;
  }

  @override
  Future<void> deleteSession(String sessionId) async {
    sessions.removeWhere((session) => session['id'] == sessionId);
    messagesBySession.remove(sessionId);
  }

  @override
  Future<List<Map<String, dynamic>>> listMessages(String sessionId) async {
    messageLoads[sessionId] = (messageLoads[sessionId] ?? 0) + 1;
    return [...messagesBySession[sessionId] ?? const []];
  }

  @override
  Future<Map<String, dynamic>> sendMessage(
    String sessionId,
    String content,
  ) async {
    sentMessages.add(content);
    return sendResult ??
        {
          'message': _message(
            'sent-${sentMessages.length}',
            sessionId,
            'USER',
            content,
          ),
          'assistant': null,
          'aiStatus': 'UNCONFIGURED',
          'ai': {'status': 'UNCONFIGURED', 'code': 'AI_PROVIDER_UNCONFIGURED'},
        };
  }
}
