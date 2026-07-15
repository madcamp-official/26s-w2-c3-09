import 'dart:async';

import 'package:dio/dio.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:mousekeeper_character_assets/character_assets.dart';
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
  Future<Map<String, dynamic>> getQuickView(String roomId);
  Future<Map<String, dynamic>> createQuickCleanup(String roomId);
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
const _desktopChatBlue = Color(0xFFB7D7EA);
const _desktopChatHeader = Color(0xFF9FC4DA);
const _desktopChatYellow = Color(0xFFFEE500);
const _desktopChatInk = Color(0xFF26313B);

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
  Future<Map<String, dynamic>> getQuickView(String roomId) =>
      _api.get('/v1/rooms/$roomId/chat/quick-view');

  @override
  Future<Map<String, dynamic>> createQuickCleanup(String roomId) =>
      _api.post('/v1/rooms/$roomId/chat/quick-cleanup', const {});

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

List<Map<String, dynamic>> chatQuickPrompts(Map<String, dynamic>? quickView) {
  final raw = quickView?['prompts'];
  if (raw is! List) return const [];
  return raw
      .whereType<Map>()
      .map(Map<String, dynamic>.from)
      .where(
        (prompt) =>
            prompt['label'] is String &&
            (prompt['label'] as String).trim().isNotEmpty &&
            prompt['prompt'] is String &&
            (prompt['prompt'] as String).trim().isNotEmpty,
      )
      .take(4)
      .toList(growable: false);
}

int chatQuickCount(Map<String, dynamic>? quickView, String key) {
  final value = quickView?[key];
  return value is int && value > 0 ? value : 0;
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

List<Map<String, dynamic>> upsertChatSession(
  List<Map<String, dynamic>> sessions,
  Map<String, dynamic> updated,
) {
  final updatedId = updated['id'];
  if (updatedId is! String) return sessions;
  final replaced = replaceChatSession(sessions, updated);
  if (sessions.any((session) => session['id'] == updatedId)) return replaced;
  return List.unmodifiable([updated, ...sessions]);
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
  const ChatPage({
    super.key,
    required this.roomId,
    this.roomName,
    this.gateway,
  });

  final String roomId;
  final String? roomName;
  final ChatGateway? gateway;

  @override
  ConsumerState<ChatPage> createState() => _ChatPageState();
}

class _ChatPageState extends ConsumerState<ChatPage> {
  final input = TextEditingController();
  final _inputFocus = FocusNode();
  final _scrollController = ScrollController();
  ChatConversationState _conversation = const ChatConversationState.empty();
  List<Map<String, dynamic>> _pendingProposals = const [];
  Map<String, dynamic>? _quickView;
  Object? _error;
  bool _loading = true;
  bool _sending = false;
  bool _loadingMore = false;
  bool _creating = false;
  bool _renaming = false;
  bool _deleting = false;
  bool _quickCleanupBusy = false;
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
    _inputFocus.dispose();
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
      unawaited(_refreshQuickView());
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
      unawaited(_refreshQuickView());
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

  Future<void> _refreshQuickView() async {
    try {
      final quickView = await _gateway.getQuickView(widget.roomId);
      if (_disposed) return;
      setState(() => _quickView = quickView);
    } catch (_) {
      // Quick actions are an enhancement over durable sessions and messages. A temporary failure
      // must not hide or replace the conversation that is already available.
    }
  }

  Future<void> _runQuickCleanup() async {
    if (_quickCleanupBusy) return;
    setState(() => _quickCleanupBusy = true);
    try {
      final result = await _gateway.createQuickCleanup(widget.roomId);
      if (_disposed) return;
      final sessionRaw = result['session'];
      if (sessionRaw is! Map) {
        throw StateError('INVALID_RESPONSE: quick cleanup session');
      }
      final session = Map<String, dynamic>.from(sessionRaw);
      final sessionId = session['id'];
      if (sessionId is! String || sessionId.isEmpty) {
        throw StateError('INVALID_RESPONSE: quick cleanup session id');
      }
      final messages = <Map<String, dynamic>>[];
      for (final key in const ['message', 'assistant']) {
        final raw = result[key];
        if (raw is Map) messages.add(Map<String, dynamic>.from(raw));
      }
      final visibleMessages = sessionId == _conversation.selectedSessionId
          ? mergeChatMessages(_conversation.messages, messages)
          : messages;
      final regularSessions = _conversation.sessions
          .where((item) => !isPendingApprovalSessionId(item['id'] as String?))
          .toList(growable: false);
      final pendingSession = _conversation.sessions.firstWhere(
        (item) => isPendingApprovalSessionId(item['id'] as String?),
        orElse: () => pendingApprovalSession(_pendingProposals.length),
      );
      setState(() {
        _conversation = ChatConversationState(
          sessions: [
            ...upsertChatSession(regularSessions, session),
            pendingSession,
          ],
          messages: visibleMessages,
          selectedSessionId: sessionId,
          messageCursor: lastChatMessageId(visibleMessages),
          hasMoreMessages: false,
        );
      });
      _invalidateChatReadModels(sessionId: sessionId);
      unawaited(_markVisibleSessionRead());
      unawaited(_refreshPendingProposals());
      unawaited(_refreshQuickView());
      _scrollToBottom();
    } catch (error) {
      _showSnack('빠른 정리를 시작하지 못했습니다: ${chatErrorMessage(error)}');
    } finally {
      if (!_disposed) setState(() => _quickCleanupBusy = false);
    }
  }

  void _useQuickPrompt(String prompt) {
    input.value = TextEditingValue(
      text: prompt,
      selection: TextSelection.collapsed(offset: prompt.length),
    );
    _inputFocus.requestFocus();
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
    final selectedSession = _conversation.sessions
        .cast<Map<String, dynamic>?>()
        .firstWhere(
          (session) => session?['id'] == _conversation.selectedSessionId,
          orElse: () => null,
        );
    final quickPrompts = chatQuickPrompts(_quickView);
    final pendingActionCount = chatQuickCount(_quickView, 'pendingActionCount');
    final unreadCount = chatQuickCount(_quickView, 'unreadCount');
    return Scaffold(
      backgroundColor: _desktopChatBlue,
      body: SafeArea(
        child: Column(
          children: [
            _DesktopChatHeader(
              roomName: widget.roomName ?? '관리 폴더',
              sessionTitle: selectedSession == null
                  ? '서버 채팅 세션 연결 중'
                  : chatSessionTitle(selectedSession),
              connected: !_loading && _error == null,
              onBack: () => Navigator.of(context).maybePop(),
            ),
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
            if (!selectedIsPendingApproval)
              _DesktopQuickActionBar(
                prompts: quickPrompts,
                cleanupBusy: _quickCleanupBusy,
                onQuickCleanup: () => unawaited(_runQuickCleanup()),
                onUsePrompt: _useQuickPrompt,
              ),
            if (!selectedIsPendingApproval &&
                (pendingActionCount > 0 || unreadCount > 0))
              _DesktopQuickMeta(
                pendingActionCount: pendingActionCount,
                unreadCount: unreadCount,
              ),
            Expanded(child: _buildBody()),
            _DesktopChatComposer(
              controller: input,
              focusNode: _inputFocus,
              enabled:
                  _conversation.selectedSessionId != null &&
                  !selectedIsPendingApproval,
              sending: _sending,
              onSend: () => unawaited(send()),
            ),
          ],
        ),
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
      return const _DesktopEmptyChat(
        message: '이 방 기준으로 이야기할게요.\n파일 목록, 히스토리, 정리 요청을 말로 물어보세요.',
      );
    }
    return Column(
      children: [
        Expanded(
          child: ListView.builder(
            controller: _scrollController,
            padding: const EdgeInsets.fromLTRB(12, 12, 12, 8),
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
            padding: EdgeInsets.fromLTRB(14, 4, 14, 8),
            child: Align(
              alignment: Alignment.centerLeft,
              child: Text(
                '답을 만들고 있어요!',
                key: ValueKey('chat-answer-pending'),
                style: TextStyle(
                  color: Color(0xFF294B60),
                  fontWeight: FontWeight.w700,
                ),
              ),
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

class _DesktopChatHeader extends StatelessWidget {
  const _DesktopChatHeader({
    required this.roomName,
    required this.sessionTitle,
    required this.connected,
    required this.onBack,
  });

  final String roomName;
  final String sessionTitle;
  final bool connected;
  final VoidCallback onBack;

  @override
  Widget build(BuildContext context) => ColoredBox(
    color: _desktopChatHeader,
    child: Padding(
      padding: const EdgeInsets.fromLTRB(8, 8, 14, 8),
      child: Row(
        children: [
          IconButton(
            key: const ValueKey('chat-back-button'),
            onPressed: onBack,
            tooltip: '뒤로 가기',
            icon: const Icon(Icons.chevron_left, size: 30),
            color: _desktopChatInk,
          ),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  roomName,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: const TextStyle(
                    color: _desktopChatInk,
                    fontSize: 19,
                    fontWeight: FontWeight.w800,
                  ),
                ),
                Text(
                  sessionTitle,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: TextStyle(
                    color: _desktopChatInk.withValues(alpha: 0.62),
                    fontSize: 12,
                  ),
                ),
              ],
            ),
          ),
          Container(
            width: 12,
            height: 12,
            decoration: BoxDecoration(
              shape: BoxShape.circle,
              color: connected
                  ? const Color(0xFF19A45B)
                  : const Color(0xFF7B8790),
              boxShadow: connected
                  ? const [BoxShadow(color: Color(0x3319A45B), spreadRadius: 4)]
                  : null,
            ),
          ),
        ],
      ),
    ),
  );
}

class _DesktopQuickActionBar extends StatelessWidget {
  const _DesktopQuickActionBar({
    required this.prompts,
    required this.cleanupBusy,
    required this.onQuickCleanup,
    required this.onUsePrompt,
  });

  final List<Map<String, dynamic>> prompts;
  final bool cleanupBusy;
  final VoidCallback onQuickCleanup;
  final ValueChanged<String> onUsePrompt;

  @override
  Widget build(BuildContext context) => ColoredBox(
    color: _desktopChatBlue,
    child: SizedBox(
      height: 48,
      child: ListView(
        scrollDirection: Axis.horizontal,
        padding: const EdgeInsets.fromLTRB(10, 6, 10, 6),
        children: [
          _DesktopQuickButton(
            key: const ValueKey('chat-quick-cleanup'),
            label: '빠른 정리',
            busy: cleanupBusy,
            onPressed: cleanupBusy ? null : onQuickCleanup,
          ),
          for (final prompt in prompts) ...[
            const SizedBox(width: 7),
            _DesktopQuickButton(
              label: prompt['label'] as String,
              onPressed: () => onUsePrompt(prompt['prompt'] as String),
            ),
          ],
        ],
      ),
    ),
  );
}

class _DesktopQuickButton extends StatelessWidget {
  const _DesktopQuickButton({
    super.key,
    required this.label,
    required this.onPressed,
    this.busy = false,
  });

  final String label;
  final VoidCallback? onPressed;
  final bool busy;

  @override
  Widget build(BuildContext context) => OutlinedButton(
    onPressed: onPressed,
    style: OutlinedButton.styleFrom(
      minimumSize: const Size(0, 34),
      padding: const EdgeInsets.symmetric(horizontal: 14),
      foregroundColor: _desktopChatInk,
      backgroundColor: Colors.white.withValues(alpha: 0.92),
      side: BorderSide(color: _desktopChatInk.withValues(alpha: 0.20)),
      shape: const StadiumBorder(),
      textStyle: const TextStyle(fontWeight: FontWeight.w700),
    ),
    child: busy
        ? const SizedBox.square(
            dimension: 15,
            child: CircularProgressIndicator(strokeWidth: 2),
          )
        : Text(label),
  );
}

class _DesktopQuickMeta extends StatelessWidget {
  const _DesktopQuickMeta({
    required this.pendingActionCount,
    required this.unreadCount,
  });

  final int pendingActionCount;
  final int unreadCount;

  @override
  Widget build(BuildContext context) => ColoredBox(
    color: _desktopChatBlue,
    child: Padding(
      padding: const EdgeInsets.fromLTRB(12, 0, 12, 5),
      child: Row(
        children: [
          if (pendingActionCount > 0)
            _DesktopMetaChip(label: '제안 $pendingActionCount'),
          if (pendingActionCount > 0 && unreadCount > 0)
            const SizedBox(width: 6),
          if (unreadCount > 0) _DesktopMetaChip(label: '새 메시지 $unreadCount'),
        ],
      ),
    ),
  );
}

class _DesktopMetaChip extends StatelessWidget {
  const _DesktopMetaChip({required this.label});

  final String label;

  @override
  Widget build(BuildContext context) => DecoratedBox(
    decoration: BoxDecoration(
      color: _desktopChatInk.withValues(alpha: 0.10),
      borderRadius: BorderRadius.circular(10),
    ),
    child: Padding(
      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 3),
      child: Text(
        label,
        style: TextStyle(
          color: _desktopChatInk.withValues(alpha: 0.72),
          fontSize: 11,
          fontWeight: FontWeight.w700,
        ),
      ),
    ),
  );
}

class _DesktopChatComposer extends StatelessWidget {
  const _DesktopChatComposer({
    required this.controller,
    required this.focusNode,
    required this.enabled,
    required this.sending,
    required this.onSend,
  });

  final TextEditingController controller;
  final FocusNode focusNode;
  final bool enabled;
  final bool sending;
  final VoidCallback onSend;

  @override
  Widget build(BuildContext context) => ColoredBox(
    color: const Color(0xFFF7F7F7),
    child: Padding(
      padding: const EdgeInsets.fromLTRB(10, 9, 10, 10),
      child: Row(
        children: [
          Expanded(
            child: TextField(
              key: const ValueKey('chat-message-field'),
              controller: controller,
              focusNode: focusNode,
              maxLength: 2000,
              enabled: enabled,
              textInputAction: TextInputAction.send,
              decoration: InputDecoration(
                hintText: enabled ? 'AI에게 파일 정리 요청을 입력하세요' : '대화를 선택하세요',
                counterText: '',
                isDense: true,
                filled: true,
                fillColor: Colors.white,
                contentPadding: const EdgeInsets.symmetric(
                  horizontal: 16,
                  vertical: 12,
                ),
                border: OutlineInputBorder(
                  borderRadius: BorderRadius.circular(22),
                  borderSide: const BorderSide(color: Color(0xFFD8DDE2)),
                ),
                enabledBorder: OutlineInputBorder(
                  borderRadius: BorderRadius.circular(22),
                  borderSide: const BorderSide(color: Color(0xFFD8DDE2)),
                ),
              ),
              onSubmitted: (_) {
                if (!sending && enabled) onSend();
              },
            ),
          ),
          const SizedBox(width: 8),
          IconButton.filled(
            key: const ValueKey('chat-send-button'),
            tooltip: '보내기',
            onPressed: sending || !enabled ? null : onSend,
            style: IconButton.styleFrom(
              backgroundColor: _desktopChatYellow,
              foregroundColor: const Color(0xFF2F2500),
              disabledBackgroundColor: const Color(0xFFE3E3E3),
            ),
            icon: const Icon(Icons.send_rounded),
          ),
        ],
      ),
    ),
  );
}

class _DesktopEmptyChat extends StatelessWidget {
  const _DesktopEmptyChat({required this.message});

  final String message;

  @override
  Widget build(BuildContext context) => Align(
    alignment: Alignment.topLeft,
    child: Padding(
      padding: const EdgeInsets.all(14),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          const _MouseChatAvatar(),
          const SizedBox(width: 8),
          Flexible(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  'MouseKeeper',
                  style: TextStyle(
                    color: _desktopChatInk.withValues(alpha: 0.76),
                    fontSize: 11,
                    fontWeight: FontWeight.w700,
                  ),
                ),
                const SizedBox(height: 3),
                DecoratedBox(
                  decoration: const BoxDecoration(
                    color: Colors.white,
                    borderRadius: BorderRadius.only(
                      topRight: Radius.circular(15),
                      bottomLeft: Radius.circular(15),
                      bottomRight: Radius.circular(15),
                    ),
                  ),
                  child: Padding(
                    padding: const EdgeInsets.symmetric(
                      horizontal: 12,
                      vertical: 10,
                    ),
                    child: Text(message),
                  ),
                ),
              ],
            ),
          ),
        ],
      ),
    ),
  );
}

class _MouseChatAvatar extends StatelessWidget {
  const _MouseChatAvatar();

  @override
  Widget build(BuildContext context) => ClipRRect(
    borderRadius: BorderRadius.circular(12),
    child: ColoredBox(
      color: const Color(0xFFFFF6D9),
      child: Image.asset(
        mousekeeperPairingIconAsset,
        package: mousekeeperMascotPackage,
        width: 34,
        height: 34,
        fit: BoxFit.cover,
        filterQuality: FilterQuality.none,
      ),
    ),
  );
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
  Widget build(BuildContext context) => ColoredBox(
    color: _desktopChatHeader.withValues(alpha: 0.88),
    child: Padding(
      padding: const EdgeInsets.fromLTRB(10, 5, 8, 7),
      child: Row(
        children: [
          Expanded(
            child: DecoratedBox(
              decoration: BoxDecoration(
                color: Colors.white.withValues(alpha: 0.92),
                borderRadius: BorderRadius.circular(16),
                border: Border.all(
                  color: _desktopChatInk.withValues(alpha: 0.14),
                ),
              ),
              child: Padding(
                padding: const EdgeInsets.symmetric(horizontal: 12),
                child: DropdownButtonHideUnderline(
                  child: DropdownButton<String>(
                    key: const ValueKey('chat-session-picker'),
                    value: selectedSessionId,
                    isDense: true,
                    isExpanded: true,
                    icon: const Icon(Icons.expand_more),
                    items: [
                      for (final session in sessions)
                        DropdownMenuItem<String>(
                          value: session['id'] as String?,
                          child: Text(
                            chatSessionTitle(session),
                            overflow: TextOverflow.ellipsis,
                            style: const TextStyle(
                              color: _desktopChatInk,
                              fontWeight: FontWeight.w700,
                            ),
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
          ),
          const SizedBox(width: 4),
          IconButton(
            key: const ValueKey('chat-create-session'),
            tooltip: '새 대화',
            onPressed: creating ? null : onCreate,
            icon: creating
                ? const SizedBox.square(
                    dimension: 20,
                    child: CircularProgressIndicator(strokeWidth: 2),
                  )
                : const Icon(Icons.add_comment_outlined, size: 20),
            color: _desktopChatInk,
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
                : const Icon(Icons.edit_outlined, size: 20),
            color: _desktopChatInk,
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
                : const Icon(Icons.delete_outline, size: 20),
            color: _desktopChatInk,
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
    final bubble = DecoratedBox(
      decoration: BoxDecoration(
        color: fromUser
            ? _desktopChatYellow
            : isDraft
            ? const Color(0xFFEAF5FF)
            : Colors.white,
        borderRadius: BorderRadius.only(
          topLeft: Radius.circular(fromUser ? 15 : 3),
          topRight: Radius.circular(fromUser ? 3 : 15),
          bottomLeft: const Radius.circular(15),
          bottomRight: const Radius.circular(15),
        ),
        border: isDraft
            ? Border.all(color: const Color(0xFF7DAFC8), width: 1.2)
            : null,
      ),
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
        child: Column(
          crossAxisAlignment: fromUser
              ? CrossAxisAlignment.end
              : CrossAxisAlignment.start,
          children: [
            if (isDraft)
              const Padding(
                padding: EdgeInsets.only(bottom: 4),
                child: Text(
                  '확인 카드',
                  style: TextStyle(
                    color: Color(0xFF436577),
                    fontSize: 11,
                    fontWeight: FontWeight.w800,
                  ),
                ),
              ),
            Text(
              message['content'] as String? ?? '',
              style: const TextStyle(color: Color(0xFF2D2500), height: 1.35),
            ),
            if (isDraft && draftId != null) ...[
              const SizedBox(height: 8),
              if (draftStatus == 'DRAFT')
                Wrap(
                  spacing: 8,
                  runSpacing: 6,
                  children: [
                    FilledButton(
                      key: ValueKey('$keyPrefix-confirm-$draftId'),
                      onPressed: draftBusy
                          ? null
                          : () => isRuleDraft
                                ? onConfirmRuleDraft(draftId)
                                : onConfirmDraft(draftId),
                      style: FilledButton.styleFrom(
                        backgroundColor: const Color(0xFF436577),
                        foregroundColor: Colors.white,
                      ),
                      child: draftBusy
                          ? const SizedBox.square(
                              dimension: 16,
                              child: CircularProgressIndicator(
                                strokeWidth: 2,
                                color: Colors.white,
                              ),
                            )
                          : const Text('승인'),
                    ),
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
                  visualDensity: VisualDensity.compact,
                ),
            ],
          ],
        ),
      ),
    );
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 5),
      child: Align(
        alignment: fromUser ? Alignment.centerRight : Alignment.centerLeft,
        child: ConstrainedBox(
          constraints: BoxConstraints(
            maxWidth: MediaQuery.sizeOf(context).width * 0.78,
          ),
          child: fromUser
              ? bubble
              : Row(
                  mainAxisSize: MainAxisSize.max,
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    const _MouseChatAvatar(),
                    const SizedBox(width: 8),
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(
                            'MouseKeeper',
                            style: TextStyle(
                              color: _desktopChatInk.withValues(alpha: 0.76),
                              fontSize: 11,
                              fontWeight: FontWeight.w700,
                            ),
                          ),
                          const SizedBox(height: 3),
                          bubble,
                        ],
                      ),
                    ),
                  ],
                ),
        ),
      ),
    );
  }
}
