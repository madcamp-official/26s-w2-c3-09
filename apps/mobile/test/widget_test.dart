import 'package:flutter_test/flutter_test.dart';
import 'package:housemouse/main.dart';

void main() {
  testWidgets('외부 인증 미설정 상태를 명확히 표시한다', (tester) async {
    await tester.pumpWidget(const HousemouseApp());
    expect(find.text('HOUSEMOUSE'), findsOneWidget);
    expect(find.textContaining('UNCONFIGURED'), findsOneWidget);
  });
}
