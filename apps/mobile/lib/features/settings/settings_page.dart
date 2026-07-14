import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../core/notifications/push_notifications.dart';
import '../auth/auth_controller.dart';
import '../auth/connection_gate_controller.dart';

const _ink = Color(0xFF3B2A24);
const _paper = Color(0xFFFFFAF4);
const _paperMuted = Color(0xFFF3E8DC);
const _line = Color(0xFFB9A696);
const _danger = Color(0xFFA83B35);

class MouseKeeperSettingsPage extends ConsumerWidget {
  const MouseKeeperSettingsPage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final gate = ref.watch(connectionGateControllerProvider);
    final data = gate.asData?.value;
    final device = data?.devices.firstOrNull;
    final rooms = data?.rooms ?? const <Map<String, dynamic>>[];
    final deviceId = device?['id'] as String?;
    final operation = deviceId == null
        ? null
        : data?.operation(DisconnectKind.device, deviceId);
    final isDisconnecting = operation?.phase == DisconnectPhase.disconnecting;
    final disconnectFailed = operation?.phase == DisconnectPhase.failed;

    return Scaffold(
      backgroundColor: const Color(0xFFF4E9DC),
      appBar: AppBar(
        title: const Text(
          'MOUSEKEEPER 설정',
          style: TextStyle(fontWeight: FontWeight.w900, letterSpacing: -0.4),
        ),
      ),
      body: gate.isLoading && data == null
          ? const Center(child: CircularProgressIndicator())
          : ListView(
              padding: const EdgeInsets.fromLTRB(18, 12, 18, 32),
              children: [
                const _SettingsHeading(
                  eyebrow: 'CONNECTION',
                  title: '지금 연결된 공간',
                  description: '모바일은 한 번에 한 대의 데스크탑과 연결됩니다.',
                ),
                const SizedBox(height: 14),
                _PixelPanel(
                  child: device == null
                      ? const _EmptySettingsText('현재 연결된 데스크탑이 없습니다.')
                      : Column(
                          crossAxisAlignment: CrossAxisAlignment.stretch,
                          children: [
                            _SettingsInfoTile(
                              icon: Icons.desktop_windows_outlined,
                              title: _deviceName(device),
                              subtitle: _presenceLabel(
                                device['presence'] as String?,
                              ),
                              trailing: _PresenceDot(
                                online: device['presence'] != 'OFFLINE',
                              ),
                            ),
                            const Divider(height: 26),
                            Text(
                              '다른 PC를 연결하려면 먼저 현재 페어링을 끊어야 합니다.',
                              style: Theme.of(context).textTheme.bodySmall
                                  ?.copyWith(color: const Color(0xFF77675D)),
                            ),
                            const SizedBox(height: 12),
                            OutlinedButton.icon(
                              key: const ValueKey('disconnect-paired-desktop'),
                              onPressed: isDisconnecting
                                  ? null
                                  : () => disconnectFailed
                                        ? ref
                                              .read(
                                                connectionGateControllerProvider
                                                    .notifier,
                                              )
                                              .retryDisconnect(
                                                DisconnectKind.device,
                                                deviceId!,
                                              )
                                        : _confirmDisconnect(
                                            context,
                                            ref,
                                            deviceId!,
                                            _deviceName(device),
                                          ),
                              icon: isDisconnecting
                                  ? const SizedBox.square(
                                      dimension: 18,
                                      child: CircularProgressIndicator(
                                        strokeWidth: 2,
                                      ),
                                    )
                                  : Icon(
                                      disconnectFailed
                                          ? Icons.refresh
                                          : Icons.link_off,
                                    ),
                              label: Text(
                                isDisconnecting
                                    ? '페어링 끊는 중…'
                                    : disconnectFailed
                                    ? '페어링 끊기 다시 시도'
                                    : '페어링 끊기',
                              ),
                              style: OutlinedButton.styleFrom(
                                foregroundColor: _danger,
                                side: const BorderSide(
                                  color: _danger,
                                  width: 1.5,
                                ),
                                padding: const EdgeInsets.symmetric(
                                  vertical: 14,
                                ),
                              ),
                            ),
                            if (disconnectFailed) ...[
                              const SizedBox(height: 8),
                              Text(
                                operation?.message ?? '연결 해제에 실패했습니다.',
                                style: Theme.of(
                                  context,
                                ).textTheme.bodySmall?.copyWith(color: _danger),
                              ),
                            ],
                          ],
                        ),
                ),
                const SizedBox(height: 24),
                const _SettingsHeading(
                  eyebrow: 'MANAGED FOLDERS',
                  title: '연결된 관리 폴더',
                  description: 'PC에서 등록한 폴더만 MouseKeeper가 안전하게 관리합니다.',
                ),
                const SizedBox(height: 14),
                _PixelPanel(
                  child: rooms.isEmpty
                      ? const _EmptySettingsText('PC에서 관리 폴더를 등록하면 여기에 표시됩니다.')
                      : Column(
                          children: [
                            for (
                              var index = 0;
                              index < rooms.length;
                              index++
                            ) ...[
                              _SettingsInfoTile(
                                icon: Icons.folder_outlined,
                                title:
                                    rooms[index]['name'] as String? ?? '관리 폴더',
                                subtitle: _roomSubtitle(rooms[index]),
                              ),
                              if (index != rooms.length - 1)
                                const Divider(height: 20),
                            ],
                          ],
                        ),
                ),
                const SizedBox(height: 24),
                _PixelPanel(
                  child: TextButton.icon(
                    onPressed: () => _signOut(context, ref),
                    icon: const Icon(Icons.logout),
                    label: const Text('로그아웃'),
                  ),
                ),
              ],
            ),
    );
  }

  static Future<void> _confirmDisconnect(
    BuildContext context,
    WidgetRef ref,
    String deviceId,
    String deviceName,
  ) async {
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (dialogContext) => AlertDialog(
        title: const Text('PC 페어링을 끊을까요?'),
        content: Text(
          '$deviceName 연결과 관리 폴더 표시가 해제됩니다. '
          'PC의 원본 폴더와 파일은 삭제되지 않습니다.',
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(dialogContext).pop(false),
            child: const Text('취소'),
          ),
          FilledButton(
            onPressed: () => Navigator.of(dialogContext).pop(true),
            style: FilledButton.styleFrom(backgroundColor: _danger),
            child: const Text('페어링 끊기'),
          ),
        ],
      ),
    );
    if (confirmed != true || !context.mounted) return;
    await ref
        .read(connectionGateControllerProvider.notifier)
        .disconnectDevice(deviceId);
  }

  static Future<void> _signOut(BuildContext context, WidgetRef ref) async {
    try {
      await ref.read(pushNotificationsProvider.notifier).unregister();
      await ref.read(authControllerProvider.notifier).signOut();
    } catch (error) {
      if (!context.mounted) return;
      ScaffoldMessenger.of(
        context,
      ).showSnackBar(SnackBar(content: Text('로그아웃 준비에 실패했습니다: $error')));
    }
  }

  static String _deviceName(Map<String, dynamic> device) =>
      device['deviceName'] as String? ??
      device['name'] as String? ??
      'MouseKeeper Desktop';

  static String _presenceLabel(String? presence) => switch (presence) {
    'ONLINE_IDLE' => '온라인 · 대기 중',
    'ONLINE_EXECUTING' => '온라인 · 작업 중',
    'DEGRADED' => '연결 불안정',
    'OFFLINE' => '연결 끊김',
    null => '상태 확인 중',
    _ => presence,
  };

  static String _roomSubtitle(Map<String, dynamic> room) {
    final cleanliness = room['cleanlinessScore'];
    return cleanliness is num ? '청결도 ${cleanliness.round()}점' : '연결됨';
  }
}

class _SettingsHeading extends StatelessWidget {
  const _SettingsHeading({
    required this.eyebrow,
    required this.title,
    required this.description,
  });

  final String eyebrow;
  final String title;
  final String description;

  @override
  Widget build(BuildContext context) => Column(
    crossAxisAlignment: CrossAxisAlignment.start,
    children: [
      Text(
        eyebrow,
        style: Theme.of(context).textTheme.labelSmall?.copyWith(
          color: const Color(0xFF9A6249),
          fontWeight: FontWeight.w900,
          letterSpacing: 1.2,
        ),
      ),
      const SizedBox(height: 3),
      Text(
        title,
        style: Theme.of(
          context,
        ).textTheme.titleLarge?.copyWith(fontWeight: FontWeight.w900),
      ),
      const SizedBox(height: 4),
      Text(
        description,
        style: Theme.of(context).textTheme.bodySmall?.copyWith(
          color: const Color(0xFF77675D),
          height: 1.4,
        ),
      ),
    ],
  );
}

class _PixelPanel extends StatelessWidget {
  const _PixelPanel({required this.child});

  final Widget child;

  @override
  Widget build(BuildContext context) => DecoratedBox(
    decoration: BoxDecoration(
      color: _paper,
      borderRadius: BorderRadius.circular(18),
      border: Border.all(color: _line, width: 1.5),
      boxShadow: const [
        BoxShadow(color: Color(0x443B2A24), offset: Offset(4, 4)),
      ],
    ),
    child: Padding(padding: const EdgeInsets.all(16), child: child),
  );
}

class _SettingsInfoTile extends StatelessWidget {
  const _SettingsInfoTile({
    required this.icon,
    required this.title,
    required this.subtitle,
    this.trailing,
  });

  final IconData icon;
  final String title;
  final String subtitle;
  final Widget? trailing;

  @override
  Widget build(BuildContext context) => Row(
    children: [
      DecoratedBox(
        decoration: BoxDecoration(
          color: _paperMuted,
          borderRadius: BorderRadius.circular(10),
          border: Border.all(color: _line),
        ),
        child: Padding(
          padding: const EdgeInsets.all(9),
          child: Icon(icon, color: _ink, size: 22),
        ),
      ),
      const SizedBox(width: 12),
      Expanded(
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(title, style: const TextStyle(fontWeight: FontWeight.w800)),
            const SizedBox(height: 2),
            Text(
              subtitle,
              style: Theme.of(
                context,
              ).textTheme.bodySmall?.copyWith(color: const Color(0xFF77675D)),
            ),
          ],
        ),
      ),
      trailing ?? const SizedBox.shrink(),
    ],
  );
}

class _PresenceDot extends StatelessWidget {
  const _PresenceDot({required this.online});

  final bool online;

  @override
  Widget build(BuildContext context) => Container(
    width: 11,
    height: 11,
    decoration: BoxDecoration(
      color: online ? const Color(0xFF4E9B67) : const Color(0xFF9A8B82),
      shape: BoxShape.circle,
      border: Border.all(color: _paper, width: 2),
      boxShadow: const [BoxShadow(color: Color(0x553B2A24), blurRadius: 2)],
    ),
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
      style: Theme.of(
        context,
      ).textTheme.bodyMedium?.copyWith(color: const Color(0xFF77675D)),
    ),
  );
}
