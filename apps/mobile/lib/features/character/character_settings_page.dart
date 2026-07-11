import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import '../../core/network/api_client.dart';

typedef CharacterSave =
    Future<Map<String, dynamic>> Function(Map<String, dynamic> body);

class CharacterSettingsPage extends ConsumerStatefulWidget {
  const CharacterSettingsPage({
    super.key,
    required this.initialCharacter,
    this.save,
  });

  final Map<String, dynamic> initialCharacter;
  final CharacterSave? save;

  @override
  ConsumerState<CharacterSettingsPage> createState() =>
      _CharacterSettingsPageState();
}

class _CharacterSettingsPageState extends ConsumerState<CharacterSettingsPage> {
  static const _furVariants = ['brown', 'cream'];
  static const _accessories = ['none', 'scarf'];
  static const _roomThemes = ['warm', 'forest'];

  late Set<String> _unlocked;
  late String _furVariant;
  late String _accessory;
  late String _roomTheme;
  late bool _animationsEnabled;
  bool _saving = false;

  @override
  void initState() {
    super.initState();
    final appearance = Map<String, dynamic>.from(
      widget.initialCharacter['appearance'] as Map? ?? const {},
    );
    _unlocked = (widget.initialCharacter['unlockedItems'] as List? ?? const [])
        .whereType<String>()
        .toSet();
    if (_unlocked.isEmpty) {
      _unlocked = {'fur:brown', 'accessory:none', 'theme:warm'};
    }
    _furVariant = _known(
      appearance['furVariant'],
      _furVariants,
      _furVariants.first,
    );
    if (!_unlocked.contains('fur:$_furVariant')) {
      _furVariant = _furVariants.first;
    }
    _accessory = _known(
      appearance['accessory'],
      _accessories,
      _accessories.first,
    );
    if (!_unlocked.contains('accessory:$_accessory')) {
      _accessory = _accessories.first;
    }
    _roomTheme = _known(
      widget.initialCharacter['roomTheme'],
      _roomThemes,
      _roomThemes.first,
    );
    if (!_unlocked.contains('theme:$_roomTheme')) {
      _roomTheme = _roomThemes.first;
    }
    _animationsEnabled = appearance['animationsEnabled'] as bool? ?? true;
  }

  String _known(Object? value, List<String> allowed, String fallback) =>
      value is String && allowed.contains(value) ? value : fallback;

  @override
  Widget build(BuildContext context) => Scaffold(
    appBar: AppBar(title: const Text('캐릭터와 방 꾸미기')),
    body: ListView(
      padding: const EdgeInsets.all(16),
      children: [
        if (widget.initialCharacter['riveAssetStatus'] == 'UNCONFIGURED') ...[
          const Card(
            color: Color(0xFFFFF3E0),
            child: ListTile(
              leading: Icon(Icons.animation_outlined),
              title: Text('캐릭터 애니메이션 미설정'),
              subtitle: Text(
                'Rive asset이 없어 현재는 선택값만 안전하게 저장합니다. 오류 코드: UNCONFIGURED',
              ),
            ),
          ),
          const SizedBox(height: 12),
        ],
        Text('외형', style: Theme.of(context).textTheme.titleLarge),
        if (widget.initialCharacter['nextUnlockAffinity'] case final int next)
          ListTile(
            contentPadding: EdgeInsets.zero,
            leading: const Icon(Icons.lock_outline),
            title: Text('호감도 $next에서 크림색·목도리·숲 테마 해금'),
            subtitle: Text(
              '현재 호감도 ${widget.initialCharacter['affinityTotal'] ?? 0}',
            ),
          ),
        const SizedBox(height: 12),
        DropdownButtonFormField<String>(
          initialValue: _furVariant,
          decoration: const InputDecoration(
            labelText: '털 색상',
            border: OutlineInputBorder(),
          ),
          items: [
            const DropdownMenuItem(value: 'brown', child: Text('갈색')),
            DropdownMenuItem(
              value: 'cream',
              enabled: _unlocked.contains('fur:cream'),
              child: Text(_unlocked.contains('fur:cream') ? '크림색' : '크림색 · 잠김'),
            ),
          ],
          onChanged: _saving
              ? null
              : (value) => setState(() => _furVariant = value!),
        ),
        const SizedBox(height: 12),
        DropdownButtonFormField<String>(
          initialValue: _accessory,
          decoration: const InputDecoration(
            labelText: '액세서리',
            border: OutlineInputBorder(),
          ),
          items: [
            const DropdownMenuItem(value: 'none', child: Text('없음')),
            DropdownMenuItem(
              value: 'scarf',
              enabled: _unlocked.contains('accessory:scarf'),
              child: Text(
                _unlocked.contains('accessory:scarf') ? '목도리' : '목도리 · 잠김',
              ),
            ),
          ],
          onChanged: _saving
              ? null
              : (value) => setState(() => _accessory = value!),
        ),
        const SizedBox(height: 24),
        Text('방 테마', style: Theme.of(context).textTheme.titleLarge),
        const SizedBox(height: 12),
        SegmentedButton<String>(
          segments: [
            const ButtonSegment(value: 'warm', label: Text('포근함')),
            ButtonSegment(
              value: 'forest',
              enabled: _unlocked.contains('theme:forest'),
              label: Text(_unlocked.contains('theme:forest') ? '숲' : '숲 · 잠김'),
            ),
          ],
          selected: {_roomTheme},
          onSelectionChanged: _saving
              ? null
              : (value) => setState(() => _roomTheme = value.single),
        ),
        const SizedBox(height: 16),
        SwitchListTile(
          contentPadding: EdgeInsets.zero,
          title: const Text('캐릭터 애니메이션'),
          subtitle: const Text('끄면 모든 상태 애니메이션을 정지하도록 설정합니다.'),
          value: _animationsEnabled,
          onChanged: _saving
              ? null
              : (value) => setState(() => _animationsEnabled = value),
        ),
        const SizedBox(height: 28),
        FilledButton.icon(
          onPressed: _saving ? null : _save,
          icon: _saving
              ? const SizedBox.square(
                  dimension: 18,
                  child: CircularProgressIndicator(strokeWidth: 2),
                )
              : const Icon(Icons.save_outlined),
          label: const Text('선택 저장'),
        ),
      ],
    ),
  );

  Future<void> _save() async {
    setState(() => _saving = true);
    final body = <String, dynamic>{
      'appearance': {
        'furVariant': _furVariant,
        'accessory': _accessory,
        'animationsEnabled': _animationsEnabled,
      },
      'roomTheme': _roomTheme,
    };
    try {
      final save =
          widget.save ??
          (payload) =>
              ref.read(apiClientProvider).patch('/v1/character', payload);
      await save(body);
      if (mounted) Navigator.pop(context, true);
    } catch (error) {
      if (mounted) {
        ScaffoldMessenger.of(
          context,
        ).showSnackBar(SnackBar(content: Text('캐릭터 설정 저장 실패: $error')));
        setState(() => _saving = false);
      }
    }
  }
}
