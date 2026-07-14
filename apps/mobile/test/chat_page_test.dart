import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:mousekeeper/core/sync/realtime_controller.dart';
import 'package:mousekeeper/features/chat/chat_page.dart';

void main() {
  test('chat reducers preserve identity for no-op updates', () {
    final sessions = [_session('s1', '첫 대화')];
    expect(touchChatSessionPreview(sessions, 's1', ''), same(sessions));
    expect(
      touchChatSessionPreview(sessions, 'missing', 'hello'),
      same(sessions),
    );

    final messages = [
      _message(
        'm1',
        's1',
        'ASSISTANT',
        'draft',
        messageType: 'COMMAND_DRAFT',
        structuredPayload: {
          'id': 'draft-1',
          'status': 'DRAFT',
          'commandId': null,
        },
      ),
    ];
    expect(mergeChatMessages(messages, const []), same(messages));
    expect(mergeChatMessages(messages, [messages.single]), same(messages));
    expect(
      patchCommandDraftMessages(messages, 'draft-1', {
        'id': 'draft-1',
        'status': 'DRAFT',
        'commandId': null,
      }),
      same(messages),
    );
    expect(
      patchCommandDraftMessages(messages, 'missing', const {}),
      same(messages),
    );
    expect(
      replaceChatSession(sessions, _session('missing', '새 제목')),
      same(sessions),
    );
    expect(replaceChatSession(sessions, sessions.single), same(sessions));
  });

  test('chat conversation state keeps selection and pagination together', () {
    final state = ChatConversationState(
      sessions: [_session('s1', 'first session')],
      messages: const [],
      selectedSessionId: 's1',
      messageCursor: null,
      hasMoreMessages: false,
    );
    final page = [
      for (var index = 0; index < chatMessagePageSize; index += 1)
        _message('m$index', 's1', 'USER', 'message $index'),
    ];
    final loaded = state.withLoadedMessages(page);

    expect(loaded.selectedSessionId, 's1');
    expect(loaded.messageCursor, 'm${chatMessagePageSize - 1}');
    expect(loaded.hasMoreMessages, true);

    final switching = loaded.copyWith(
      selectedSessionId: 's2',
      messages: const [],
      clearMessageCursor: true,
      hasMoreMessages: false,
    );

    expect(switching.selectedSessionId, 's2');
    expect(switching.messages, isEmpty);
    expect(switching.messageCursor, isNull);
    expect(switching.hasMoreMessages, false);
  });

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

  testWidgets('provider backed chat loaders share the gateway provider', (
    tester,
  ) async {
    final gateway = _FakeChatGateway(
      sessions: [_session('s1', 'provider session')],
      messagesBySession: {
        's1': [_message('m1', 's1', 'USER', 'provider message')],
      },
    );

    await tester.pumpWidget(
      ProviderScope(
        overrides: [chatGatewayProvider.overrideWithValue(gateway)],
        child: const MaterialApp(home: ChatPage(roomId: 'room-1')),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.text('provider session'), findsOneWidget);
    expect(find.text('provider message'), findsOneWidget);
    expect(gateway.messageLoads, {'s1': 1});
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
          structuredPayload: {
            'id': 'draft-1',
            'status': 'DRAFT',
            'commandId': null,
          },
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
    expect(
      find.byKey(const ValueKey('chat-command-draft-confirm-draft-1')),
      findsOneWidget,
    );
    expect(gateway.messageLoads, {'s1': 1});
    expect(gateway.sentMessages, ['정리해줘']);
  });

  testWidgets('confirming a command draft patches the message without reload', (
    tester,
  ) async {
    final gateway = _FakeChatGateway(
      sessions: [_session('s1', '첫 대화')],
      messagesBySession: {
        's1': [
          _message(
            'm-draft',
            's1',
            'ASSISTANT',
            '이 작업을 실행할까요?',
            messageType: 'COMMAND_DRAFT',
            structuredPayload: {
              'id': 'draft-1',
              'status': 'DRAFT',
              'commandId': null,
            },
          ),
        ],
      },
    );

    await _pumpChat(tester, gateway);
    await tester.tap(
      find.byKey(const ValueKey('chat-command-draft-confirm-draft-1')),
    );
    await tester.pumpAndSettle();

    expect(gateway.confirmedDraftIds, ['draft-1']);
    expect(gateway.messageLoads, {'s1': 1});
    expect(
      find.byKey(const ValueKey('chat-command-draft-status-draft-1')),
      findsOneWidget,
    );
    expect(find.text('MATERIALIZED'), findsOneWidget);
  });

  testWidgets('rejecting a command draft patches the message without reload', (
    tester,
  ) async {
    final gateway = _FakeChatGateway(
      sessions: [_session('s1', '첫 대화')],
      messagesBySession: {
        's1': [
          _message(
            'm-draft',
            's1',
            'ASSISTANT',
            '이 작업을 취소할까요?',
            messageType: 'COMMAND_DRAFT',
            structuredPayload: {
              'id': 'draft-1',
              'status': 'DRAFT',
              'commandId': null,
            },
          ),
        ],
      },
    );

    await _pumpChat(tester, gateway);
    await tester.tap(
      find.byKey(const ValueKey('chat-command-draft-reject-draft-1')),
    );
    await tester.pumpAndSettle();

    expect(gateway.rejectedDraftIds, ['draft-1']);
    expect(gateway.messageLoads, {'s1': 1});
    expect(
      find.byKey(const ValueKey('chat-command-draft-status-draft-1')),
      findsOneWidget,
    );
    expect(find.text('REJECTED'), findsOneWidget);
  });

  testWidgets('load more appends the next message page by cursor', (
    tester,
  ) async {
    final messages = [
      for (var index = 0; index < chatMessagePageSize + 1; index += 1)
        _message('m$index', 's1', 'USER', '메시지 $index'),
    ];
    final gateway = _FakeChatGateway(
      sessions: [_session('s1', '첫 대화')],
      messagesBySession: {'s1': messages},
    );

    await _pumpChat(tester, gateway);

    expect(find.text('메시지 ${chatMessagePageSize - 1}'), findsOneWidget);
    expect(find.text('메시지 $chatMessagePageSize'), findsNothing);
    expect(
      find.byKey(const ValueKey('chat-load-more-messages')),
      findsOneWidget,
    );

    await tester.tap(find.byKey(const ValueKey('chat-load-more-messages')));
    await tester.pumpAndSettle();

    expect(find.text('메시지 $chatMessagePageSize'), findsOneWidget);
    expect(find.byKey(const ValueKey('chat-load-more-messages')), findsNothing);
    expect(gateway.messageRequests, [
      {'sessionId': 's1', 'cursor': null, 'limit': chatMessagePageSize},
      {'sessionId': 's1', 'cursor': 'm29', 'limit': chatMessagePageSize},
    ]);
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

  testWidgets('renaming a session patches the session row without reload', (
    tester,
  ) async {
    final gateway = _FakeChatGateway(
      sessions: [_session('s1', '첫 대화')],
      messagesBySession: {
        's1': [_message('m1', 's1', 'USER', '첫 메시지')],
      },
    );

    await _pumpChat(tester, gateway);
    await tester.tap(find.byKey(const ValueKey('chat-rename-session')));
    await tester.pumpAndSettle();
    await tester.enterText(
      find.byKey(const ValueKey('chat-session-title-field')),
      '새 제목',
    );
    await tester.tap(find.byKey(const ValueKey('chat-session-title-save')));
    await tester.pumpAndSettle();

    expect(find.text('새 제목'), findsOneWidget);
    expect(find.text('첫 메시지'), findsOneWidget);
    expect(gateway.updatedTitles, ['s1:새 제목']);
    expect(gateway.messageLoads, {'s1': 1});
  });

  testWidgets('realtime message event fetches only the selected session page', (
    tester,
  ) async {
    final gateway = _FakeChatGateway(
      sessions: [_session('s1', '첫 대화'), _session('s2', '둘째 대화')],
      messagesBySession: {
        's1': [_message('m1', 's1', 'USER', '첫 메시지')],
        's2': [_message('other', 's2', 'USER', '다른 세션 메시지')],
      },
    );

    await _pumpChat(tester, gateway);
    gateway.messagesBySession['s1']!.add(
      _message('m2', 's1', 'ASSISTANT', '새 답장'),
    );
    final container = ProviderScope.containerOf(
      tester.element(find.byType(ChatPage)),
    );
    container
        .read(realtimeChatMessageUpdateProvider.notifier)
        .emit(
          const RealtimeChatMessageUpdate(
            messageId: 'm2',
            sessionId: 's1',
            roomId: 'room-1',
          ),
        );
    await tester.pumpAndSettle();

    expect(find.text('새 답장'), findsOneWidget);
    expect(find.text('다른 세션 메시지'), findsNothing);
    expect(gateway.messageRequests, [
      {'sessionId': 's1', 'cursor': null, 'limit': chatMessagePageSize},
      {'sessionId': 's1', 'cursor': 'm1', 'limit': chatMessagePageSize},
    ]);
  });

  testWidgets(
    'realtime message for another session refreshes only session previews',
    (tester) async {
      final gateway = _FakeChatGateway(
        sessions: [
          _session('s1', 'first session'),
          {..._session('s2', ''), 'messagePreview': 'old preview'},
        ],
        messagesBySession: {
          's1': [_message('m1', 's1', 'USER', 'first message')],
          's2': [_message('other', 's2', 'USER', 'other session message')],
        },
      );

      await _pumpChat(tester, gateway);
      expect(gateway.sessionLoads, 1);
      gateway.sessions[1] = {
        ...gateway.sessions[1],
        'messagePreview': 'new other session preview',
        'updatedAt': '2026-07-14T00:02:00.000Z',
      };

      final container = ProviderScope.containerOf(
        tester.element(find.byType(ChatPage)),
      );
      container
          .read(realtimeChatMessageUpdateProvider.notifier)
          .emit(
            const RealtimeChatMessageUpdate(
              messageId: 'm2',
              sessionId: 's2',
              roomId: 'room-1',
            ),
          );
      await tester.pumpAndSettle();

      expect(gateway.sessionLoads, 2);
      expect(gateway.messageRequests, [
        {'sessionId': 's1', 'cursor': null, 'limit': chatMessagePageSize},
      ]);
      expect(find.text('new other session preview'), findsNothing);

      await tester.tap(find.byKey(const ValueKey('chat-session-picker')));
      await tester.pumpAndSettle();

      expect(find.text('new other session preview'), findsOneWidget);
    },
  );
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
  Map<String, dynamic>? structuredPayload,
}) => {
  'id': id,
  'roomId': 'room-1',
  'sessionId': sessionId,
  'senderType': senderType,
  'messageType': messageType,
  'content': content,
  'structuredPayload': structuredPayload,
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
  final List<Map<String, Object?>> messageRequests = [];
  final List<String> sentMessages = [];
  final List<String> confirmedDraftIds = [];
  final List<String> rejectedDraftIds = [];
  final List<String> updatedTitles = [];
  int createdSessions = 0;
  int sessionLoads = 0;

  @override
  Future<List<Map<String, dynamic>>> listSessions(String roomId) async {
    sessionLoads += 1;
    return [...sessions];
  }

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
  Future<Map<String, dynamic>> updateSession(
    String sessionId,
    String title,
  ) async {
    updatedTitles.add('$sessionId:$title');
    final index = sessions.indexWhere((session) => session['id'] == sessionId);
    if (index < 0) throw StateError('NOT_FOUND');
    final updated = {
      ...sessions[index],
      'title': title,
      'updatedAt': '2026-07-14T00:01:00.000Z',
    };
    sessions[index] = updated;
    return updated;
  }

  @override
  Future<void> deleteSession(String sessionId) async {
    sessions.removeWhere((session) => session['id'] == sessionId);
    messagesBySession.remove(sessionId);
  }

  @override
  Future<List<Map<String, dynamic>>> listMessages(
    String sessionId, {
    String? cursor,
    int limit = chatMessagePageSize,
  }) async {
    messageLoads[sessionId] = (messageLoads[sessionId] ?? 0) + 1;
    messageRequests.add({
      'sessionId': sessionId,
      'cursor': cursor,
      'limit': limit,
    });
    final messages = messagesBySession[sessionId] ?? const [];
    final start = cursor == null
        ? 0
        : messages.indexWhere((message) => message['id'] == cursor) + 1;
    final safeStart = start < 0 ? 0 : start;
    final safeEnd = (safeStart + limit).clamp(safeStart, messages.length);
    return [...messages.sublist(safeStart, safeEnd)];
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

  @override
  Future<Map<String, dynamic>> confirmCommandDraft(
    String draftId,
    String idempotencyKey,
  ) async {
    confirmedDraftIds.add(draftId);
    return {
      'draft': {
        'id': draftId,
        'status': 'MATERIALIZED',
        'commandId': 'command-$draftId',
      },
      'command': {'id': 'command-$draftId'},
    };
  }

  @override
  Future<Map<String, dynamic>> rejectCommandDraft(String draftId) async {
    rejectedDraftIds.add(draftId);
    return {
      'draft': {'id': draftId, 'status': 'REJECTED', 'commandId': null},
    };
  }
}
