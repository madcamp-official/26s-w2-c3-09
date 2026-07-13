import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:mousekeeper/app.dart';
import 'package:mousekeeper/features/character/character_settings_page.dart';
import 'package:mousekeeper/features/chat/readme_command_page.dart';
import 'package:mousekeeper/features/home/home_page.dart';
import 'package:mousekeeper/features/files/smart_cache_page.dart';
import 'package:mousekeeper/features/proposals/proposal_page.dart';

void main() {
  testWidgets('외부 인증 미설정 상태를 명확히 표시한다', (tester) async {
    await tester.pumpWidget(
      const ProviderScope(
        child: MouseKeeperApp(configurationError: 'FIREBASE_ENABLED'),
      ),
    );
    expect(find.text('MOUSEKEEPER'), findsOneWidget);
    expect(find.textContaining('UNCONFIGURED'), findsOneWidget);
  });

  testWidgets('오프라인 캐시와 빈 방 상태를 명확히 표시한다', (tester) async {
    await tester.pumpWidget(
      const MaterialApp(
        home: Scaffold(
          body: Column(children: [OfflineCacheBanner(), EmptyRoomsCard()]),
        ),
      ),
    );
    expect(find.text('오프라인 표시 데이터'), findsOneWidget);
    expect(find.text('마지막으로 동기화된 정보를 표시합니다.'), findsOneWidget);
    expect(find.text('등록된 방이 없습니다'), findsOneWidget);
  });

  testWidgets('연결 오류 상태에서 다시 시도할 수 있다', (tester) async {
    var retried = false;
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: HomeConnectionError(
            error: StateError('NETWORK_UNAVAILABLE'),
            onRetry: () => retried = true,
          ),
        ),
      ),
    );
    expect(find.textContaining('서버와 연결되지 않았습니다.'), findsOneWidget);
    await tester.tap(find.text('다시 시도'));
    expect(retried, isTrue);
  });

  testWidgets('제안 항목에 상대 경로와 이유, 충돌을 표시한다', (tester) async {
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: ProposalItemsList(
            items: [
              {
                'actionType': 'QUARANTINE',
                'sourceRelativePath': 'old/report.pdf',
                'destinationRelativePath': null,
                'reasonCode': 'AGE_RULE_MATCH',
                'conflictState': 'NAME_CONFLICT',
              },
            ],
          ),
        ),
      ),
    );
    expect(find.textContaining('old/report.pdf'), findsOneWidget);
    expect(find.textContaining('MOUSEKEEPER 휴지통'), findsOneWidget);
    expect(find.textContaining('AGE_RULE_MATCH'), findsOneWidget);
    expect(find.textContaining('NAME_CONFLICT'), findsOneWidget);
  });

  testWidgets('오프라인 캐시 최신성 및 대기 명령 경고를 숨기지 않는다', (tester) async {
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: ListView(
            children: [
              const PendingCommandWarning(),
              const DesktopOfflineCacheWarning(),
              CachedFileTile(
                file: {
                  'sourceRelativePath': 'reports/monthly.pdf',
                  'availabilityStatus': 'AVAILABLE',
                  'freshnessStatus': 'UNVERIFIED_OFFLINE',
                  'cachedAt': '2026-07-11T10:00:00Z',
                  'lastVerifiedAt': '2026-07-11T09:00:00Z',
                },
                isDownloading: false,
                progress: null,
                onDownload: () {},
                onRemove: () {},
              ),
            ],
          ),
        ),
      ),
    );
    expect(find.textContaining('명령 처리 후'), findsOneWidget);
    expect(find.textContaining('PC 에이전트와 연결되지 않음'), findsOneWidget);
    expect(find.textContaining('다운로드 가능'), findsOneWidget);
    expect(find.textContaining('최신 여부 미확인'), findsOneWidget);
    expect(find.textContaining('원본 마지막 확인'), findsOneWidget);
  });

  testWidgets('Rive 미설정을 숨기지 않고 캐릭터 선택값을 저장한다', (tester) async {
    Map<String, dynamic>? saved;
    await tester.pumpWidget(
      ProviderScope(
        child: MaterialApp(
          home: CharacterSettingsPage(
            initialCharacter: const {
              'appearance': <String, dynamic>{},
              'roomTheme': null,
              'riveAssetStatus': 'UNCONFIGURED',
            },
            save: (body) async {
              saved = body;
              return body;
            },
          ),
        ),
      ),
    );
    expect(find.textContaining('UNCONFIGURED'), findsOneWidget);
    await tester.tap(find.text('선택 저장'));
    await tester.pumpAndSettle();
    expect(saved?['roomTheme'], 'warm');
    expect(
      (saved?['appearance'] as Map<String, dynamic>)['furVariant'],
      'brown',
    );
    expect(
      (saved?['appearance'] as Map<String, dynamic>)['animationsEnabled'],
      isTrue,
    );
  });

  testWidgets('호감도 전에는 두 번째 외형과 테마를 잠금 표시한다', (tester) async {
    await tester.pumpWidget(
      ProviderScope(
        child: MaterialApp(
          home: CharacterSettingsPage(
            initialCharacter: const {
              'appearance': <String, dynamic>{},
              'roomTheme': null,
              'affinityTotal': 2,
              'nextUnlockAffinity': 3,
              'unlockedItems': ['fur:brown', 'accessory:none', 'theme:warm'],
              'riveAssetStatus': 'UNCONFIGURED',
            },
            save: _saveCharacterForWidgetTest,
          ),
        ),
      ),
    );
    expect(find.textContaining('호감도 3에서'), findsOneWidget);
    expect(find.textContaining('숲 · 잠김'), findsOneWidget);
    expect(find.text('캐릭터 애니메이션'), findsOneWidget);
  });

  testWidgets('README 질문을 구조화된 command로 만들고 승인 전 변경하지 않는다', (tester) async {
    Map<String, dynamic>? submitted;
    await tester.pumpWidget(
      ProviderScope(
        child: MaterialApp(
          home: ReadmeCommandPage(
            roomId: 'room-id',
            submit: (command) async {
              submitted = command;
              return false;
            },
          ),
        ),
      ),
    );
    expect(find.textContaining('승인 전에는 파일을 변경하지 않습니다'), findsOneWidget);
    await tester.enterText(find.byType(TextFormField).at(0), '모바일 앱 프로젝트');
    await tester.enterText(find.byType(TextFormField).at(1), '새 팀원');
    await tester.enterText(find.byType(TextFormField).at(2), '설치 방법\n실행 방법');
    await tester.tap(find.text('README 제안 요청'));
    await tester.pumpAndSettle();
    expect(submitted?['intent'], 'README');
    final payload = submitted?['payload'] as Map<String, dynamic>;
    expect(payload['purpose'], '모바일 앱 프로젝트');
    expect(payload['sections'], ['설치 방법', '실행 방법']);
  });

  testWidgets('Desktop이 보낸 README draft와 diff만 검토 화면에 표시한다', (tester) async {
    await tester.pumpWidget(
      const MaterialApp(
        home: Scaffold(
          body: ProposalSummaryCard(
            summary: {
              'readmeDiff': '- old\n+ new',
              'readmeDraft': '# Project\nRun safely.',
            },
          ),
        ),
      ),
    );
    expect(find.text('README 초안과 실제 diff'), findsOneWidget);
    expect(find.textContaining('- old'), findsOneWidget);
    expect(find.textContaining('# Project'), findsOneWidget);
  });
}

Future<Map<String, dynamic>> _saveCharacterForWidgetTest(
  Map<String, dynamic> body,
) async => body;
