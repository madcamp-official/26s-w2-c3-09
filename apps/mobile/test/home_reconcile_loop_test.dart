import 'dart:async';

import 'package:flutter_test/flutter_test.dart';
import 'package:mousekeeper/features/home/home_page.dart';

void main() {
  testWidgets(
    'five-second safety reconcile refreshes only for actual changes',
    (tester) async {
      expect(homeAuthoritativeReconcileInterval, const Duration(seconds: 5));

      final pending = <Completer<bool>>[];
      var reconcileCalls = 0;
      var refreshCalls = 0;
      final loop = HomeAuthoritativeReconcileLoop(
        reconcile: () {
          reconcileCalls++;
          final completer = Completer<bool>();
          pending.add(completer);
          return completer.future;
        },
        onChanged: () => refreshCalls++,
      )..start();

      await tester.pump(const Duration(milliseconds: 4999));
      expect(reconcileCalls, 0);
      await tester.pump(const Duration(milliseconds: 1));
      expect(reconcileCalls, 1);

      // Slow requests do not accumulate another authoritative read.
      await tester.pump(const Duration(seconds: 4));
      expect(reconcileCalls, 1);

      pending.single.complete(false);
      await tester.pump();
      expect(refreshCalls, 0);

      await tester.pump(homeAuthoritativeReconcileInterval);
      expect(reconcileCalls, 2);
      pending.last.complete(true);
      await tester.pump();
      expect(refreshCalls, 1);

      loop.dispose();
      await tester.pump(const Duration(seconds: 10));
      expect(reconcileCalls, 2);
      expect(refreshCalls, 1);
    },
  );
}
