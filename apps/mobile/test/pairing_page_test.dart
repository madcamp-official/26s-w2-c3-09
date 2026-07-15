import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:mousekeeper/features/auth/pairing_page.dart';

void main() {
  testWidgets('페어링 대기 화면은 전용 아이콘과 10분 만료 안내를 표시한다', (tester) async {
    tester.view.physicalSize = const Size(320, 568);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    await tester.pumpWidget(
      ProviderScope(
        child: MaterialApp(home: PairingPage(onClaim: (_) async {})),
      ),
    );
    await tester.pump();

    final image = tester.widget<Image>(
      find.byKey(const ValueKey('pairing-waiting-mouse-icon')),
    );
    expect(image.image, isA<AssetImage>());
    expect(
      (image.image as AssetImage).assetName,
      'assets/images/pairing_mouse_icon.png',
    );
    expect(find.text('코드는 10분 후 만료됩니다'), findsOneWidget);

    for (final label in [
      'MOUSEKEEPER',
      '기기를 연결해주세요',
      'Desktop의 코드를 입력해주세요',
      '코드는 10분 후 만료됩니다',
    ]) {
      final text = tester.widget<Text>(find.text(label));
      expect(text.maxLines, 1, reason: '$label should stay on one line');
      expect(text.softWrap, isFalse, reason: '$label should not wrap');
    }
    expect(tester.takeException(), isNull);
  });
}
