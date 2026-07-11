import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import '../../core/network/api_client.dart';
import 'readme_command_page.dart';

class ChatPage extends ConsumerStatefulWidget {
  const ChatPage({super.key, required this.roomId});
  final String roomId;
  @override
  ConsumerState<ChatPage> createState() => _ChatPageState();
}

class _ChatPageState extends ConsumerState<ChatPage> {
  final input = TextEditingController();
  late Future<List<Map<String, dynamic>>> messages;
  bool sending = false;
  @override
  void initState() {
    super.initState();
    reload();
  }

  void reload() => messages = ref
      .read(apiClientProvider)
      .getList('/v1/rooms/${widget.roomId}/chat');
  @override
  void dispose() {
    input.dispose();
    super.dispose();
  }

  Future<void> send() async {
    final content = input.text.trim();
    if (content.isEmpty) return;
    setState(() => sending = true);
    try {
      final result = await ref.read(apiClientProvider).post(
        '/v1/rooms/${widget.roomId}/chat',
        {'content': content},
      );
      input.clear();
      setState(reload);
      if (result['aiStatus'] == 'UNCONFIGURED' && mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(content: Text('AI가 아직 설정되지 않아 메시지만 저장했습니다.')),
        );
      }
    } catch (error) {
      if (mounted) {
        ScaffoldMessenger.of(
          context,
        ).showSnackBar(SnackBar(content: Text('메시지 저장 실패: $error')));
      }
    } finally {
      if (mounted) setState(() => sending = false);
    }
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
        Expanded(
          child: FutureBuilder<List<Map<String, dynamic>>>(
            future: messages,
            builder: (context, snapshot) {
              if (snapshot.connectionState != ConnectionState.done) {
                return const Center(child: CircularProgressIndicator());
              }
              if (snapshot.hasError) {
                return Center(
                  child: Text(
                    '채팅을 불러오지 못했습니다.\n${snapshot.error}',
                    textAlign: TextAlign.center,
                  ),
                );
              }
              final items = snapshot.data!;
              if (items.isEmpty) {
                return const Center(
                  child: Text(
                    '아직 메시지가 없습니다.\n정리 요청은 방 화면의 명령 버튼을 사용하세요.',
                    textAlign: TextAlign.center,
                  ),
                );
              }
              return ListView.builder(
                padding: const EdgeInsets.all(16),
                itemCount: items.length,
                itemBuilder: (context, index) => Align(
                  alignment: items[index]['senderType'] == 'USER'
                      ? Alignment.centerRight
                      : Alignment.centerLeft,
                  child: Card(
                    child: Padding(
                      padding: const EdgeInsets.all(12),
                      child: Text(items[index]['content'] as String),
                    ),
                  ),
                ),
              );
            },
          ),
        ),
        SafeArea(
          child: Padding(
            padding: const EdgeInsets.all(12),
            child: Row(
              children: [
                Expanded(
                  child: TextField(
                    controller: input,
                    maxLength: 2000,
                    decoration: const InputDecoration(
                      hintText: '메시지',
                      border: OutlineInputBorder(),
                    ),
                  ),
                ),
                const SizedBox(width: 8),
                IconButton(
                  onPressed: sending ? null : send,
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
