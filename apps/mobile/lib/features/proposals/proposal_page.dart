import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:uuid/uuid.dart';
import '../../core/network/api_client.dart';
import '../../core/sync/mutation_queue.dart';

class ProposalPage extends ConsumerStatefulWidget {
  const ProposalPage({super.key, required this.proposalId});
  final String proposalId;
  @override
  ConsumerState<ProposalPage> createState() => _ProposalPageState();
}

class _ProposalPageState extends ConsumerState<ProposalPage> {
  late Future<Map<String, dynamic>> _proposal;
  bool _submitting = false;
  @override
  void initState() {
    super.initState();
    _proposal = ref
        .read(apiClientProvider)
        .get('/v1/proposals/${widget.proposalId}');
  }

  Future<void> _decide(String type, List<dynamic> items) async {
    setState(() => _submitting = true);
    try {
      final result = await ref
          .read(mutationQueueProvider)
          .postOrQueue(
            mutationType: 'CREATE_DECISION',
            path: '/v1/proposals/${widget.proposalId}/decisions',
            body: {
              'decisionType': type,
              'approvedItemIds': type == 'APPROVE'
                  ? items
                        .map((item) => (item as Map<String, dynamic>)['id'])
                        .toList()
                  : <String>[],
            },
            idempotencyKey: const Uuid().v4(),
          );
      if (mounted) {
        if (result.queued) {
          ScaffoldMessenger.of(context).showSnackBar(
            const SnackBar(content: Text('결정을 오프라인 요청함에 저장했습니다.')),
          );
        }
        Navigator.of(context).pop();
      }
    } catch (error) {
      if (mounted) {
        ScaffoldMessenger.of(
          context,
        ).showSnackBar(SnackBar(content: Text('결정 저장 실패: $error')));
      }
    } finally {
      if (mounted) setState(() => _submitting = false);
    }
  }

  @override
  Widget build(BuildContext context) => Scaffold(
    appBar: AppBar(title: const Text('제안 검토')),
    body: FutureBuilder<Map<String, dynamic>>(
      future: _proposal,
      builder: (context, snapshot) {
        if (snapshot.connectionState != ConnectionState.done) {
          return const Center(child: CircularProgressIndicator());
        }
        if (snapshot.hasError) {
          return Center(
            child: Text(
              '제안을 불러오지 못했습니다.\n${snapshot.error}',
              textAlign: TextAlign.center,
            ),
          );
        }
        final items = snapshot.data!['items'] as List<dynamic>? ?? const [];
        final summary = Map<String, dynamic>.from(
          snapshot.data!['summary'] as Map? ?? const {},
        );
        return Column(
          children: [
            if (ProposalSummaryCard.hasReadmePreview(summary))
              Padding(
                padding: const EdgeInsets.fromLTRB(16, 16, 16, 0),
                child: ProposalSummaryCard(summary: summary),
              ),
            Expanded(child: ProposalItemsList(items: items)),
            SafeArea(
              child: Padding(
                padding: const EdgeInsets.all(16),
                child: Row(
                  children: [
                    Expanded(
                      child: OutlinedButton(
                        onPressed: _submitting
                            ? null
                            : () => _decide('REJECT', items),
                        child: const Text('거절'),
                      ),
                    ),
                    const SizedBox(width: 12),
                    Expanded(
                      child: FilledButton(
                        onPressed: _submitting || items.isEmpty
                            ? null
                            : () => _decide('APPROVE', items),
                        child: const Text('전체 승인'),
                      ),
                    ),
                  ],
                ),
              ),
            ),
          ],
        );
      },
    ),
  );
}

class ProposalSummaryCard extends StatelessWidget {
  const ProposalSummaryCard({super.key, required this.summary});
  final Map<String, dynamic> summary;

  static bool hasReadmePreview(Map<String, dynamic> summary) =>
      summary['readmeDraft'] is String || summary['readmeDiff'] is String;

  @override
  Widget build(BuildContext context) {
    final draft = summary['readmeDraft'] as String?;
    final diff = summary['readmeDiff'] as String?;
    return Card(
      color: const Color(0xFFF5F5F5),
      child: ExpansionTile(
        initiallyExpanded: true,
        leading: const Icon(Icons.difference_outlined),
        title: const Text('README 초안과 실제 diff'),
        subtitle: const Text('PC가 읽은 현재 파일을 기준으로 만든 검토용 내용입니다.'),
        children: [
          ConstrainedBox(
            constraints: const BoxConstraints(maxHeight: 280),
            child: SingleChildScrollView(
              padding: const EdgeInsets.fromLTRB(16, 0, 16, 16),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  if (diff != null) ...[
                    Text('변경점', style: Theme.of(context).textTheme.titleSmall),
                    const SizedBox(height: 6),
                    SelectableText(diff),
                  ],
                  if (draft != null) ...[
                    const SizedBox(height: 12),
                    Text(
                      '완성 초안',
                      style: Theme.of(context).textTheme.titleSmall,
                    ),
                    const SizedBox(height: 6),
                    SelectableText(draft),
                  ],
                ],
              ),
            ),
          ),
        ],
      ),
    );
  }
}

class ProposalItemsList extends StatelessWidget {
  const ProposalItemsList({super.key, required this.items});
  final List<dynamic> items;

  @override
  Widget build(BuildContext context) {
    if (items.isEmpty) return const Center(child: Text('제안 항목이 없습니다'));
    return ListView.builder(
      padding: const EdgeInsets.all(16),
      itemCount: items.length,
      itemBuilder: (context, index) {
        final item = Map<String, dynamic>.from(items[index] as Map);
        final conflict = item['conflictState'] as String? ?? 'NONE';
        return Card(
          child: ListTile(
            title: Text(item['actionType'] as String? ?? '작업'),
            subtitle: Text(
              '${item['sourceRelativePath'] ?? ''}\n'
              '→ ${item['destinationRelativePath'] ?? 'MOUSEKEEPER 휴지통'}\n'
              '이유: ${item['reasonCode'] ?? '정리 규칙 일치'}'
              '${conflict == 'NONE' ? '' : '\n충돌: $conflict'}',
            ),
          ),
        );
      },
    );
  }
}
