import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:uuid/uuid.dart';
import 'package:dio/dio.dart';
import '../../core/network/api_client.dart';
import '../../core/sync/mutation_queue.dart';
import '../../core/sync/realtime_controller.dart';
import '../../storage/display_cache.dart';
import '../auth/connection_gate_controller.dart';
import '../proposals/proposal_page.dart';
import '../files/files_page.dart';
import '../files/smart_cache_page.dart';
import '../chat/chat_page.dart';
import '../rules/rules_page.dart';

class _RoomContent {
  const _RoomContent({
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

  _RoomContent copyWith({
    List<Map<String, dynamic>>? commands,
    List<Map<String, dynamic>>? proposals,
    List<Map<String, dynamic>>? executions,
  }) => _RoomContent(
    commands: commands ?? this.commands,
    proposals: proposals ?? this.proposals,
    executions: executions ?? this.executions,
    activity: activity,
    snapshot: snapshot,
    isOffline: isOffline,
  );
}

List<Map<String, dynamic>> patchCommandItemsForRealtimeUpdate({
  required List<Map<String, dynamic>> commands,
  required RealtimeHomeUpdate update,
  required String roomId,
}) {
  if (update.kind != RealtimeHomeUpdateKind.commandStatus ||
      update.roomId != roomId ||
      update.commandId == null ||
      update.commandStatus == null) {
    return commands;
  }
  final commandId = update.commandId!;
  final status = update.commandStatus!;
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
  if (update.kind != RealtimeHomeUpdateKind.proposalCreated ||
      update.roomId != roomId ||
      update.proposalId == null ||
      update.proposalStatus == null) {
    return proposals;
  }
  final proposalId = update.proposalId!;
  final status = update.proposalStatus!;
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
  late Future<_RoomContent> _content;
  _RoomContent? _latestContent;
  bool _suppressNextRealtimeRevisionReload = false;
  bool _submitting = false;
  @override
  void initState() {
    super.initState();
    _reload();
  }

  Future<_RoomContent> _load() async {
    final api = ref.read(apiClientProvider);
    final cache = ref.read(displayCacheProvider);
    final id = widget.room['id'] as String;
    try {
      final lists = await Future.wait([
        api.getList('/v1/rooms/$id/commands'),
        api.getList('/v1/rooms/$id/proposals/open'),
        api.getList('/v1/rooms/$id/executions'),
        api.getList('/v1/rooms/$id/activity?limit=20'),
      ]);
      final snapshot = await api.getNullable('/v1/rooms/$id/snapshots/latest');
      if (!_roomIsActive(id)) throw StateError('ROOM_REMOVED');
      await Future.wait([
        cache.replaceCommands(id, lists[0]),
        cache.replaceProposals(id, lists[1]),
        cache.replaceExecutions(id, lists[2]),
        cache.saveSnapshot(id, snapshot),
      ]);
      if (!_roomIsActive(id)) throw StateError('ROOM_REMOVED');
      return _RoomContent(
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
      return _RoomContent(
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
    _content = _load().then((content) {
      if (mounted) _latestContent = content;
      return content;
    });
  }

  bool _applyRealtimeUpdate(RealtimeHomeUpdate update) {
    final current = _latestContent;
    if (current == null) return false;
    final roomId = widget.room['id'] as String;
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
    if (identical(commands, current.commands) &&
        identical(proposals, current.proposals) &&
        identical(executions, current.executions)) {
      return false;
    }
    final patched = current.copyWith(
      commands: commands,
      proposals: proposals,
      executions: executions,
    );
    _latestContent = patched;
    _content = Future.value(patched);
    final cache = ref.read(displayCacheProvider);
    if (!identical(commands, current.commands)) {
      unawaited(cache.replaceCommands(roomId, commands));
    }
    if (!identical(proposals, current.proposals)) {
      unawaited(cache.replaceProposals(roomId, proposals));
    }
    if (!identical(executions, current.executions)) {
      unawaited(cache.replaceExecutions(roomId, executions));
    }
    setState(() {});
    return true;
  }

  Future<void> _createCommand() async {
    final roomId = widget.room['id'] as String;
    final gate = ref.read(connectionGateControllerProvider).asData?.value;
    if (gate == null ||
        gate.operation(DisconnectKind.room, roomId) != null ||
        !gate.rooms.any((room) => room['id'] == roomId)) {
      return;
    }
    setState(() => _submitting = true);
    try {
      final id = roomId;
      final result = await ref
          .read(mutationQueueProvider)
          .postOrQueue(
            mutationType: 'CREATE_COMMAND',
            path: '/v1/rooms/$id/commands',
            body: {'intent': 'ANALYZE', 'payload': <String, dynamic>{}},
            idempotencyKey: const Uuid().v4(),
            roomId: id,
          );
      if (result.queued) {
        if (mounted) {
          ScaffoldMessenger.of(context).showSnackBar(
            const SnackBar(content: Text('오프라인 요청함에 저장했습니다. 연결되면 자동 전송됩니다.')),
          );
        }
      } else {
        setState(_reload);
      }
    } catch (error) {
      if (mounted) {
        ScaffoldMessenger.of(
          context,
        ).showSnackBar(SnackBar(content: Text('명령 생성 실패: $error')));
      }
    } finally {
      if (mounted) setState(() => _submitting = false);
    }
  }

  Future<void> _confirmDisconnect() async {
    final roomName = widget.room['name'] as String? ?? '방';
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('폴더 연결 해제'),
        content: Text('$roomName 연결을 해제합니다. PC의 원본 폴더와 파일은 그대로 유지됩니다.'),
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
    return Scaffold(
      appBar: AppBar(
        title: Text(widget.room['name'] as String? ?? '방'),
        actions: [
          IconButton(
            tooltip: '정리 규칙',
            icon: const Icon(Icons.rule_folder_outlined),
            onPressed: disconnect == null
                ? () => Navigator.of(context).push(
                    MaterialPageRoute(
                      builder: (_) =>
                          RulesPage(roomId: widget.room['id'] as String),
                    ),
                  )
                : null,
          ),
          IconButton(
            tooltip: '채팅',
            icon: const Icon(Icons.chat_bubble_outline),
            onPressed: disconnect == null
                ? () => Navigator.of(context).push(
                    MaterialPageRoute(
                      builder: (_) =>
                          ChatPage(roomId: widget.room['id'] as String),
                    ),
                  )
                : null,
          ),
          IconButton(
            tooltip: '스마트 캐시',
            icon: const Icon(Icons.offline_bolt_outlined),
            onPressed: disconnect == null
                ? () => Navigator.of(context).push(
                    MaterialPageRoute(
                      builder: (_) =>
                          SmartCachePage(roomId: widget.room['id'] as String),
                    ),
                  )
                : null,
          ),
          IconButton(
            tooltip: '온라인 파일',
            icon: const Icon(Icons.folder_open),
            onPressed: disconnect == null
                ? () => Navigator.of(context).push(
                    MaterialPageRoute(
                      builder: (_) => FilesPage(
                        roomId: roomId,
                        roomName: widget.room['name'] as String? ?? '방',
                      ),
                    ),
                  )
                : null,
          ),
          IconButton(
            tooltip: '폴더 연결 해제',
            onPressed: disconnect == null ? _confirmDisconnect : null,
            icon: disconnect?.phase == DisconnectPhase.disconnecting
                ? const SizedBox.square(
                    dimension: 20,
                    child: CircularProgressIndicator(strokeWidth: 2),
                  )
                : const Icon(Icons.link_off),
          ),
        ],
      ),
      floatingActionButton: FloatingActionButton.extended(
        onPressed: _submitting || disconnect != null ? null : _createCommand,
        icon: const Icon(Icons.auto_fix_high),
        label: const Text('정리 제안 요청'),
      ),
      body: FutureBuilder<_RoomContent>(
        future: _content,
        builder: (context, snapshot) {
          if (snapshot.connectionState != ConnectionState.done) {
            return const Center(child: CircularProgressIndicator());
          }
          if (snapshot.hasError) {
            return Center(
              child: Text(
                '방 정보를 불러오지 못했습니다.\n${snapshot.error}',
                textAlign: TextAlign.center,
              ),
            );
          }
          final content = snapshot.data!;
          final commands = content.commands;
          final proposals = content.proposals;
          final executions = content.executions;
          final activity = content.activity;
          final analyzing = commands.any(
            (command) => ['DELIVERED', 'ANALYZING'].contains(command['status']),
          );
          return RefreshIndicator(
            onRefresh: () async {
              setState(_reload);
              await _content;
            },
            child: ListView(
              padding: const EdgeInsets.fromLTRB(16, 16, 16, 96),
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
                  const Card(
                    color: Color(0xFFFFF3E0),
                    child: ListTile(
                      leading: Icon(Icons.cloud_off_outlined),
                      title: Text('오프라인 상태'),
                      subtitle: Text('마지막 동기화 결과를 표시합니다.'),
                    ),
                  ),
                  const SizedBox(height: 12),
                ],
                if (analyzing) ...[
                  const Card(
                    child: ListTile(
                      leading: SizedBox(
                        width: 24,
                        height: 24,
                        child: CircularProgressIndicator(strokeWidth: 3),
                      ),
                      title: Text('PC가 폴더를 분석 중입니다'),
                      subtitle: Text('새 제안이 준비되면 자동으로 갱신됩니다.'),
                    ),
                  ),
                  const SizedBox(height: 12),
                ],
                CleanlinessCard(snapshot: content.snapshot),
                const SizedBox(height: 20),
                Text('승인 대기 제안', style: Theme.of(context).textTheme.titleLarge),
                const SizedBox(height: 8),
                if (proposals.isEmpty)
                  const Card(child: ListTile(title: Text('승인 대기 중인 제안이 없습니다')))
                else
                  ...proposals.map(
                    (proposal) => Card(
                      child: ListTile(
                        title: const Text('파일 정리 제안'),
                        subtitle: Text(proposal['status'] as String? ?? ''),
                        trailing: const Icon(Icons.chevron_right),
                        onTap: disconnect == null
                            ? () async {
                                await Navigator.of(context).push(
                                  MaterialPageRoute(
                                    builder: (_) => ProposalPage(
                                      proposalId: proposal['id'] as String,
                                      roomId: roomId,
                                    ),
                                  ),
                                );
                                if (mounted) setState(_reload);
                              }
                            : null,
                      ),
                    ),
                  ),
                const SizedBox(height: 20),
                Text('최근 명령', style: Theme.of(context).textTheme.titleLarge),
                const SizedBox(height: 8),
                if (commands.isEmpty)
                  const Card(child: ListTile(title: Text('아직 요청한 명령이 없습니다')))
                else
                  ...commands.reversed
                      .take(20)
                      .map(
                        (command) => Card(
                          child: ListTile(
                            leading: const Icon(Icons.task_alt),
                            title: Text(command['intent'] as String? ?? '명령'),
                            subtitle: Text(
                              command['status'] as String? ?? '상태 없음',
                            ),
                          ),
                        ),
                      ),
                const SizedBox(height: 20),
                Text('최근 실행 결과', style: Theme.of(context).textTheme.titleLarge),
                const SizedBox(height: 8),
                if (executions.isEmpty)
                  const Card(child: ListTile(title: Text('아직 실행 결과가 없습니다')))
                else
                  ...executions.take(10).map((item) {
                    final execution = Map<String, dynamic>.from(
                      item['execution'] as Map,
                    );
                    final status = execution['status'] as String? ?? '상태 없음';
                    return Card(
                      child: ListTile(
                        leading: Icon(
                          status == 'SUCCEEDED'
                              ? Icons.check_circle_outline
                              : status == 'EXECUTING'
                              ? Icons.pending_outlined
                              : Icons.error_outline,
                        ),
                        title: Text(_executionLabel(status)),
                        subtitle: Text(
                          execution['finishedAt'] as String? ??
                              execution['startedAt'] as String? ??
                              '',
                        ),
                      ),
                    );
                  }),
                const SizedBox(height: 20),
                Text('활동 기록', style: Theme.of(context).textTheme.titleLarge),
                const SizedBox(height: 8),
                if (activity.isEmpty)
                  Card(
                    child: ListTile(
                      title: Text(
                        content.isOffline
                            ? '활동 기록은 온라인에서 갱신됩니다'
                            : '아직 기록된 활동이 없습니다',
                      ),
                    ),
                  )
                else
                  ...activity
                      .take(20)
                      .map(
                        (event) => ListTile(
                          dense: true,
                          leading: const Icon(Icons.history),
                          title: Text(
                            _activityLabel(event['eventType'] as String?),
                          ),
                          subtitle: Text(event['occurredAt'] as String? ?? ''),
                        ),
                      ),
              ],
            ),
          );
        },
      ),
    );
  }

  String _executionLabel(String status) {
    return switch (status) {
      'SUCCEEDED' => '정리가 완료되었습니다',
      'PARTIALLY_SUCCEEDED' => '일부 작업만 완료되었습니다',
      'ROLLED_BACK' => '데스크톱에서 작업을 되돌렸습니다',
      'STALE' => '승인 후 파일이 변경되어 실행하지 않았습니다',
      'FAILED' => '작업을 완료하지 못했습니다',
      _ => '승인된 작업을 실행 중입니다',
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

class _RoomDisconnectCard extends StatelessWidget {
  const _RoomDisconnectCard({required this.operation, required this.onRetry});

  final DisconnectOperation operation;
  final VoidCallback onRetry;

  @override
  Widget build(BuildContext context) => Card(
    color: operation.phase == DisconnectPhase.failed
        ? Theme.of(context).colorScheme.errorContainer
        : null,
    child: ListTile(
      leading: operation.phase == DisconnectPhase.disconnecting
          ? const CircularProgressIndicator()
          : const Icon(Icons.error_outline),
      title: Text(
        operation.phase == DisconnectPhase.disconnecting
            ? '폴더 연결을 해제하는 중'
            : '폴더 연결 해제 실패',
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
      return const Card(
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
      return Card(
        color: Theme.of(context).colorScheme.errorContainer,
        child: ListTile(
          leading: const Icon(Icons.system_update_alt),
          title: const Text('청결도 공식 업데이트 필요'),
          subtitle: Text(
            '앱 지원 버전: $supportedCleanlinessFormulaVersion\n'
            '스냅샷 버전: $formulaVersion\n$calculatedAt',
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
        ? Colors.green
        : score >= 50
        ? Colors.orange
        : Colors.red;
    final grade = score >= 80
        ? '깨끗함'
        : score >= 50
        ? '정리 필요'
        : '많은 정리 필요';
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                SizedBox(
                  width: 68,
                  height: 68,
                  child: Stack(
                    alignment: Alignment.center,
                    children: [
                      CircularProgressIndicator(
                        value: score / 100,
                        strokeWidth: 7,
                        color: color,
                      ),
                      Text(
                        '$score',
                        style: Theme.of(context).textTheme.titleLarge,
                      ),
                    ],
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
                            ? '기존 스냅샷 · 공식 버전 정보 없음'
                            : '공식 $formulaVersion',
                      ),
                    ],
                  ),
                ),
              ],
            ),
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
