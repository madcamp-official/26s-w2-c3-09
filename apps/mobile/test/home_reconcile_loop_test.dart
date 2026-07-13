import 'dart:async';

import 'package:flutter_test/flutter_test.dart';
import 'package:mousekeeper/features/home/home_page.dart';

void main() {
  testWidgets(
    'authoritative fallback runs every two seconds without overlap and stops on dispose',
    (tester) async {
      expect(homeAuthoritativeReconcileInterval, const Duration(seconds: 2));

      final pending = <Completer<void>>[];
      var reconcileCalls = 0;
      var refreshCalls = 0;
      final loop = HomeAuthoritativeReconcileLoop(
        reconcile: () {
          reconcileCalls++;
          final completer = Completer<void>();
          pending.add(completer);
          return completer.future;
        },
        onReconciled: () => refreshCalls++,
      )..start();

      await tester.pump(const Duration(milliseconds: 1999));
      expect(reconcileCalls, 0);
      await tester.pump(const Duration(milliseconds: 1));
      expect(reconcileCalls, 1);

      // Slow requests do not accumulate another authoritative read.
      await tester.pump(const Duration(seconds: 4));
      expect(reconcileCalls, 1);

      pending.single.complete();
      await tester.pump();
      expect(refreshCalls, 1);

      await tester.pump(homeAuthoritativeReconcileInterval);
      expect(reconcileCalls, 2);

      loop.dispose();
      pending.last.complete();
      await tester.pump();
      await tester.pump(const Duration(seconds: 4));
      expect(reconcileCalls, 2);
      expect(refreshCalls, 1);
    },
  );
}
