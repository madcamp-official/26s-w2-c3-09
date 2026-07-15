import 'dart:async';

import 'package:dio/dio.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../core/network/api_client.dart';
import '../../core/sync/realtime_controller.dart';
import '../../core/widgets/cheese_loading.dart';
import '../../storage/display_cache.dart';
import '../auth/connection_gate_controller.dart';
import '../chat/chat_page.dart';
import '../files/files_page.dart';
import '../proposals/proposal_page.dart';

const _pixelInk = Color(0xFF3A2A1F);
const _pixelPaper = Color(0xFFFFE9B8);
const _pixelGreen = Color(0xFF597A45);
const _pixelRust = Color(0xFFB85C38);

class _PixelCard extends StatelessWidget {
  const _PixelCard({required this.child, this.color});
  final Widget child;
  final Color? color;

  @override
  Widget build(BuildContext context) => Card(
    color: color ?? _pixelPaper,
    elevation: 0,
    margin: const EdgeInsets.only(right: 5, bottom: 5),
    shape: const BeveledRectangleBorder(
      side: BorderSide(color: _pixelInk, width: 2),
      borderRadius: BorderRadius.all(Radius.circular(3)),
    ),
    child: DecoratedBox(
      decoration: const BoxDecoration(
        boxShadow: [BoxShadow(color: _pixelInk, offset: Offset(5, 5))],
      ),
      child: child,
    ),
  );
}

class _PixelMeter extends StatelessWidget {
  const _PixelMeter({required this.value, required this.color});
  final double value;
  final Color color;

  @override
  Widget build(BuildContext context) => Row(
    children: List.generate(10, (index) {
      final filled = index < (value.clamp(0, 1) * 10).round();
      return Expanded(
        child: Container(
          height: 14,
          margin: EdgeInsets.only(right: index == 9 ? 0 : 2),
          decoration: BoxDecoration(
            color: filled ? color : const Color(0xFFD2B982),
            border: Border.all(color: _pixelInk),
          ),
        ),
      );
    }),
  );
}

class RoomContent {
  const RoomContent({
    required this.commands,
    required this.proposals,
    required this.executions,
    required this.activity,
    required this.snapshot,
    required this.isOffline,
  });

  final List<Map<String, dynamic>> commands;
  final List<Map<String, dynamic>> proposals;
  final List<Map<String, dynamic>> executions;
  final List<Map<String, dynamic>> activity;
  final Map<String, dynamic>? snapshot;
  final bool isOffline;

  RoomContent copyWith({
    List<Map<String, dynamic>>? commands,
    List<Map<String, dynamic>>? proposals,
    List<Map<String, dynamic>>? executions,
    List<Map<String, dynamic>>? activity,
    Map<String, dynamic>? snapshot,
  }) => RoomContent(
    commands: commands ?? this.commands,
    proposals: proposals ?? this.proposals,
    executions: executions ?? this.executions,
    activity: activity ?? this.activity,
    snapshot: snapshot ?? this.snapshot,
    isOffline: isOffline,
  );
}

typedef RoomListFetcher =
    Future<List<Map<String, dynamic>>> Function(String path);

typedef RoomNullableFetcher =
    Future<Map<String, dynamic>?> Function(String path);

final roomListFetcherProvider = Provider<RoomListFetcher>((ref) {
  final api = ref.watch(apiClientProvider);
  return api.getList;
});

final roomNullableFetcherProvider = Provider<RoomNullableFetcher>((ref) {
  final api = ref.watch(apiClientProvider);
  return api.getNullable;
});

final roomCommandListProvider = FutureProvider.autoDispose
    .family<List<Map<String, dynamic>>, String>(
      (ref, roomId) =>
          ref.watch(roomListFetcherProvider)('/v1/rooms/$roomId/commands'),
    );

final roomProposalListProvider = FutureProvider.autoDispose
    .family<List<Map<String, dynamic>>, String>(
      (ref, roomId) => ref.watch(roomListFetcherProvider)(
        '/v1/rooms/$roomId/proposals/open',
      ),
    );

final roomExecutionListProvider = FutureProvider.autoDispose
    .family<List<Map<String, dynamic>>, String>(
      (ref, roomId) =>
          ref.watch(roomListFetcherProvider)('/v1/rooms/$roomId/executions'),
    );

final roomActivityListProvider = FutureProvider.autoDispose
    .family<List<Map<String, dynamic>>, String>(
      (ref, roomId) => ref.watch(roomListFetcherProvider)(
        '/v1/rooms/$roomId/activity?limit=20',
      ),
    );

final roomSnapshotProvider = FutureProvider.autoDispose
    .family<Map<String, dynamic>?, String>(
      (ref, roomId) => ref.watch(roomNullableFetcherProvider)(
        '/v1/rooms/$roomId/snapshots/latest',
      ),
    );

List<Map<String, dynamic>> patchCommandItemsForRealtimeUpdate({
  required List<Map<String, dynamic>> commands,
  required RealtimeHomeUpdate update,
  required String roomId,
}) {
  final commandId = switch (update.kind) {
    RealtimeHomeUpdateKind.commandStatus ||
    RealtimeHomeUpdateKind.decisionCreated => update.commandId,
    _ => null,
  };
  final status = switch (update.kind) {
    RealtimeHomeUpdateKind.commandStatus ||
    RealtimeHomeUpdateKind.decisionCreated => update.commandStatus,
    _ => null,
  };
  if (update.roomId != roomId || commandId == null || status == null) {
    return commands;
  }
  var changed = false;
  var matched = false;
  final next = commands
      .map((command) {
        if (command['id'] != commandId) return command;
        matched = true;
        if (command['status'] == status) return command;
        changed = true;
        return {...command, 'status': status};
      })
      .toList(growable: true);
  if (matched) return changed ? List.unmodifiable(next) : commands;
  return List.unmodifiable([
    {'id': commandId, 'status': status},
    ...commands,
  ]);
}

List<Map<String, dynamic>> patchProposalItemsForRealtimeUpdate({
  required List<Map<String, dynamic>> proposals,
  required RealtimeHomeUpdate update,
  required String roomId,
}) {
  if (update.roomId != roomId ||
      update.proposalId == null ||
      update.proposalStatus == null ||
      (update.kind != RealtimeHomeUpdateKind.proposalCreated &&
          update.kind != RealtimeHomeUpdateKind.decisionCreated)) {
    return proposals;
  }
  final proposalId = update.proposalId!;
  final status = update.proposalStatus!;
  if (update.kind == RealtimeHomeUpdateKind.decisionCreated &&
      status != 'OPEN') {
    final next = proposals
        .where((proposal) => proposal['id'] != proposalId)
        .toList(growable: false);
    return next.length == proposals.length ? proposals : next;
  }

  Map<String, dynamic> patch(Map<String, dynamic> proposal) {
    final next = {...proposal, 'status': status};
    if (update.commandId != null) next['commandId'] = update.commandId!;
    if (update.proposalSummary != null) {
      next['summary'] = update.proposalSummary!;
    }
    if (update.proposalItemCount != null) {
      next['itemCount'] = update.proposalItemCount!;
    }
    return next;
  }

  var changed = false;
  var matched = false;
  final next = proposals
      .map((proposal) {
        if (proposal['id'] != proposalId) return proposal;
        matched = true;
        final patched = patch(proposal);
        if (_mapShallowEquals(proposal, patched)) return proposal;
        changed = true;
        return patched;
      })
      .toList(growable: true);
  if (matched) return changed ? List.unmodifiable(next) : proposals;
  return List.unmodifiable([
    patch({'id': proposalId, 'roomId': roomId}),
    ...proposals,
  ]);
}

bool _mapShallowEquals(Map<String, dynamic> left, Map<String, dynamic> right) {
  if (left.length != right.length) return false;
  for (final key in left.keys) {
    if (!right.containsKey(key) || left[key] != right[key]) return false;
  }
  return true;
}

Map<String, dynamic>? patchRoomSnapshotForRealtimeUpdate({
  required Map<String, dynamic>? snapshot,
  required RealtimeHomeUpdate update,
  required String roomId,
}) {
  if (update.kind != RealtimeHomeUpdateKind.roomSnapshotUpdated ||
      update.roomId != roomId ||
      update.roomSnapshot == null) {
    return snapshot;
  }
  final next = update.roomSnapshot!;
  if (!_snapshotIsNewer(
    currentCalculatedAt: snapshot?['calculatedAt'],
    nextCalculatedAt: next['calculatedAt'],
  )) {
    return snapshot;
  }
  return Map<String, dynamic>.unmodifiable(next);
}

RoomContent reduceRoomContentForRealtimeUpdate({
  required RoomContent current,
  required RealtimeHomeUpdate update,
  required String roomId,
}) {
  final commands = patchCommandItemsForRealtimeUpdate(
    commands: current.commands,
    update: update,
    roomId: roomId,
  );
  final proposals = patchProposalItemsForRealtimeUpdate(
    proposals: current.proposals,
    update: update,
    roomId: roomId,
  );
  final executions = patchExecutionItemsForRealtimeUpdate(
    executions: current.executions,
    update: update,
    roomId: roomId,
  );
  final snapshot = patchRoomSnapshotForRealtimeUpdate(
    snapshot: current.snapshot,
    update: update,
    roomId: roomId,
  );
  if (identical(commands, current.commands) &&
      identical(proposals, current.proposals) &&
      identical(executions, current.executions) &&
      identical(snapshot, current.snapshot)) {
    return current;
  }
  return current.copyWith(
    commands: commands,
    proposals: proposals,
    executions: executions,
    snapshot: snapshot,
  );
}

bool _snapshotIsNewer({
  required Object? currentCalculatedAt,
  required Object? nextCalculatedAt,
}) {
  if (nextCalculatedAt is! String || nextCalculatedAt.isEmpty) return false;
  final next = DateTime.tryParse(nextCalculatedAt);
  if (next == null) return false;
  if (currentCalculatedAt is! String || currentCalculatedAt.isEmpty) {
    return true;
  }
  final current = DateTime.tryParse(currentCalculatedAt);
  return current == null || next.isAfter(current);
}

List<Map<String, dynamic>> patchExecutionItemsForRealtimeUpdate({
  required List<Map<String, dynamic>> executions,
  required RealtimeHomeUpdate update,
  required String roomId,
}) {
  if (update.kind != RealtimeHomeUpdateKind.executionStatus ||
      update.roomId != roomId ||
      update.executionId == null ||
      update.executionStatus == null) {
    return executions;
  }
  final executionId = update.executionId!;
  final status = update.executionStatus!;
  var changed = false;
  var matched = false;
  final next = executions
      .map((item) {
        final rawExecution = item['execution'];
        if (rawExecution is! Map || rawExecution['id'] != executionId) {
          return item;
        }
        matched = true;
        if (rawExecution['status'] == status) return item;
        changed = true;
        return {
          ...item,
          'execution': {
            ...Map<String, dynamic>.from(rawExecution),
            'status': status,
          },
        };
      })
      .toList(growable: true);
  if (matched) return changed ? List.unmodifiable(next) : executions;
  return List.unmodifiable([
    {
      'execution': {'id': executionId, 'status': status},
      'proposal': null,
    },
    ...executions,
  ]);
}

class RoomPage extends ConsumerStatefulWidget {
  const RoomPage({super.key, required this.room});

  final Map<String, dynamic> room;

  @override
  ConsumerState<RoomPage> createState() => _RoomPageState();
}

class _RoomPageState extends ConsumerState<RoomPage> {
  late Future<RoomContent> _content;
  RoomContent? _latestContent;
  bool _suppressNextRealtimeRevisionReload = false;

  @override
  void initState() {
    super.initState();
    _reload();
  }

  Future<RoomContent> _load() async {
    final cache = ref.read(displayCacheProvider);
    final id = widget.room['id'] as String;
    try {
      final lists = await Future.wait([
        ref.read(roomCommandListProvider(id).future),
        ref.read(roomProposalListProvider(id).future),
        ref.read(roomExecutionListProvider(id).future),
        ref.read(roomActivityListProvider(id).future),
      ]);
      final snapshot = await ref.read(roomSnapshotProvider(id).future);
      if (!_roomIsActive(id)) throw StateError('ROOM_REMOVED');
      await Future.wait([
        cache.replaceCommands(id, lists[0]),
        cache.replaceProposals(id, lists[1]),
        cache.replaceExecutions(id, lists[2]),
        cache.saveSnapshot(id, snapshot),
      ]);
      if (!_roomIsActive(id)) throw StateError('ROOM_REMOVED');
      return RoomContent(
        commands: lists[0],
        proposals: lists[1],
        executions: lists[2],
        activity: lists[3],
        snapshot: snapshot,
        isOffline: false,
      );
    } on DioException catch (error) {
      if (!_isOffline(error)) rethrow;
      final cached = await Future.wait<dynamic>([
        cache.commands(id),
        cache.proposals(id),
        cache.executions(id),
        cache.snapshot(id),
      ]);
      return RoomContent(
        commands: cached[0] as List<Map<String, dynamic>>,
        proposals: cached[1] as List<Map<String, dynamic>>,
        executions: cached[2] as List<Map<String, dynamic>>,
        activity: const [],
        snapshot: cached[3] as Map<String, dynamic>?,
        isOffline: true,
      );
    }
  }

  bool _roomIsActive(String roomId) {
    final gate = ref.read(connectionGateControllerProvider).asData?.value;
    return gate != null && gate.rooms.any((room) => room['id'] == roomId);
  }

  bool _isOffline(DioException error) {
    return switch (error.type) {
      DioExceptionType.connectionError ||
      DioExceptionType.connectionTimeout ||
      DioExceptionType.receiveTimeout ||
      DioExceptionType.sendTimeout => true,
      _ => false,
    };
  }

  void _reload() {
    final roomId = widget.room['id'] as String;
    ref.invalidate(roomCommandListProvider(roomId));
    ref.invalidate(roomProposalListProvider(roomId));
    ref.invalidate(roomExecutionListProvider(roomId));
    ref.invalidate(roomActivityListProvider(roomId));
    ref.invalidate(roomSnapshotProvider(roomId));
    _content = _load().then((content) {
      if (mounted) _latestContent = content;
      return content;
    });
  }

  bool _applyRealtimeUpdate(RealtimeHomeUpdate update) {
    final current = _latestContent;
    if (current == null) return false;
    final roomId = widget.room['id'] as String;
    final patched = reduceRoomContentForRealtimeUpdate(
      current: current,
      update: update,
      roomId: roomId,
    );
    if (identical(patched, current)) return false;
    _latestContent = patched;
    _content = Future.value(patched);
    final cache = ref.read(displayCacheProvider);
    if (!identical(patched.commands, current.commands)) {
      unawaited(cache.replaceCommands(roomId, patched.commands));
    }
    if (!identical(patched.proposals, current.proposals)) {
      unawaited(cache.replaceProposals(roomId, patched.proposals));
    }
    if (!identical(patched.executions, current.executions)) {
      unawaited(cache.replaceExecutions(roomId, patched.executions));
    }
    if (!identical(patched.snapshot, current.snapshot)) {
      unawaited(cache.saveSnapshot(roomId, patched.snapshot));
    }
    setState(() {});
    return true;
  }

  Future<void> _confirmDisconnect() async {
    final roomName = widget.room['name'] as String? ?? '관리 폴더';
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('관리 폴더 연결 해제'),
        content: Text('$roomName 연결을 해제합니다. PC의 원본 폴더와 파일은 삭제되지 않습니다.'),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(context, false),
            child: const Text('취소'),
          ),
          FilledButton(
            onPressed: () => Navigator.pop(context, true),
            child: const Text('연결 해제'),
          ),
        ],
      ),
    );
    if (confirmed != true) return;
    await ref
        .read(connectionGateControllerProvider.notifier)
        .disconnectRoom(widget.room['id'] as String);
  }

  @override
  Widget build(BuildContext context) {
    ref.listen(realtimeHomeUpdateProvider, (previous, next) {
      if (next == null || identical(previous, next) || !mounted) return;
      if (_applyRealtimeUpdate(next)) {
        _suppressNextRealtimeRevisionReload = true;
      }
    });
    ref.listen(realtimeRevisionProvider, (previous, next) {
      if (previous == null || !mounted) return;
      if (_suppressNextRealtimeRevisionReload) {
        _suppressNextRealtimeRevisionReload = false;
        return;
      }
      setState(_reload);
    });
    final roomId = widget.room['id'] as String;
    ref.listen(connectionGateControllerProvider, (previous, next) {
      final gate = next.asData?.value;
      if (gate == null || gate.rooms.any((room) => room['id'] == roomId)) {
        return;
      }
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted) Navigator.of(context).popUntil((route) => route.isFirst);
      });
    });
    final disconnect = ref
        .watch(connectionGateControllerProvider)
        .asData
        ?.value
        .operation(DisconnectKind.room, roomId);
    final roomName = widget.room['name'] as String? ?? '관리 폴더';
    return Scaffold(
      backgroundColor: const Color(0xFFD8C58F),
      appBar: AppBar(
        backgroundColor: _pixelGreen,
        foregroundColor: const Color(0xFFFFF4D1),
        shape: const Border(bottom: BorderSide(color: _pixelInk, width: 3)),
        title: Text(roomName),
        actions: [
          IconButton(
            tooltip: '파일 목록',
            icon: const Icon(Icons.folder_open),
            onPressed: disconnect == null
                ? () => Navigator.of(context).push(
                    MaterialPageRoute(
                      builder: (_) =>
                          FilesPage(roomId: roomId, roomName: roomName),
                    ),
                  )
                : null,
          ),
          IconButton(
            tooltip: 'AI 대화',
            icon: const Icon(Icons.chat_bubble_outline),
            onPressed: disconnect == null
                ? () => Navigator.of(context).push(
                    MaterialPageRoute(builder: (_) => ChatPage(roomId: roomId)),
                  )
                : null,
          ),
          IconButton(
            tooltip: '관리 폴더 연결 해제',
            onPressed: disconnect == null ? _confirmDisconnect : null,
            icon: disconnect?.phase == DisconnectPhase.disconnecting
                ? const SizedBox.square(
                    dimension: 20,
                    child: CheeseLoadingIndicator(size: 20),
                  )
                : const Icon(Icons.link_off),
          ),
        ],
      ),
      body: CheeseLoadingOverlay(
        loading: disconnect?.phase == DisconnectPhase.disconnecting,
        message: '관리 폴더 연결을 해제하는 중입니다',
        child: FutureBuilder<RoomContent>(
          future: _content,
          builder: (context, snapshot) {
            if (snapshot.connectionState != ConnectionState.done) {
              return const CheeseLoadingView(message: '관리 폴더 정보를 불러오는 중입니다');
            }
            if (snapshot.hasError) {
              return Center(
                child: Text(
                  '관리 폴더 정보를 불러오지 못했습니다.\n${snapshot.error}',
                  textAlign: TextAlign.center,
                ),
              );
            }
            final content = snapshot.data!;
            final history = _buildHistoryEntries(content, roomId);
            final analyzing = content.commands.any(
              (command) =>
                  ['DELIVERED', 'ANALYZING'].contains(command['status']),
            );
            return CheeseLoadingOverlay(
              loading: analyzing,
              message: '데스크탑이 관리 폴더를 분석하는 중입니다',
              child: RefreshIndicator(
                onRefresh: () async {
                  setState(_reload);
                  await _content;
                },
                child: ListView(
                  padding: const EdgeInsets.fromLTRB(16, 16, 16, 32),
                  children: [
                    if (disconnect != null) ...[
                      _RoomDisconnectCard(
                        operation: disconnect,
                        onRetry: () => ref
                            .read(connectionGateControllerProvider.notifier)
                            .retryDisconnect(DisconnectKind.room, roomId),
                      ),
                      const SizedBox(height: 12),
                    ],
                    if (content.isOffline) ...[
                      const _PixelCard(
                        color: Color(0xFFFFF3E0),
                        child: ListTile(
                          leading: Icon(Icons.cloud_off_outlined),
                          title: Text('오프라인 상태'),
                          subtitle: Text('마지막으로 동기화된 결과를 표시합니다.'),
                        ),
                      ),
                      const SizedBox(height: 12),
                    ],
                    if (analyzing) ...[
                      const _PixelCard(
                        child: ListTile(
                          leading: SizedBox(
                            width: 24,
                            height: 24,
                            child: CheeseLoadingIndicator(size: 24),
                          ),
                          title: Text('PC가 관리 폴더를 분석 중입니다'),
                          subtitle: Text('새 결과는 히스토리에 자동으로 반영됩니다.'),
                        ),
                      ),
                      const SizedBox(height: 12),
                    ],
                    CleanlinessCard(snapshot: content.snapshot),
                    const SizedBox(height: 16),
                    _PixelCard(
                      child: ListTile(
                        leading: const Icon(Icons.chat_bubble_outline),
                        title: const Text('파일 정리는 대화로 요청하세요'),
                        subtitle: const Text(
                          '수동 규칙/커맨드 선택은 숨겼습니다. AI 대화에서 실제 가능한 작업만 초안으로 제안합니다.',
                        ),
                        trailing: const Icon(Icons.chevron_right),
                        onTap: disconnect == null
                            ? () => Navigator.of(context).push(
                                MaterialPageRoute(
                                  builder: (_) => ChatPage(roomId: roomId),
                                ),
                              )
                            : null,
                      ),
                    ),
                    const SizedBox(height: 20),
                    Container(
                      padding: const EdgeInsets.symmetric(
                        horizontal: 10,
                        vertical: 6,
                      ),
                      decoration: const BoxDecoration(
                        color: _pixelGreen,
                        border: Border.fromBorderSide(
                          BorderSide(color: _pixelInk, width: 2),
                        ),
                      ),
                      child: Text(
                        '히스토리',
                        style: Theme.of(context).textTheme.titleLarge?.copyWith(
                          color: const Color(0xFFFFF4D1),
                          fontWeight: FontWeight.w900,
                        ),
                      ),
                    ),
                    const SizedBox(height: 8),
                    _HistoryTimeline(
                      entries: history,
                      emptyMessage: content.isOffline
                          ? '활동 기록은 온라인 상태에서 갱신됩니다.'
                          : '아직 기록된 활동이 없습니다.',
                      onOpenProposal: disconnect == null
                          ? (proposalId) async {
                              await Navigator.of(context).push(
                                MaterialPageRoute(
                                  builder: (_) => ProposalPage(
                                    proposalId: proposalId,
                                    roomId: roomId,
                                  ),
                                ),
                              );
                              if (mounted) setState(_reload);
                            }
                          : null,
                    ),
                  ],
                ),
              ),
            );
          },
        ),
      ),
    );
  }

  List<_HistoryEntry> _buildHistoryEntries(RoomContent content, String roomId) {
    final entries = <_HistoryEntry>[
      for (final proposal in content.proposals)
        _HistoryEntry(
          title: '승인 대기 제안 (${_dateLabel(_firstDate(proposal))})',
          subtitle: _proposalSubtitle(proposal),
          at: _firstDate(proposal),
          icon: Icons.pending_actions_outlined,
          proposalId: proposal['id'] as String?,
        ),
      for (final command in content.commands)
        _HistoryEntry(
          title: '명령 요청 (${_dateLabel(_firstDate(command))})',
          subtitle:
              '${command['intent'] as String? ?? 'COMMAND'} · ${command['status'] as String? ?? '상태 없음'}',
          at: _firstDate(command),
          icon: Icons.task_alt_outlined,
        ),
      for (final item in content.executions) _executionHistoryEntry(item),
      for (final event in content.activity)
        _HistoryEntry(
          title:
              '${_activityLabel(event['eventType'] as String?)} (${_dateLabel(_firstDate(event))})',
          subtitle: event['eventType'] as String? ?? 'activity',
          at: _firstDate(event),
          icon: Icons.history,
        ),
    ];
    entries.sort((a, b) {
      final left = a.at;
      final right = b.at;
      if (left == null && right == null) return 0;
      if (left == null) return 1;
      if (right == null) return -1;
      return right.compareTo(left);
    });
    return entries.take(30).toList(growable: false);
  }

  _HistoryEntry _executionHistoryEntry(Map<String, dynamic> item) {
    final execution = item['execution'] is Map
        ? Map<String, dynamic>.from(item['execution'] as Map)
        : <String, dynamic>{};
    final status = execution['status'] as String? ?? 'UNKNOWN';
    final at = _firstDate(execution);
    return _HistoryEntry(
      title: '최근 명령 실행 결과 (${_dateLabel(at)})',
      subtitle: _executionLabel(status),
      at: at,
      icon: status == 'SUCCEEDED'
          ? Icons.check_circle_outline
          : status == 'EXECUTING'
          ? Icons.pending_outlined
          : Icons.error_outline,
    );
  }

  DateTime? _firstDate(Map<String, dynamic> value) {
    for (final key in const [
      'finishedAt',
      'startedAt',
      'updatedAt',
      'createdAt',
      'occurredAt',
    ]) {
      final raw = value[key];
      if (raw is String) {
        final parsed = DateTime.tryParse(raw);
        if (parsed != null) return parsed.toLocal();
      }
    }
    return null;
  }

  String _dateLabel(DateTime? value) {
    if (value == null) return '일자 없음';
    String two(int number) => number.toString().padLeft(2, '0');
    return '${value.year}.${two(value.month)}.${two(value.day)} '
        '${two(value.hour)}:${two(value.minute)}';
  }

  String _proposalSubtitle(Map<String, dynamic> proposal) {
    final summary = proposal['summary'];
    if (summary is Map && summary['title'] is String) {
      return summary['title'] as String;
    }
    final itemCount = proposal['itemCount'];
    return itemCount is int ? '$itemCount개 항목 검토 필요' : '검토가 필요한 제안입니다';
  }

  String _executionLabel(String status) {
    return switch (status) {
      'SUCCEEDED' => '정리가 완료되었습니다',
      'PARTIALLY_SUCCEEDED' => '일부 작업만 완료되었습니다',
      'ROLLED_BACK' => '데스크탑에서 작업을 되돌렸습니다',
      'STALE' => '승인 뒤 파일이 변경되어 실행하지 않았습니다',
      'FAILED' => '작업을 완료하지 못했습니다',
      'EXECUTING' => '승인된 작업을 실행 중입니다',
      _ => '실행 상태: $status',
    };
  }

  String _activityLabel(String? eventType) {
    return switch (eventType) {
      'proposal.created' => 'PC가 정리 제안을 만들었습니다',
      'decision.created' => '제안에 승인 또는 거절 결정을 저장했습니다',
      'execution.completed' => '승인된 작업 실행이 끝났습니다',
      _ => 'MOUSEKEEPER 상태가 변경되었습니다',
    };
  }
}

class _HistoryEntry {
  const _HistoryEntry({
    required this.title,
    required this.subtitle,
    required this.icon,
    this.at,
    this.proposalId,
  });

  final String title;
  final String subtitle;
  final IconData icon;
  final DateTime? at;
  final String? proposalId;
}

class _HistoryTimeline extends StatelessWidget {
  const _HistoryTimeline({
    required this.entries,
    required this.emptyMessage,
    required this.onOpenProposal,
  });

  final List<_HistoryEntry> entries;
  final String emptyMessage;
  final ValueChanged<String>? onOpenProposal;

  @override
  Widget build(BuildContext context) {
    if (entries.isEmpty) {
      return _PixelCard(child: ListTile(title: Text(emptyMessage)));
    }
    return Column(
      children: [
        for (final entry in entries)
          _PixelCard(
            child: ListTile(
              leading: Icon(entry.icon),
              title: Text(entry.title),
              subtitle: Text(entry.subtitle),
              trailing: entry.proposalId != null
                  ? const Icon(Icons.chevron_right)
                  : null,
              onTap: entry.proposalId != null && onOpenProposal != null
                  ? () => onOpenProposal!(entry.proposalId!)
                  : null,
            ),
          ),
      ],
    );
  }
}

class _RoomDisconnectCard extends StatelessWidget {
  const _RoomDisconnectCard({required this.operation, required this.onRetry});

  final DisconnectOperation operation;
  final VoidCallback onRetry;

  @override
  Widget build(BuildContext context) => _PixelCard(
    color: operation.phase == DisconnectPhase.failed
        ? Theme.of(context).colorScheme.errorContainer
        : null,
    child: ListTile(
      leading: operation.phase == DisconnectPhase.disconnecting
          ? const CheeseLoadingIndicator(size: 28)
          : const Icon(Icons.error_outline),
      title: Text(
        operation.phase == DisconnectPhase.disconnecting
            ? '관리 폴더 연결을 해제하는 중'
            : '관리 폴더 연결 해제 실패',
      ),
      subtitle: operation.message == null ? null : Text(operation.message!),
      trailing: operation.phase == DisconnectPhase.failed
          ? TextButton(onPressed: onRetry, child: const Text('다시 시도'))
          : null,
    ),
  );
}

const supportedCleanlinessFormulaVersion = 'mousekeeper-cleanliness-v1';

bool cleanlinessFormulaMismatch(Map<String, dynamic>? snapshot) {
  final formulaVersion = snapshot?['formulaVersion'];
  return formulaVersion is String &&
      formulaVersion != supportedCleanlinessFormulaVersion;
}

String cleanlinessCalculatedAtLabel(Object? value) {
  if (value is! String) return '마지막 계산 시각 정보 없음';
  final parsed = DateTime.tryParse(value)?.toLocal();
  if (parsed == null) return '마지막 계산 시각 확인 불가';
  String two(int number) => number.toString().padLeft(2, '0');
  return '마지막 계산 ${parsed.year}.${two(parsed.month)}.${two(parsed.day)} '
      '${two(parsed.hour)}:${two(parsed.minute)}';
}

String cleanlinessReasonLabel(String code) => switch (code) {
  'UNORGANIZED_FILES' => '정리되지 않은 파일',
  'UNREADABLE_OR_UNSAFE_ENTRIES' => '읽을 수 없거나 안전하지 않은 항목',
  'PROPOSAL_CONFLICTS' => '정리 제안 충돌',
  _ => '기타 감점',
};

class CleanlinessCard extends StatelessWidget {
  const CleanlinessCard({super.key, required this.snapshot});

  final Map<String, dynamic>? snapshot;

  @override
  Widget build(BuildContext context) {
    final score = snapshot?['score'] as int?;
    final metrics = snapshot?['metrics'];
    if (score == null) {
      return const _PixelCard(
        child: ListTile(
          leading: Icon(Icons.auto_graph_outlined),
          title: Text('청결도 계산 전'),
          subtitle: Text('PC 에이전트가 폴더를 스캔하면 청결도가 표시됩니다.'),
        ),
      );
    }
    final formulaVersion = snapshot?['formulaVersion'] as String?;
    final calculatedAt = cleanlinessCalculatedAtLabel(
      snapshot?['calculatedAt'],
    );
    if (cleanlinessFormulaMismatch(snapshot)) {
      return _PixelCard(
        color: Theme.of(context).colorScheme.errorContainer,
        child: ListTile(
          leading: const Icon(Icons.system_update_alt),
          title: const Text('청결도 공식 업데이트 필요'),
          subtitle: Text(
            '지원 버전: $supportedCleanlinessFormulaVersion\n'
            '받은 버전: $formulaVersion\n$calculatedAt',
          ),
        ),
      );
    }
    final metricMap = metrics is Map
        ? Map<String, dynamic>.from(metrics)
        : const <String, dynamic>{};
    final deductions = (metricMap['deductions'] as List? ?? const <Object>[])
        .whereType<Map>()
        .map(Map<String, dynamic>.from)
        .toList(growable: false);
    final color = score >= 80
        ? _pixelGreen
        : score >= 50
        ? const Color(0xFFC58B32)
        : _pixelRust;
    final grade = score >= 80
        ? '깔끔해요'
        : score >= 50
        ? '정리 필요'
        : '많은 정리 필요';
    return _PixelCard(
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                Container(
                  width: 68,
                  height: 68,
                  alignment: Alignment.center,
                  decoration: BoxDecoration(
                    color: color,
                    border: Border.all(color: _pixelInk, width: 3),
                    boxShadow: const [
                      BoxShadow(color: _pixelInk, offset: Offset(4, 4)),
                    ],
                  ),
                  child: Text(
                    '$score',
                    style: Theme.of(context).textTheme.titleLarge?.copyWith(
                      color: const Color(0xFFFFF4D1),
                      fontWeight: FontWeight.w900,
                    ),
                  ),
                ),
                const SizedBox(width: 16),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        grade,
                        style: Theme.of(context).textTheme.titleMedium,
                      ),
                      const SizedBox(height: 4),
                      Text(calculatedAt),
                      Text(
                        formulaVersion == null
                            ? '기준 오류 · 공식 버전 정보 없음'
                            : '공식 $formulaVersion',
                      ),
                    ],
                  ),
                ),
              ],
            ),
            const SizedBox(height: 14),
            _PixelMeter(value: score / 100, color: color),
            if (deductions.isNotEmpty) ...[
              const Divider(height: 24),
              Text('감점 사유', style: Theme.of(context).textTheme.titleSmall),
              const SizedBox(height: 4),
              ...deductions.map((deduction) {
                final code = deduction['reasonCode'] as String? ?? 'UNKNOWN';
                final count = deduction['count'] as int? ?? 0;
                final points = deduction['points'] as int? ?? 0;
                return Padding(
                  padding: const EdgeInsets.symmetric(vertical: 3),
                  child: Row(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      const Icon(Icons.remove_circle_outline, size: 18),
                      const SizedBox(width: 8),
                      Expanded(
                        child: Text(
                          '${cleanlinessReasonLabel(code)} ($code) · '
                          '$count개 · -$points점',
                        ),
                      ),
                    ],
                  ),
                );
              }),
            ] else ...[
              const Divider(height: 24),
              const Text('감점 사유 없음'),
            ],
          ],
        ),
      ),
    );
  }
}
