import 'package:flutter/material.dart';

import '../auth/pairing_page.dart';

class MouseKeeperSettingsPage extends StatelessWidget {
  const MouseKeeperSettingsPage({
    super.key,
    required this.devices,
    required this.rooms,
  });

  final List<Map<String, dynamic>> devices;
  final List<Map<String, dynamic>> rooms;

  @override
  Widget build(BuildContext context) => Scaffold(
    appBar: AppBar(title: const Text('설정')),
    body: ListView(
      padding: const EdgeInsets.all(16),
      children: [
        _SettingsSection(
          icon: Icons.computer_outlined,
          title: '연결된 PC',
          subtitle: devices.isEmpty
              ? '아직 연결된 PC가 없습니다.'
              : '${devices.length}대의 PC가 연결되어 있습니다.',
          children: devices.isEmpty
              ? const [_EmptySettingsText('데스크톱 앱에서 6자리 코드를 만든 뒤 페어링해 주세요.')]
              : [
                  for (final device in devices)
                    _SettingsInfoTile(
                      icon: Icons.desktop_windows_outlined,
                      title: _deviceName(device),
                      subtitle: _presenceLabel(device['presence'] as String?),
                    ),
                ],
        ),
        const SizedBox(height: 14),
        _SettingsSection(
          icon: Icons.link_outlined,
          title: '페어링 설정',
          subtitle: '새 PC 연결은 데스크톱 앱의 6자리 코드로 진행합니다.',
          children: [
            FilledButton.icon(
              onPressed: () => Navigator.of(
                context,
              ).push(MaterialPageRoute(builder: (_) => const PairingPage())),
              icon: const Icon(Icons.add_link_outlined),
              label: const Text('새 PC 페어링'),
            ),
          ],
        ),
        const SizedBox(height: 14),
        _SettingsSection(
          icon: Icons.folder_special_outlined,
          title: '연결된 폴더',
          subtitle: rooms.isEmpty
              ? '아직 연결된 관리 폴더가 없습니다.'
              : '${rooms.length}개의 관리 폴더가 연결되어 있습니다.',
          children: rooms.isEmpty
              ? const [_EmptySettingsText('PC에서 관리 폴더를 등록하면 여기에 표시됩니다.')]
              : [
                  for (final room in rooms)
                    _SettingsInfoTile(
                      icon: Icons.folder_outlined,
                      title: room['name'] as String? ?? '관리 폴더',
                      subtitle: _roomSubtitle(room),
                    ),
                ],
        ),
      ],
    ),
  );

  static String _deviceName(Map<String, dynamic> device) =>
      device['deviceName'] as String? ??
      device['name'] as String? ??
      'MouseKeeper Desktop';

  static String _presenceLabel(String? presence) => switch (presence) {
    'ONLINE_IDLE' => '온라인 · 대기 중',
    'ONLINE_EXECUTING' => '온라인 · 작업 중',
    'OFFLINE' => '오프라인',
    null => '상태 확인 중',
    _ => presence,
  };

  static String _roomSubtitle(Map<String, dynamic> room) {
    final deviceName = room['deviceName'];
    final cleanliness = room['cleanlinessScore'];
    final parts = <String>[
      if (deviceName is String && deviceName.isNotEmpty) deviceName,
      if (cleanliness is num) '청결도 ${cleanliness.round()}점',
    ];
    return parts.isEmpty ? '연결됨' : parts.join(' · ');
  }
}

class _SettingsSection extends StatelessWidget {
  const _SettingsSection({
    required this.icon,
    required this.title,
    required this.subtitle,
    required this.children,
  });

  final IconData icon;
  final String title;
  final String subtitle;
  final List<Widget> children;

  @override
  Widget build(BuildContext context) => Card(
    child: Padding(
      padding: const EdgeInsets.all(16),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Icon(icon),
              const SizedBox(width: 12),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(title, style: Theme.of(context).textTheme.titleMedium),
                    const SizedBox(height: 2),
                    Text(
                      subtitle,
                      style: Theme.of(context).textTheme.bodySmall?.copyWith(
                        color: Theme.of(context).colorScheme.outline,
                      ),
                    ),
                  ],
                ),
              ),
            ],
          ),
          const SizedBox(height: 12),
          ...children,
        ],
      ),
    ),
  );
}

class _SettingsInfoTile extends StatelessWidget {
  const _SettingsInfoTile({
    required this.icon,
    required this.title,
    required this.subtitle,
  });

  final IconData icon;
  final String title;
  final String subtitle;

  @override
  Widget build(BuildContext context) => ListTile(
    contentPadding: EdgeInsets.zero,
    leading: Icon(icon),
    title: Text(title),
    subtitle: Text(subtitle),
  );
}

class _EmptySettingsText extends StatelessWidget {
  const _EmptySettingsText(this.text);

  final String text;

  @override
  Widget build(BuildContext context) => Padding(
    padding: const EdgeInsets.symmetric(vertical: 8),
    child: Text(
      text,
      style: Theme.of(context).textTheme.bodyMedium?.copyWith(
        color: Theme.of(context).colorScheme.outline,
      ),
    ),
  );
}
