import 'dart:async';

import 'package:dio/dio.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:uuid/uuid.dart';

import '../../core/network/api_client.dart';
import 'readme_command_page.dart';

abstract interface class ChatGateway {
  Future<List<Map<String, dynamic>>> listSessions(String roomId);
  Future<Map<String, dynamic>> createSession(String roomId);
  Future<void> deleteSession(String sessionId);
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
}

const chatMessagePageSize = 30;

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
  Future<void> deleteSession(String sessionId) async {
    await _api.delete('/v1/chat-sessions/$sessionId');
  }

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
  if (ai is Map && ai['kind'] == 'COMMAND_DRAFT') {
    return '확인 카드가 생성되었습니다. 내용을 보고 수락 또는 거절해 주세요.';
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

String? commandDraftIdFromMessage(Map<String, dynamic> message) {
  if (message['messageType'] != 'COMMAND_DRAFT') return null;
  final payload = message['structuredPayload'];
  if (payload is Map && payload['id'] is String) {
    return payload['id'] as String;
  }
  return null;
}

String commandDraftStatusFromMessage(Map<String, dynamic> message) {
  final payload = message['structuredPayload'];
  if (payload is Map && payload['status'] is String) {
    return payload['status'] as String;
  }
  return 'DRAFT';
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
  List<Map<String, dynamic>> _sessions = const [];
  List<Map<String, dynamic>> _messages = const [];
  String? _selectedSessionId;
  Object? _error;
  bool _loading = true;
  bool _sending = false;
  bool _loadingMore = false;
  bool _creating = false;
  bool _deleting = false;
  bool _disposed = false;
  final Set<String> _draftingIds = {};
  int _loadVersion = 0;
  String? _messageCursor;
  bool _hasMoreMessages = false;

  ChatGateway get _gateway =>
      widget.gateway ?? ApiChatGateway(ref.read(apiClientProvider));

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
      var sessions = await _gateway.listSessions(widget.roomId);
      if (_stale(version)) return;
      if (sessions.isEmpty) {
        final created = await _gateway.createSession(widget.roomId);
        if (_stale(version)) return;
        sessions = [created];
      }
      final selectedId = _selectExistingOrFirst(sessions, _selectedSessionId);
      final messages = selectedId == null
          ? <Map<String, dynamic>>[]
          : await _gateway.listMessages(selectedId);
      if (_stale(version)) return;
      setState(() {
        _sessions = sessions;
        _selectedSessionId = selectedId;
        _messages = messages;
        _messageCursor = _lastMessageId(messages);
        _hasMoreMessages = messages.length == chatMessagePageSize;
        _loading = false;
      });
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
      _selectedSessionId = sessionId;
      _messages = const [];
      _loading = true;
      _error = null;
    });
    try {
      final messages = await _gateway.listMessages(sessionId);
      if (_stale(version)) return;
      setState(() {
        _messages = messages;
        _messageCursor = _lastMessageId(messages);
        _hasMoreMessages = messages.length == chatMessagePageSize;
        _loading = false;
      });
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
      setState(() {
        _sessions = [created, ..._sessions];
        _selectedSessionId = created['id'] as String;
        _messages = const [];
        _messageCursor = null;
        _hasMoreMessages = false;
        _error = null;
      });
    } catch (error) {
      _showSnack(chatErrorMessage(error));
    } finally {
      if (!_disposed) setState(() => _creating = false);
    }
  }

  Future<void> _deleteSelectedSession() async {
    final sessionId = _selectedSessionId;
    if (sessionId == null || _deleting) return;
    setState(() => _deleting = true);
    try {
      await _gateway.deleteSession(sessionId);
      if (_disposed) return;
      final remaining = _sessions
          .where((session) => session['id'] != sessionId)
          .toList(growable: false);
      final nextId = _selectExistingOrFirst(remaining, null);
      setState(() {
        _sessions = remaining;
        _selectedSessionId = nextId;
        _messages = const [];
        _messageCursor = null;
        _hasMoreMessages = false;
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
    final sessionId = _selectedSessionId;
    final content = input.text.trim();
    if (sessionId == null || content.isEmpty || _sending) return;
    setState(() => _sending = true);
    try {
      final result = await _gateway.sendMessage(sessionId, content);
      if (_disposed) return;
      input.clear();
      final newMessages = <Map<String, dynamic>>[
        ..._messages,
        if (result['message'] is Map<String, dynamic>)
          result['message'] as Map<String, dynamic>,
        if (result['assistant'] is Map<String, dynamic>)
          result['assistant'] as Map<String, dynamic>,
      ];
      setState(() {
        _messages = newMessages;
        _messageCursor = _lastMessageId(newMessages);
        _sessions = _touchSessionPreview(_sessions, sessionId, content);
      });
      _scrollToBottom();
      final notice = chatSendNotice(result);
      if (notice.isNotEmpty) _showSnack(notice);
    } catch (error) {
      _showSnack('메시지 저장 실패: ${chatErrorMessage(error)}');
    } finally {
      if (!_disposed) setState(() => _sending = false);
    }
  }

  Future<void> _loadMoreMessages() async {
    final sessionId = _selectedSessionId;
    final cursor = _messageCursor;
    if (sessionId == null || cursor == null || _loadingMore) return;
    setState(() => _loadingMore = true);
    try {
      final next = await _gateway.listMessages(sessionId, cursor: cursor);
      if (_disposed || sessionId != _selectedSessionId) return;
      final merged = _mergeMessages(_messages, next);
      setState(() {
        _messages = merged;
        _messageCursor = _lastMessageId(merged);
        _hasMoreMessages = next.length == chatMessagePageSize;
      });
      _scrollToBottom();
    } catch (error) {
      _showSnack('메시지를 더 불러오지 못했습니다: ${chatErrorMessage(error)}');
    } finally {
      if (!_disposed) setState(() => _loadingMore = false);
    }
  }

  Future<void> _confirmCommandDraft(String draftId) async {
    if (_draftingIds.contains(draftId)) return;
    setState(() => _draftingIds.add(draftId));
    try {
      final result = await _gateway.confirmCommandDraft(
        draftId,
        const Uuid().v4(),
      );
      if (_disposed) return;
      _patchDraftMessage(draftId, result['draft']);
      _showSnack('Command draft confirmed.');
    } catch (error) {
      _showSnack(
        'Command draft confirmation failed: ${chatErrorMessage(error)}',
      );
    } finally {
      if (!_disposed) setState(() => _draftingIds.remove(draftId));
    }
  }

  Future<void> _rejectCommandDraft(String draftId) async {
    if (_draftingIds.contains(draftId)) return;
    setState(() => _draftingIds.add(draftId));
    try {
      final result = await _gateway.rejectCommandDraft(draftId);
      if (_disposed) return;
      _patchDraftMessage(draftId, result['draft']);
      _showSnack('Command draft rejected.');
    } catch (error) {
      _showSnack('Command draft rejection failed: ${chatErrorMessage(error)}');
    } finally {
      if (!_disposed) setState(() => _draftingIds.remove(draftId));
    }
  }

  void _patchDraftMessage(String draftId, Object? draft) {
    if (draft is! Map) return;
    final nextPayload = Map<String, dynamic>.from(draft);
    setState(() {
      _messages = [
        for (final message in _messages)
          if (commandDraftIdFromMessage(message) == draftId)
            {...message, 'structuredPayload': nextPayload}
          else
            message,
      ];
    });
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

  static String? _selectExistingOrFirst(
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

  static List<Map<String, dynamic>> _touchSessionPreview(
    List<Map<String, dynamic>> sessions,
    String sessionId,
    String preview,
  ) => [
    for (final session in sessions)
      if (session['id'] == sessionId)
        {...session, 'messagePreview': preview}
      else
        session,
  ];

  static String? _lastMessageId(List<Map<String, dynamic>> messages) {
    if (messages.isEmpty) return null;
    return messages.last['id'] as String?;
  }

  static List<Map<String, dynamic>> _mergeMessages(
    List<Map<String, dynamic>> existing,
    List<Map<String, dynamic>> received,
  ) {
    final seen = existing
        .map((message) => message['id'])
        .whereType<String>()
        .toSet();
    return [
      ...existing,
      for (final message in received)
        if (message['id'] is String && seen.add(message['id'] as String))
          message,
    ];
  }

  @override
  Widget build(BuildContext context) => Scaffold(
    appBar: AppBar(
      title: const Text('집쥐인과 대화'),
      actions: [
        IconButton(
          tooltip: 'README 초안 요청',
          icon: const Icon(Icons.description_outlined),
          onPressed: () => Navigator.of(context).push(
            MaterialPageRoute(
              builder: (_) => ReadmeCommandPage(roomId: widget.roomId),
            ),
          ),
        ),
      ],
    ),
    body: Column(
      children: [
        _SessionBar(
          sessions: _sessions,
          selectedSessionId: _selectedSessionId,
          creating: _creating,
          deleting: _deleting,
          onCreate: _createSession,
          onDelete: _deleteSelectedSession,
          onSelected: (id) {
            if (id != _selectedSessionId) unawaited(_selectSession(id));
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
                    enabled: _selectedSessionId != null && !_sending,
                    decoration: const InputDecoration(
                      hintText: '메시지',
                      border: OutlineInputBorder(),
                    ),
                    onSubmitted: (_) => unawaited(send()),
                  ),
                ),
                const SizedBox(width: 8),
                IconButton(
                  key: const ValueKey('chat-send-button'),
                  tooltip: '보내기',
                  onPressed: _sending || _selectedSessionId == null
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
    if (_selectedSessionId == null) {
      return const Center(
        child: Text('대화가 없습니다.\n새 대화를 만들어 주세요.', textAlign: TextAlign.center),
      );
    }
    if (_messages.isEmpty) {
      return const Center(
        child: Text(
          '아직 메시지가 없습니다.\n정리 요청은 이 대화에서 확인 카드로 진행됩니다.',
          textAlign: TextAlign.center,
        ),
      );
    }
    return ListView.builder(
      controller: _scrollController,
      padding: const EdgeInsets.all(16),
      itemCount: _messages.length + (_hasMoreMessages ? 1 : 0),
      itemBuilder: (context, index) {
        if (index == _messages.length) {
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
          message: _messages[index],
          busyDraftIds: _draftingIds,
          onConfirmDraft: (draftId) => unawaited(_confirmCommandDraft(draftId)),
          onRejectDraft: (draftId) => unawaited(_rejectCommandDraft(draftId)),
        );
      },
    );
  }
}

class _SessionBar extends StatelessWidget {
  const _SessionBar({
    required this.sessions,
    required this.selectedSessionId,
    required this.creating,
    required this.deleting,
    required this.onCreate,
    required this.onDelete,
    required this.onSelected,
  });

  final List<Map<String, dynamic>> sessions;
  final String? selectedSessionId;
  final bool creating;
  final bool deleting;
  final VoidCallback onCreate;
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
            key: const ValueKey('chat-delete-session'),
            tooltip: '대화 삭제',
            onPressed: selectedSessionId == null || deleting ? null : onDelete,
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
  });

  final Map<String, dynamic> message;
  final Set<String> busyDraftIds;
  final ValueChanged<String> onConfirmDraft;
  final ValueChanged<String> onRejectDraft;

  @override
  Widget build(BuildContext context) {
    final fromUser = message['senderType'] == 'USER';
    final isDraft = message['messageType'] == 'COMMAND_DRAFT';
    final draftId = commandDraftIdFromMessage(message);
    final draftStatus = commandDraftStatusFromMessage(message);
    final draftBusy = draftId != null && busyDraftIds.contains(draftId);
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
                        key: ValueKey('chat-command-draft-confirm-$draftId'),
                        onPressed: draftBusy
                            ? null
                            : () => onConfirmDraft(draftId),
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
                        key: ValueKey('chat-command-draft-reject-$draftId'),
                        onPressed: draftBusy
                            ? null
                            : () => onRejectDraft(draftId),
                        child: const Text('거절'),
                      ),
                    ],
                  )
                else
                  Chip(
                    key: ValueKey('chat-command-draft-status-$draftId'),
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
