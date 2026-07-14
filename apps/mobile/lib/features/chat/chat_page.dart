import 'dart:async';

import 'package:dio/dio.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:uuid/uuid.dart';

import '../../core/network/api_client.dart';
import '../../core/sync/realtime_controller.dart';
import '../proposals/proposal_page.dart';

abstract interface class ChatGateway {
  Future<List<Map<String, dynamic>>> listSessions(String roomId);
  Future<Map<String, dynamic>> createSession(String roomId);
  Future<Map<String, dynamic>> updateSession(String sessionId, String title);
  Future<void> deleteSession(String sessionId);
  Future<Map<String, dynamic>> markSessionRead(
    String sessionId, {
    String? lastReadMessageId,
  });
  Future<List<Map<String, dynamic>>> listMessages(
    String sessionId, {
    String? cursor,
    int limit = chatMessagePageSize,
  });
  Future<Map<String, dynamic>> sendMessage(String sessionId, String content);
  Future<Map<String, dynamic>> confirmCommandDraft(
    String draftId,
    String idempotencyKey,
  );
  Future<Map<String, dynamic>> rejectCommandDraft(String draftId);
  Future<Map<String, dynamic>> confirmRuleDraft(
    String draftId,
    String idempotencyKey,
  );
  Future<Map<String, dynamic>> rejectRuleDraft(String draftId);
  Future<List<Map<String, dynamic>>> listPendingProposals(String roomId);
}

const chatMessagePageSize = 30;
const pendingApprovalSessionId = '__mousekeeper_pending_approvals__';

bool isPendingApprovalSessionId(String? value) =>
    value == pendingApprovalSessionId;

Map<String, dynamic> pendingApprovalSession(int count) => {
  'id': pendingApprovalSessionId,
  'title': count > 0 ? '승인 대기방 ($count)' : '승인 대기방',
  'messagePreview': count > 0 ? '$count개의 제안 검토 필요' : '승인 대기 제안 없음',
  'synthetic': true,
};

final chatGatewayProvider = Provider<ChatGateway>(
  (ref) => ApiChatGateway(ref.watch(apiClientProvider)),
);

final chatSessionListProvider = FutureProvider.autoDispose
    .family<List<Map<String, dynamic>>, String>((ref, roomId) {
      return ref.watch(chatGatewayProvider).listSessions(roomId);
    });

final chatMessagesProvider = FutureProvider.autoDispose
    .family<List<Map<String, dynamic>>, String>((ref, sessionId) {
      return ref.watch(chatGatewayProvider).listMessages(sessionId);
    });

final chatMessagePageProvider = FutureProvider.autoDispose
    .family<List<Map<String, dynamic>>, ChatMessagesPageQuery>((ref, query) {
      return ref
          .watch(chatGatewayProvider)
          .listMessages(
            query.sessionId,
            cursor: query.cursor,
            limit: query.limit,
          );
    });

final chatPendingProposalListProvider = FutureProvider.autoDispose
    .family<List<Map<String, dynamic>>, String>((ref, roomId) {
      return ref.watch(chatGatewayProvider).listPendingProposals(roomId);
    });

class ChatMessagesPageQuery {
  const ChatMessagesPageQuery({
    required this.sessionId,
    required this.cursor,
    this.limit = chatMessagePageSize,
  });

  final String sessionId;
  final String cursor;
  final int limit;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is ChatMessagesPageQuery &&
          other.sessionId == sessionId &&
          other.cursor == cursor &&
          other.limit == limit;

  @override
  int get hashCode => Object.hash(sessionId, cursor, limit);
}

class ApiChatGateway implements ChatGateway {
  ApiChatGateway(this._api);

  final ApiClient _api;

  @override
  Future<List<Map<String, dynamic>>> listSessions(String roomId) =>
      _api.getList('/v1/rooms/$roomId/chat-sessions');

  @override
  Future<Map<String, dynamic>> createSession(String roomId) =>
      _api.post('/v1/rooms/$roomId/chat-sessions', const {});

  @override
  Future<Map<String, dynamic>> updateSession(String sessionId, String title) =>
      _api.patch('/v1/chat-sessions/$sessionId', {'title': title});

  @override
  Future<void> deleteSession(String sessionId) async {
    await _api.delete('/v1/chat-sessions/$sessionId');
  }

  @override
  Future<Map<String, dynamic>> markSessionRead(
    String sessionId, {
    String? lastReadMessageId,
  }) => _api.post('/v1/chat-sessions/$sessionId/read', {
    'lastReadMessageId': ?lastReadMessageId,
  });

  @override
  Future<List<Map<String, dynamic>>> listMessages(
    String sessionId, {
    String? cursor,
    int limit = chatMessagePageSize,
  }) {
    final path = Uri(
      path: '/v1/chat-sessions/$sessionId/messages',
      queryParameters: {'limit': '$limit', 'cursor': ?cursor},
    ).toString();
    return _api.getList(path);
  }

  @override
  Future<Map<String, dynamic>> sendMessage(String sessionId, String content) =>
      _api.post('/v1/chat-sessions/$sessionId/messages', {'content': content});

  @override
  Future<Map<String, dynamic>> confirmCommandDraft(
    String draftId,
    String idempotencyKey,
  ) => _api.post(
    '/v1/command-drafts/$draftId/confirm',
    const {},
    idempotencyKey: idempotencyKey,
  );

  @override
  Future<Map<String, dynamic>> rejectCommandDraft(String draftId) =>
      _api.post('/v1/command-drafts/$draftId/reject', const {});

  @override
  Future<Map<String, dynamic>> confirmRuleDraft(
    String draftId,
    String idempotencyKey,
  ) => _api.post(
    '/v1/rule-drafts/$draftId/confirm',
    const {},
    idempotencyKey: idempotencyKey,
  );

  @override
  Future<Map<String, dynamic>> rejectRuleDraft(String draftId) =>
      _api.post('/v1/rule-drafts/$draftId/reject', const {});

  @override
  Future<List<Map<String, dynamic>>> listPendingProposals(String roomId) =>
      _api.getList('/v1/rooms/$roomId/proposals/open');
}

String chatSessionTitle(Map<String, dynamic> session) {
  final title = (session['title'] as String?)?.trim();
  if (title != null && title.isNotEmpty) return title;
  final preview = (session['messagePreview'] as String?)?.trim();
  if (preview != null && preview.isNotEmpty) return preview;
  return '새 대화';
}

String chatSendNotice(Map<String, dynamic> result) {
  final status = result['aiStatus'];
  final ai = result['ai'];
  if (status == 'UNCONFIGURED') {
    return 'AI가 아직 설정되지 않아 메시지만 저장했습니다.';
  }
  if (status == 'INVALID') {
    return 'AI 응답이 계약 검증을 통과하지 못해 실행하지 않았습니다.';
  }
  if (ai is Map) {
    return switch (ai['kind']) {
      'COMMAND_DRAFT' => '확인 카드가 생성되었습니다. 내용을 보고 수락 또는 거절해 주세요.',
      'RULE_DRAFT' => '정리 규칙 초안이 생성되었습니다. 내용을 보고 수락 또는 거절해 주세요.',
      'QUERY' => 'AI가 파일 조회 요청을 만들었습니다. 결과가 채팅에 표시됩니다.',
      _ => '',
    };
  }
  return '';
}

String chatErrorMessage(Object error) {
  if (error is DioException) {
    final data = error.response?.data;
    if (data is Map && data['code'] == 'CHAT_SESSION_LIMIT_REACHED') {
      return '대화는 최대 5개까지 만들 수 있습니다. 기존 대화를 삭제한 뒤 다시 시도하세요.';
    }
  }
  final raw = error.toString();
  if (raw.contains('CHAT_SESSION_LIMIT_REACHED')) {
    return '대화는 최대 5개까지 만들 수 있습니다. 기존 대화를 삭제한 뒤 다시 시도하세요.';
  }
  return '채팅 작업을 완료하지 못했습니다.';
}

bool isActionDraftMessage(Map<String, dynamic> message) =>
    message['messageType'] == 'COMMAND_DRAFT' ||
    message['messageType'] == 'RULE_DRAFT';

bool isRuleDraftMessage(Map<String, dynamic> message) =>
    message['messageType'] == 'RULE_DRAFT';

String? actionDraftIdFromMessage(Map<String, dynamic> message) {
  if (!isActionDraftMessage(message)) return null;
  final payload = message['structuredPayload'];
  if (payload is Map && payload['id'] is String) {
    return payload['id'] as String;
  }
  return null;
}

String? commandDraftIdFromMessage(Map<String, dynamic> message) {
  if (message['messageType'] != 'COMMAND_DRAFT') return null;
  final payload = message['structuredPayload'];
  if (payload is Map && payload['id'] is String) {
    return payload['id'] as String;
  }
  return null;
}

String actionDraftStatusFromMessage(Map<String, dynamic> message) {
  final payload = message['structuredPayload'];
  if (payload is Map && payload['status'] is String) {
    return payload['status'] as String;
  }
  return 'DRAFT';
}

String commandDraftStatusFromMessage(Map<String, dynamic> message) =>
    actionDraftStatusFromMessage(message);

String? selectChatSessionId(
  List<Map<String, dynamic>> sessions,
  String? preferredId,
) {
  if (preferredId != null) {
    for (final session in sessions) {
      if (session['id'] == preferredId) return preferredId;
    }
  }
  if (sessions.isEmpty) return null;
  return sessions.first['id'] as String?;
}

String? lastChatMessageId(List<Map<String, dynamic>> messages) {
  if (messages.isEmpty) return null;
  return messages.last['id'] as String?;
}

List<Map<String, dynamic>> touchChatSessionPreview(
  List<Map<String, dynamic>> sessions,
  String sessionId,
  String preview,
) {
  var changed = false;
  final next = [
    for (final session in sessions)
      if (session['id'] == sessionId && session['messagePreview'] != preview)
        (() {
          changed = true;
          return {...session, 'messagePreview': preview};
        })()
      else
        session,
  ];
  return changed ? List.unmodifiable(next) : sessions;
}

List<Map<String, dynamic>> replaceChatSession(
  List<Map<String, dynamic>> sessions,
  Map<String, dynamic> updated,
) {
  final updatedId = updated['id'];
  if (updatedId is! String) return sessions;
  var changed = false;
  var found = false;
  final next = [
    for (final session in sessions)
      if (session['id'] == updatedId)
        (() {
          found = true;
          if (_jsonEquivalent(session, updated)) return session;
          changed = true;
          return updated;
        })()
      else
        session,
  ];
  return found && changed ? List.unmodifiable(next) : sessions;
}

List<Map<String, dynamic>> _replacePendingApprovalSession(
  List<Map<String, dynamic>> sessions,
  int pendingCount,
) {
  final nextPending = pendingApprovalSession(pendingCount);
  var replaced = false;
  final next = [
    for (final session in sessions)
      if (isPendingApprovalSessionId(session['id'] as String?))
        (() {
          replaced = true;
          return nextPending;
        })()
      else
        session,
  ];
  if (!replaced) next.add(nextPending);
  return List.unmodifiable(next);
}

List<Map<String, dynamic>> mergeChatMessages(
  List<Map<String, dynamic>> existing,
  List<Map<String, dynamic>> received,
) {
  if (received.isEmpty) return existing;
  final seen = existing
      .map((message) => message['id'])
      .whereType<String>()
      .toSet();
  var changed = false;
  final next = <Map<String, dynamic>>[...existing];
  for (final message in received) {
    final id = message['id'];
    if (id is String && seen.add(id)) {
      next.add(message);
      changed = true;
    }
  }
  return changed ? List.unmodifiable(next) : existing;
}

Map<String, dynamic> optimisticUserChatMessage({
  required String sessionId,
  required String content,
}) => {
  'id': 'local-${const Uuid().v4()}',
  'roomId': null,
  'sessionId': sessionId,
  'senderType': 'USER',
  'messageType': 'TEXT',
  'content': content,
  'structuredPayload': null,
  'commandId': null,
  'createdAt': DateTime.now().toUtc().toIso8601String(),
};

List<Map<String, dynamic>> removeChatMessageById(
  List<Map<String, dynamic>> messages,
  String messageId,
) {
  var changed = false;
  final next = <Map<String, dynamic>>[];
  for (final message in messages) {
    if (message['id'] == messageId) {
      changed = true;
      continue;
    }
    next.add(message);
  }
  return changed ? List.unmodifiable(next) : messages;
}

List<Map<String, dynamic>> reconcileOptimisticChatMessage({
  required List<Map<String, dynamic>> messages,
  required String optimisticId,
  required Map<String, dynamic> authoritative,
}) {
  final authoritativeId = authoritative['id'];
  if (authoritativeId is String &&
      messages.any(
        (message) =>
            message['id'] == authoritativeId && message['id'] != optimisticId,
      )) {
    return removeChatMessageById(messages, optimisticId);
  }

  var changed = false;
  var replaced = false;
  final next = [
    for (final message in messages)
      if (message['id'] == optimisticId)
        (() {
          changed = true;
          replaced = true;
          return authoritative;
        })()
      else
        message,
  ];
  if (!replaced) {
    return mergeChatMessages(messages, [authoritative]);
  }
  return changed ? List.unmodifiable(next) : messages;
}

List<Map<String, dynamic>> patchActionDraftMessages(
  List<Map<String, dynamic>> messages,
  String draftId,
  Object? draft,
) {
  if (draft is! Map) return messages;
  final nextPayload = Map<String, dynamic>.from(draft);
  var changed = false;
  final next = [
    for (final message in messages)
      if (actionDraftIdFromMessage(message) == draftId)
        (() {
          if (_jsonEquivalent(message['structuredPayload'], nextPayload)) {
            return message;
          }
          changed = true;
          return {...message, 'structuredPayload': nextPayload};
        })()
      else
        message,
  ];
  return changed ? List.unmodifiable(next) : messages;
}

List<Map<String, dynamic>> patchCommandDraftMessages(
  List<Map<String, dynamic>> messages,
  String draftId,
  Object? draft,
) => patchActionDraftMessages(messages, draftId, draft);

bool _jsonEquivalent(Object? left, Object? right) {
  if (identical(left, right)) return true;
  if (left is Map && right is Map) {
    if (left.length != right.length) return false;
    for (final entry in left.entries) {
      if (!right.containsKey(entry.key) ||
          !_jsonEquivalent(entry.value, right[entry.key])) {
        return false;
      }
    }
    return true;
  }
  if (left is List && right is List) {
    if (left.length != right.length) return false;
    for (var index = 0; index < left.length; index++) {
      if (!_jsonEquivalent(left[index], right[index])) return false;
    }
    return true;
  }
  return left == right;
}

class ChatConversationState {
  ChatConversationState({
    required List<Map<String, dynamic>> sessions,
    required List<Map<String, dynamic>> messages,
    required this.selectedSessionId,
    required this.messageCursor,
    required this.hasMoreMessages,
  }) : sessions = List.unmodifiable(sessions),
       messages = List.unmodifiable(messages);

  const ChatConversationState.empty()
    : sessions = const [],
      messages = const [],
      selectedSessionId = null,
      messageCursor = null,
      hasMoreMessages = false;

  final List<Map<String, dynamic>> sessions;
  final List<Map<String, dynamic>> messages;
  final String? selectedSessionId;
  final String? messageCursor;
  final bool hasMoreMessages;

  ChatConversationState copyWith({
    List<Map<String, dynamic>>? sessions,
    List<Map<String, dynamic>>? messages,
    String? selectedSessionId,
    bool clearSelectedSessionId = false,
    String? messageCursor,
    bool clearMessageCursor = false,
    bool? hasMoreMessages,
  }) => ChatConversationState(
    sessions: sessions ?? this.sessions,
    messages: messages ?? this.messages,
    selectedSessionId: clearSelectedSessionId
        ? null
        : selectedSessionId ?? this.selectedSessionId,
    messageCursor: clearMessageCursor
        ? null
        : messageCursor ?? this.messageCursor,
    hasMoreMessages: hasMoreMessages ?? this.hasMoreMessages,
  );

  ChatConversationState withLoadedMessages(
    List<Map<String, dynamic>> messages,
  ) => copyWith(
    messages: messages,
    messageCursor: lastChatMessageId(messages),
    clearMessageCursor: messages.isEmpty,
    hasMoreMessages: messages.length == chatMessagePageSize,
  );
}

class ChatPage extends ConsumerStatefulWidget {
  const ChatPage({super.key, required this.roomId, this.gateway});

  final String roomId;
  final ChatGateway? gateway;

  @override
  ConsumerState<ChatPage> createState() => _ChatPageState();
}

class _ChatPageState extends ConsumerState<ChatPage> {
  final input = TextEditingController();
  final _scrollController = ScrollController();
  ChatConversationState _conversation = const ChatConversationState.empty();
  List<Map<String, dynamic>> _pendingProposals = const [];
  Object? _error;
  bool _loading = true;
  bool _sending = false;
  bool _loadingMore = false;
  bool _creating = false;
  bool _renaming = false;
  bool _deleting = false;
  bool _disposed = false;
  final Set<String> _draftingIds = {};
  final Set<String> _realtimeMessageIdsInFlight = {};
  bool _realtimeSessionRefreshInFlight = false;
  int _loadVersion = 0;

  ChatGateway get _gateway => widget.gateway ?? ref.read(chatGatewayProvider);

  Future<List<Map<String, dynamic>>> _listSessions() {
    final gateway = widget.gateway;
    if (gateway != null) return gateway.listSessions(widget.roomId);
    return ref.read(chatSessionListProvider(widget.roomId).future);
  }

  Future<List<Map<String, dynamic>>> _listPendingProposals() {
    final gateway = widget.gateway;
    if (gateway != null) return gateway.listPendingProposals(widget.roomId);
    return ref.read(chatPendingProposalListProvider(widget.roomId).future);
  }

  Future<List<Map<String, dynamic>>> _listInitialMessages(String sessionId) {
    final gateway = widget.gateway;
    if (gateway != null) return gateway.listMessages(sessionId);
    return ref.read(chatMessagesProvider(sessionId).future);
  }

  Future<List<Map<String, dynamic>>> _listMoreMessages(
    String sessionId,
    String cursor,
  ) {
    final gateway = widget.gateway;
    if (gateway != null) {
      return gateway.listMessages(sessionId, cursor: cursor);
    }
    return ref.read(
      chatMessagePageProvider(
        ChatMessagesPageQuery(sessionId: sessionId, cursor: cursor),
      ).future,
    );
  }

  void _invalidateChatReadModels({String? sessionId}) {
    if (widget.gateway != null) return;
    ref.invalidate(chatSessionListProvider(widget.roomId));
    ref.invalidate(chatPendingProposalListProvider(widget.roomId));
    if (sessionId != null) {
      ref.invalidate(chatMessagesProvider(sessionId));
    }
  }

  @override
  void initState() {
    super.initState();
    unawaited(_loadInitial());
  }

  @override
  void dispose() {
    _disposed = true;
    _loadVersion++;
    input.dispose();
    _scrollController.dispose();
    super.dispose();
  }

  Future<void> _loadInitial() async {
    final version = ++_loadVersion;
    setState(() {
      _loading = true;
      _error = null;
    });
    try {
      var sessions = await _listSessions();
      final pendingProposals = await _listPendingProposals();
      if (_stale(version)) return;
      if (sessions.isEmpty) {
        final created = await _gateway.createSession(widget.roomId);
        _invalidateChatReadModels(sessionId: created['id'] as String?);
        if (_stale(version)) return;
        sessions = [created];
      }
      final visibleSessions = [
        ...sessions,
        pendingApprovalSession(pendingProposals.length),
      ];
      final selectedId = selectChatSessionId(
        visibleSessions,
        _conversation.selectedSessionId,
      );
      final messages =
          selectedId == null || isPendingApprovalSessionId(selectedId)
          ? <Map<String, dynamic>>[]
          : await _listInitialMessages(selectedId);
      if (_stale(version)) return;
      setState(() {
        _pendingProposals = pendingProposals;
        _conversation = ChatConversationState(
          sessions: visibleSessions,
          messages: messages,
          selectedSessionId: selectedId,
          messageCursor: lastChatMessageId(messages),
          hasMoreMessages: messages.length == chatMessagePageSize,
        );
        _loading = false;
      });
      unawaited(_markVisibleSessionRead());
      _scrollToBottom();
    } catch (error) {
      if (_stale(version)) return;
      setState(() {
        _error = error;
        _loading = false;
      });
    }
  }

  Future<void> _selectSession(String sessionId) async {
    final version = ++_loadVersion;
    setState(() {
      _conversation = _conversation.copyWith(
        selectedSessionId: sessionId,
        messages: const [],
        clearMessageCursor: true,
        hasMoreMessages: false,
      );
      _loading = true;
      _error = null;
    });
    try {
      if (isPendingApprovalSessionId(sessionId)) {
        final pendingProposals = await _listPendingProposals();
        if (_stale(version)) return;
        setState(() {
          _pendingProposals = pendingProposals;
          _conversation = _conversation.copyWith(
            sessions: _replacePendingApprovalSession(
              _conversation.sessions,
              pendingProposals.length,
            ),
            messages: const [],
            clearMessageCursor: true,
            hasMoreMessages: false,
          );
          _loading = false;
        });
        return;
      }
      final messages = await _listInitialMessages(sessionId);
      if (_stale(version)) return;
      setState(() {
        _conversation = _conversation.withLoadedMessages(messages);
        _loading = false;
      });
      unawaited(_markVisibleSessionRead());
      _scrollToBottom();
    } catch (error) {
      if (_stale(version)) return;
      setState(() {
        _error = error;
        _loading = false;
      });
    }
  }

  Future<void> _createSession() async {
    if (_creating) return;
    setState(() => _creating = true);
    try {
      final created = await _gateway.createSession(widget.roomId);
      if (_disposed) return;
      _invalidateChatReadModels(sessionId: created['id'] as String?);
      setState(() {
        _conversation = _conversation.copyWith(
          sessions: [created, ..._conversation.sessions],
          selectedSessionId: created['id'] as String,
          messages: const [],
          clearMessageCursor: true,
          hasMoreMessages: false,
        );
        _error = null;
      });
    } catch (error) {
      _showSnack(chatErrorMessage(error));
    } finally {
      if (!_disposed) setState(() => _creating = false);
    }
  }

  Future<void> _renameSelectedSession() async {
    final sessionId = _conversation.selectedSessionId;
    if (sessionId == null ||
        isPendingApprovalSessionId(sessionId) ||
        _renaming) {
      return;
    }
    final current = _conversation.sessions
        .where((session) => session['id'] == sessionId)
        .cast<Map<String, dynamic>>()
        .firstOrNull;
    if (current == null) return;
    final title = await _promptSessionTitle(current);
    if (title == null || title == chatSessionTitle(current)) return;
    if (_disposed) return;
    setState(() => _renaming = true);
    try {
      final updated = await _gateway.updateSession(sessionId, title);
      if (_disposed) return;
      _invalidateChatReadModels(sessionId: sessionId);
      setState(() {
        _conversation = _conversation.copyWith(
          sessions: replaceChatSession(_conversation.sessions, updated),
        );
        _error = null;
      });
    } catch (error) {
      _showSnack('대화 제목을 바꾸지 못했습니다: ${chatErrorMessage(error)}');
    } finally {
      if (!_disposed) setState(() => _renaming = false);
    }
  }

  Future<String?> _promptSessionTitle(Map<String, dynamic> session) async {
    return showDialog<String>(
      context: context,
      builder: (context) =>
          _ChatSessionTitleDialog(initialTitle: chatSessionTitle(session)),
    );
  }

  Future<void> _deleteSelectedSession() async {
    final sessionId = _conversation.selectedSessionId;
    if (sessionId == null ||
        isPendingApprovalSessionId(sessionId) ||
        _deleting) {
      return;
    }
    setState(() => _deleting = true);
    try {
      await _gateway.deleteSession(sessionId);
      if (_disposed) return;
      _invalidateChatReadModels(sessionId: sessionId);
      final remaining = _conversation.sessions
          .where((session) => session['id'] != sessionId)
          .toList(growable: false);
      final nextId = selectChatSessionId(remaining, null);
      setState(() {
        _conversation = _conversation.copyWith(
          sessions: remaining,
          selectedSessionId: nextId,
          clearSelectedSessionId: nextId == null,
          messages: const [],
          clearMessageCursor: true,
          hasMoreMessages: false,
        );
      });
      if (nextId != null) {
        await _selectSession(nextId);
      }
    } catch (error) {
      _showSnack(chatErrorMessage(error));
    } finally {
      if (!_disposed) setState(() => _deleting = false);
    }
  }

  Future<void> send() async {
    final sessionId = _conversation.selectedSessionId;
    final content = input.text.trim();
    if (sessionId == null ||
        isPendingApprovalSessionId(sessionId) ||
        content.isEmpty ||
        _sending) {
      return;
    }
    final optimistic = optimisticUserChatMessage(
      sessionId: sessionId,
      content: content,
    );
    final optimisticId = optimistic['id'] as String;
    input.clear();
    setState(() {
      _sending = true;
      _conversation = _conversation.copyWith(
        messages: [..._conversation.messages, optimistic],
        sessions: touchChatSessionPreview(
          _conversation.sessions,
          sessionId,
          content,
        ),
      );
    });
    _scrollToBottom();
    try {
      final result = await _gateway.sendMessage(sessionId, content);
      if (_disposed) return;
      _invalidateChatReadModels(sessionId: sessionId);
      var newMessages = _conversation.messages;
      final resultMessage = result['message'];
      if (resultMessage is Map<String, dynamic>) {
        newMessages = reconcileOptimisticChatMessage(
          messages: newMessages,
          optimisticId: optimisticId,
          authoritative: resultMessage,
        );
      } else {
        newMessages = removeChatMessageById(newMessages, optimisticId);
      }
      final assistant = result['assistant'];
      if (assistant is Map<String, dynamic>) {
        newMessages = mergeChatMessages(newMessages, [assistant]);
      }
      setState(() {
        _conversation = _conversation.copyWith(
          messages: newMessages,
          messageCursor: lastChatMessageId(newMessages),
          clearMessageCursor: newMessages.isEmpty,
        );
      });
      unawaited(_markVisibleSessionRead());
      _scrollToBottom();
      final notice = chatSendNotice(result);
      if (notice.isNotEmpty) _showSnack(notice);
      if (assistant is Map<String, dynamic>) {
        if (isActionDraftMessage(assistant)) {
          unawaited(_refreshPendingProposals());
        }
      }
    } catch (error) {
      if (!_disposed) {
        final failedMessages = removeChatMessageById(
          _conversation.messages,
          optimisticId,
        );
        setState(() {
          _conversation = _conversation.copyWith(
            messages: failedMessages,
            messageCursor: lastChatMessageId(failedMessages),
            clearMessageCursor: failedMessages.isEmpty,
          );
          input.text = content;
          input.selection = TextSelection.collapsed(offset: input.text.length);
        });
      }
      _showSnack('메시지 저장 실패: ${chatErrorMessage(error)}');
    } finally {
      if (!_disposed) setState(() => _sending = false);
    }
  }

  Future<void> _loadMoreMessages() async {
    final sessionId = _conversation.selectedSessionId;
    final cursor = _conversation.messageCursor;
    if (sessionId == null ||
        isPendingApprovalSessionId(sessionId) ||
        cursor == null ||
        _loadingMore) {
      return;
    }
    setState(() => _loadingMore = true);
    try {
      final next = await _listMoreMessages(sessionId, cursor);
      if (_disposed || sessionId != _conversation.selectedSessionId) return;
      final merged = mergeChatMessages(_conversation.messages, next);
      setState(() {
        _conversation = _conversation.copyWith(
          messages: merged,
          messageCursor: lastChatMessageId(merged),
          clearMessageCursor: merged.isEmpty,
          hasMoreMessages: next.length == chatMessagePageSize,
        );
      });
      unawaited(_markVisibleSessionRead());
      _scrollToBottom();
    } catch (error) {
      _showSnack('메시지를 더 불러오지 못했습니다: ${chatErrorMessage(error)}');
    } finally {
      if (!_disposed) setState(() => _loadingMore = false);
    }
  }

  Future<void> _applyRealtimeChatMessage(
    RealtimeChatMessageUpdate update,
  ) async {
    if (update.roomId != null && update.roomId != widget.roomId) return;
    unawaited(_refreshSessionsForRealtime(update));
    if (update.sessionId != _conversation.selectedSessionId) {
      return;
    }
    if (_conversation.messages.any(
      (message) => message['id'] == update.messageId,
    )) {
      return;
    }
    if (!_realtimeMessageIdsInFlight.add(update.messageId)) return;
    try {
      final sessionId = update.sessionId;
      final cursor = _conversation.messageCursor;
      final received = cursor == null
          ? await _listInitialMessages(sessionId)
          : await _listMoreMessages(sessionId, cursor);
      if (_disposed || sessionId != _conversation.selectedSessionId) return;
      final merged = mergeChatMessages(_conversation.messages, received);
      if (identical(merged, _conversation.messages)) return;
      setState(() {
        _conversation = _conversation.copyWith(
          messages: merged,
          messageCursor: lastChatMessageId(merged),
          clearMessageCursor: merged.isEmpty,
          hasMoreMessages: received.length == chatMessagePageSize,
        );
      });
      unawaited(_markVisibleSessionRead());
      _scrollToBottom();
    } catch (_) {
      // Realtime chat events are a latency optimization. If this targeted
      // fetch fails, the next explicit open/refresh still reads the durable
      // session messages from the server.
    } finally {
      _realtimeMessageIdsInFlight.remove(update.messageId);
    }
  }

  Future<void> _refreshSessionsForRealtime(
    RealtimeChatMessageUpdate update,
  ) async {
    await _refreshSessionsForRealtimeRoom(update.roomId);
  }

  Future<void> _refreshSessionsForRealtimeRoom(String? roomId) async {
    if (roomId != null && roomId != widget.roomId) return;
    if (_realtimeSessionRefreshInFlight) return;
    _realtimeSessionRefreshInFlight = true;
    try {
      _invalidateChatReadModels();
      final sessions = await _listSessions();
      final pendingProposals = await _listPendingProposals();
      final visibleSessions = [
        ...sessions,
        pendingApprovalSession(pendingProposals.length),
      ];
      if (_disposed) return;
      final selectedId = selectChatSessionId(
        visibleSessions,
        _conversation.selectedSessionId,
      );
      final selectedStillAvailable =
          selectedId != null && selectedId == _conversation.selectedSessionId;
      setState(() {
        _pendingProposals = pendingProposals;
        _conversation = _conversation.copyWith(
          sessions: visibleSessions,
          selectedSessionId: selectedId,
          clearSelectedSessionId: selectedId == null,
          messages: selectedStillAvailable ? null : const [],
          clearMessageCursor: !selectedStillAvailable,
          hasMoreMessages: selectedStillAvailable ? null : false,
        );
      });
    } catch (_) {
      // Session previews/order are an optimization over the durable chat
      // store. A failed lightweight refresh must not reload messages or
      // replace the visible conversation with an error page.
    } finally {
      _realtimeSessionRefreshInFlight = false;
    }
  }

  Future<void> _markVisibleSessionRead() async {
    final sessionId = _conversation.selectedSessionId;
    if (sessionId == null) return;
    try {
      final updated = await _gateway.markSessionRead(
        sessionId,
        lastReadMessageId: lastChatMessageId(_conversation.messages),
      );
      if (_disposed || sessionId != _conversation.selectedSessionId) return;
      _invalidateChatReadModels(sessionId: sessionId);
      setState(() {
        _conversation = _conversation.copyWith(
          sessions: replaceChatSession(_conversation.sessions, updated),
        );
      });
    } catch (_) {
      // Read state only drives shared badges/notifications. Message display is
      // still backed by durable server rows, and the next session refresh can
      // repair unread counts without hiding the visible conversation.
    }
  }

  Future<void> _confirmCommandDraft(String draftId) async {
    await _confirmActionDraft(
      draftId,
      () => _gateway.confirmCommandDraft(draftId, const Uuid().v4()),
      'Command draft confirmed.',
      'Command draft confirmation failed',
    );
  }

  Future<void> _rejectCommandDraft(String draftId) async {
    await _rejectActionDraft(
      draftId,
      () => _gateway.rejectCommandDraft(draftId),
      'Command draft rejected.',
      'Command draft rejection failed',
    );
  }

  Future<void> _confirmRuleDraft(String draftId) async {
    await _confirmActionDraft(
      draftId,
      () => _gateway.confirmRuleDraft(draftId, const Uuid().v4()),
      'Rule draft confirmed.',
      'Rule draft confirmation failed',
    );
  }

  Future<void> _rejectRuleDraft(String draftId) async {
    await _rejectActionDraft(
      draftId,
      () => _gateway.rejectRuleDraft(draftId),
      'Rule draft rejected.',
      'Rule draft rejection failed',
    );
  }

  Future<void> _confirmActionDraft(
    String draftId,
    Future<Map<String, dynamic>> Function() action,
    String successMessage,
    String failurePrefix,
  ) async {
    if (_draftingIds.contains(draftId)) return;
    setState(() => _draftingIds.add(draftId));
    try {
      final result = await action();
      if (_disposed) return;
      _patchDraftMessage(draftId, result['draft']);
      unawaited(_refreshPendingProposals());
      _invalidateChatReadModels(sessionId: _conversation.selectedSessionId);
      _showSnack(successMessage);
    } catch (error) {
      _showSnack('$failurePrefix: ${chatErrorMessage(error)}');
    } finally {
      if (!_disposed) setState(() => _draftingIds.remove(draftId));
    }
  }

  Future<void> _rejectActionDraft(
    String draftId,
    Future<Map<String, dynamic>> Function() action,
    String successMessage,
    String failurePrefix,
  ) => _confirmActionDraft(draftId, action, successMessage, failurePrefix);

  void _patchDraftMessage(String draftId, Object? draft) {
    final next = patchActionDraftMessages(
      _conversation.messages,
      draftId,
      draft,
    );
    if (identical(next, _conversation.messages)) return;
    setState(() {
      _conversation = _conversation.copyWith(messages: next);
    });
  }

  Future<void> _refreshPendingProposals() async {
    try {
      final pendingProposals = await _listPendingProposals();
      if (_disposed) return;
      setState(() {
        _pendingProposals = pendingProposals;
        _conversation = _conversation.copyWith(
          sessions: _replacePendingApprovalSession(
            _conversation.sessions,
            pendingProposals.length,
          ),
        );
      });
    } catch (_) {
      // Pending proposal refresh is a UI badge update. The durable command
      // draft mutation above already completed or failed explicitly.
    }
  }

  bool _stale(int version) => _disposed || version != _loadVersion;

  void _showSnack(String message) {
    if (!_disposed && mounted) {
      ScaffoldMessenger.of(
        context,
      ).showSnackBar(SnackBar(content: Text(message)));
    }
  }

  void _scrollToBottom() {
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!_scrollController.hasClients) return;
      _scrollController.jumpTo(_scrollController.position.maxScrollExtent);
    });
  }

  @override
  Widget build(BuildContext context) {
    ref.listen(realtimeChatMessageUpdateProvider, (previous, next) {
      if (next == null || identical(previous, next) || !mounted) return;
      unawaited(_applyRealtimeChatMessage(next));
    });
    ref.listen(realtimeChatSessionUpdateProvider, (previous, next) {
      if (next == null || identical(previous, next) || !mounted) return;
      unawaited(_refreshSessionsForRealtimeRoom(next.roomId));
    });
    final selectedIsPendingApproval = isPendingApprovalSessionId(
      _conversation.selectedSessionId,
    );
    return Scaffold(
      appBar: AppBar(title: const Text('집쥐인과 대화')),
      body: Column(
        children: [
          _SessionBar(
            sessions: _conversation.sessions,
            selectedSessionId: _conversation.selectedSessionId,
            pendingApprovalSelected: selectedIsPendingApproval,
            creating: _creating,
            renaming: _renaming,
            deleting: _deleting,
            onCreate: _createSession,
            onRename: _renameSelectedSession,
            onDelete: _deleteSelectedSession,
            onSelected: (id) {
              if (id != _conversation.selectedSessionId) {
                unawaited(_selectSession(id));
              }
            },
          ),
          Expanded(child: _buildBody()),
          SafeArea(
            child: Padding(
              padding: const EdgeInsets.all(12),
              child: Row(
                children: [
                  Expanded(
                    child: TextField(
                      key: const ValueKey('chat-message-field'),
                      controller: input,
                      maxLength: 2000,
                      enabled:
                          _conversation.selectedSessionId != null &&
                          !selectedIsPendingApproval,
                      decoration: const InputDecoration(
                        hintText: 'AI에게 파일 정리 요청을 말로 입력하세요',
                        border: OutlineInputBorder(),
                      ),
                      onSubmitted: (_) => unawaited(send()),
                    ),
                  ),
                  const SizedBox(width: 8),
                  IconButton(
                    key: const ValueKey('chat-send-button'),
                    tooltip: '보내기',
                    onPressed:
                        _sending ||
                            selectedIsPendingApproval ||
                            _conversation.selectedSessionId == null
                        ? null
                        : send,
                    icon: const Icon(Icons.send),
                  ),
                ],
              ),
            ),
          ),
        ],
      ),
    );
  }

  Widget _buildBody() {
    if (_loading) return const Center(child: CircularProgressIndicator());
    if (_error != null) {
      return Center(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Text(
              '채팅을 불러오지 못했습니다.\n${chatErrorMessage(_error!)}',
              textAlign: TextAlign.center,
            ),
            const SizedBox(height: 12),
            FilledButton(
              onPressed: () => unawaited(_loadInitial()),
              child: const Text('다시 시도'),
            ),
          ],
        ),
      );
    }
    if (_conversation.selectedSessionId == null) {
      return const Center(
        child: Text('대화가 없습니다.\n새 대화를 만들어 주세요.', textAlign: TextAlign.center),
      );
    }
    if (isPendingApprovalSessionId(_conversation.selectedSessionId)) {
      return _PendingApprovalRoom(
        proposals: _pendingProposals,
        onRefresh: () => unawaited(_refreshPendingProposals()),
        onOpenProposal: (proposalId) async {
          await Navigator.of(context).push(
            MaterialPageRoute(
              builder: (_) =>
                  ProposalPage(proposalId: proposalId, roomId: widget.roomId),
            ),
          );
          if (!_disposed) unawaited(_refreshPendingProposals());
        },
      );
    }
    if (_conversation.messages.isEmpty) {
      return const Center(
        child: Text(
          '아직 메시지가 없습니다.\n파일 목록, 히스토리, 정리 요청을 말로 물어보세요.',
          textAlign: TextAlign.center,
        ),
      );
    }
    return Column(
      children: [
        Expanded(
          child: ListView.builder(
            controller: _scrollController,
            padding: const EdgeInsets.all(16),
            itemCount:
                _conversation.messages.length +
                (_conversation.hasMoreMessages ? 1 : 0),
            itemBuilder: (context, index) {
              if (index == _conversation.messages.length) {
                return Padding(
                  padding: const EdgeInsets.symmetric(vertical: 8),
                  child: OutlinedButton.icon(
                    key: const ValueKey('chat-load-more-messages'),
                    onPressed: _loadingMore
                        ? null
                        : () => unawaited(_loadMoreMessages()),
                    icon: _loadingMore
                        ? const SizedBox.square(
                            dimension: 16,
                            child: CircularProgressIndicator(strokeWidth: 2),
                          )
                        : const Icon(Icons.expand_more),
                    label: const Text('다음 메시지 더 보기'),
                  ),
                );
              }
              return _ChatBubble(
                message: _conversation.messages[index],
                busyDraftIds: _draftingIds,
                onConfirmDraft: (draftId) =>
                    unawaited(_confirmCommandDraft(draftId)),
                onRejectDraft: (draftId) =>
                    unawaited(_rejectCommandDraft(draftId)),
                onConfirmRuleDraft: (draftId) =>
                    unawaited(_confirmRuleDraft(draftId)),
                onRejectRuleDraft: (draftId) =>
                    unawaited(_rejectRuleDraft(draftId)),
              );
            },
          ),
        ),
        if (_sending)
          const Padding(
            padding: EdgeInsets.fromLTRB(16, 4, 16, 8),
            child: Align(
              alignment: Alignment.centerLeft,
              child: Text('답을 만들고 있어요!', key: ValueKey('chat-answer-pending')),
            ),
          ),
      ],
    );
  }
}

class _ChatSessionTitleDialog extends StatefulWidget {
  const _ChatSessionTitleDialog({required this.initialTitle});

  final String initialTitle;

  @override
  State<_ChatSessionTitleDialog> createState() =>
      _ChatSessionTitleDialogState();
}

class _ChatSessionTitleDialogState extends State<_ChatSessionTitleDialog> {
  late final TextEditingController controller;

  @override
  void initState() {
    super.initState();
    controller = TextEditingController(text: widget.initialTitle);
  }

  @override
  void dispose() {
    controller.dispose();
    super.dispose();
  }

  void _submit() {
    final title = controller.text.trim();
    if (title.isNotEmpty) Navigator.of(context).pop(title);
  }

  @override
  Widget build(BuildContext context) => AlertDialog(
    title: const Text('대화 제목 수정'),
    content: TextField(
      key: const ValueKey('chat-session-title-field'),
      controller: controller,
      autofocus: true,
      maxLength: 120,
      decoration: const InputDecoration(
        labelText: '제목',
        border: OutlineInputBorder(),
      ),
      onSubmitted: (_) => _submit(),
    ),
    actions: [
      TextButton(
        onPressed: () => Navigator.of(context).pop(),
        child: const Text('취소'),
      ),
      FilledButton(
        key: const ValueKey('chat-session-title-save'),
        onPressed: _submit,
        child: const Text('저장'),
      ),
    ],
  );
}

class _PendingApprovalRoom extends StatelessWidget {
  const _PendingApprovalRoom({
    required this.proposals,
    required this.onRefresh,
    required this.onOpenProposal,
  });

  final List<Map<String, dynamic>> proposals;
  final VoidCallback onRefresh;
  final ValueChanged<String> onOpenProposal;

  @override
  Widget build(BuildContext context) {
    if (proposals.isEmpty) {
      return Center(
        child: Padding(
          padding: const EdgeInsets.all(24),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              const Icon(Icons.inbox_outlined, size: 48),
              const SizedBox(height: 12),
              const Text(
                '승인 대기 중인 제안이 없습니다.\nAI 대화에서 파일 정리를 요청하면 확인 카드가 여기에 모입니다.',
                textAlign: TextAlign.center,
              ),
              const SizedBox(height: 12),
              OutlinedButton.icon(
                onPressed: onRefresh,
                icon: const Icon(Icons.refresh),
                label: const Text('새로고침'),
              ),
            ],
          ),
        ),
      );
    }
    return RefreshIndicator(
      onRefresh: () async => onRefresh(),
      child: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          Card(
            color: Theme.of(context).colorScheme.secondaryContainer,
            child: const ListTile(
              leading: Icon(Icons.pending_actions_outlined),
              title: Text('승인 대기방'),
              subtitle: Text('AI가 만든 확인 카드를 검토하고 승인 또는 거절하세요.'),
            ),
          ),
          const SizedBox(height: 8),
          for (final proposal in proposals)
            Card(
              child: ListTile(
                leading: const Icon(Icons.rule_folder_outlined),
                title: Text(_proposalTitle(proposal)),
                subtitle: Text(_proposalSubtitle(proposal)),
                trailing: const Icon(Icons.chevron_right),
                onTap: proposal['id'] is String
                    ? () => onOpenProposal(proposal['id'] as String)
                    : null,
              ),
            ),
        ],
      ),
    );
  }

  static String _proposalTitle(Map<String, dynamic> proposal) {
    final summary = proposal['summary'];
    if (summary is Map && summary['title'] is String) {
      return summary['title'] as String;
    }
    return '파일 정리 제안';
  }

  static String _proposalSubtitle(Map<String, dynamic> proposal) {
    final itemCount = proposal['itemCount'];
    final status = proposal['status'] as String? ?? 'OPEN';
    return itemCount is int ? '$status · $itemCount개 항목' : status;
  }
}

class _SessionBar extends StatelessWidget {
  const _SessionBar({
    required this.sessions,
    required this.selectedSessionId,
    required this.pendingApprovalSelected,
    required this.creating,
    required this.renaming,
    required this.deleting,
    required this.onCreate,
    required this.onRename,
    required this.onDelete,
    required this.onSelected,
  });

  final List<Map<String, dynamic>> sessions;
  final String? selectedSessionId;
  final bool pendingApprovalSelected;
  final bool creating;
  final bool renaming;
  final bool deleting;
  final VoidCallback onCreate;
  final VoidCallback onRename;
  final VoidCallback onDelete;
  final ValueChanged<String> onSelected;

  @override
  Widget build(BuildContext context) => Material(
    color: Theme.of(context).colorScheme.surfaceContainerHighest,
    child: Padding(
      padding: const EdgeInsets.fromLTRB(12, 8, 12, 8),
      child: Row(
        children: [
          Expanded(
            child: InputDecorator(
              decoration: const InputDecoration(
                labelText: '대화 세션',
                border: OutlineInputBorder(),
                isDense: true,
              ),
              child: DropdownButtonHideUnderline(
                child: DropdownButton<String>(
                  key: const ValueKey('chat-session-picker'),
                  value: selectedSessionId,
                  isExpanded: true,
                  items: [
                    for (final session in sessions)
                      DropdownMenuItem<String>(
                        value: session['id'] as String?,
                        child: Text(
                          chatSessionTitle(session),
                          overflow: TextOverflow.ellipsis,
                        ),
                      ),
                  ],
                  onChanged: (value) {
                    if (value != null) onSelected(value);
                  },
                ),
              ),
            ),
          ),
          const SizedBox(width: 8),
          IconButton(
            key: const ValueKey('chat-create-session'),
            tooltip: '새 대화',
            onPressed: creating ? null : onCreate,
            icon: creating
                ? const SizedBox.square(
                    dimension: 20,
                    child: CircularProgressIndicator(strokeWidth: 2),
                  )
                : const Icon(Icons.add_comment_outlined),
          ),
          IconButton(
            key: const ValueKey('chat-rename-session'),
            tooltip: '대화 제목 수정',
            onPressed:
                selectedSessionId == null || pendingApprovalSelected || renaming
                ? null
                : onRename,
            icon: renaming
                ? const SizedBox.square(
                    dimension: 20,
                    child: CircularProgressIndicator(strokeWidth: 2),
                  )
                : const Icon(Icons.edit_outlined),
          ),
          IconButton(
            key: const ValueKey('chat-delete-session'),
            tooltip: '대화 삭제',
            onPressed:
                selectedSessionId == null || pendingApprovalSelected || deleting
                ? null
                : onDelete,
            icon: deleting
                ? const SizedBox.square(
                    dimension: 20,
                    child: CircularProgressIndicator(strokeWidth: 2),
                  )
                : const Icon(Icons.delete_outline),
          ),
        ],
      ),
    ),
  );
}

class _ChatBubble extends StatelessWidget {
  const _ChatBubble({
    required this.message,
    required this.busyDraftIds,
    required this.onConfirmDraft,
    required this.onRejectDraft,
    required this.onConfirmRuleDraft,
    required this.onRejectRuleDraft,
  });

  final Map<String, dynamic> message;
  final Set<String> busyDraftIds;
  final ValueChanged<String> onConfirmDraft;
  final ValueChanged<String> onRejectDraft;
  final ValueChanged<String> onConfirmRuleDraft;
  final ValueChanged<String> onRejectRuleDraft;

  @override
  Widget build(BuildContext context) {
    final fromUser = message['senderType'] == 'USER';
    final isDraft = isActionDraftMessage(message);
    final isRuleDraft = isRuleDraftMessage(message);
    final draftId = actionDraftIdFromMessage(message);
    final draftStatus = actionDraftStatusFromMessage(message);
    final draftBusy = draftId != null && busyDraftIds.contains(draftId);
    final keyPrefix = isRuleDraft ? 'chat-rule-draft' : 'chat-command-draft';
    return Align(
      alignment: fromUser ? Alignment.centerRight : Alignment.centerLeft,
      child: Card(
        color: isDraft
            ? Theme.of(context).colorScheme.secondaryContainer
            : null,
        child: Padding(
          padding: const EdgeInsets.all(12),
          child: Column(
            crossAxisAlignment: fromUser
                ? CrossAxisAlignment.end
                : CrossAxisAlignment.start,
            children: [
              if (isDraft)
                Text('확인 카드', style: Theme.of(context).textTheme.labelMedium),
              Text(message['content'] as String? ?? ''),
              if (isDraft && draftId != null) ...[
                const SizedBox(height: 8),
                if (draftStatus == 'DRAFT')
                  Row(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      FilledButton(
                        key: ValueKey('$keyPrefix-confirm-$draftId'),
                        onPressed: draftBusy
                            ? null
                            : () => isRuleDraft
                                  ? onConfirmRuleDraft(draftId)
                                  : onConfirmDraft(draftId),
                        child: draftBusy
                            ? const SizedBox.square(
                                dimension: 16,
                                child: CircularProgressIndicator(
                                  strokeWidth: 2,
                                ),
                              )
                            : const Text('승인'),
                      ),
                      const SizedBox(width: 8),
                      TextButton(
                        key: ValueKey('$keyPrefix-reject-$draftId'),
                        onPressed: draftBusy
                            ? null
                            : () => isRuleDraft
                                  ? onRejectRuleDraft(draftId)
                                  : onRejectDraft(draftId),
                        child: const Text('거절'),
                      ),
                    ],
                  )
                else
                  Chip(
                    key: ValueKey('$keyPrefix-status-$draftId'),
                    label: Text(draftStatus),
                  ),
              ],
            ],
          ),
        ),
      ),
    );
  }
}
