import 'package:flutter_test/flutter_test.dart';
import 'package:housemouse/features/character/housemouse_motion.dart';
import 'package:housemouse_character_assets/character_assets.dart';

void main() {
  test('server character kinds map to the intended PNG motion', () {
    expect(
      housemouseMotionForCharacterKind('WAITING_APPROVAL'),
      HousemouseMotion.considering,
    );
    expect(housemouseMotionForCharacterKind('SUCCESS'), HousemouseMotion.clean);
    expect(
      housemouseMotionForCharacterKind('ERROR'),
      HousemouseMotion.fighting,
    );
  });

  test('offline takes priority over stale execution results', () {
    final motion = housemouseMotionForHome(
      isOffline: false,
      presences: const ['OFFLINE'],
      executionStatuses: const ['SUCCEEDED'],
      hasPendingProposal: false,
      realtimeCharacterKind: 'SUCCESS',
    );
    expect(motion, HousemouseMotion.sleeping);
  });

  test('active execution and pending approval use live state motions', () {
    expect(
      housemouseMotionForHome(
        isOffline: false,
        presences: const ['ONLINE_EXECUTING'],
        executionStatuses: const [],
        hasPendingProposal: false,
      ),
      HousemouseMotion.working,
    );
    expect(
      housemouseMotionForHome(
        isOffline: false,
        presences: const ['ONLINE_IDLE'],
        executionStatuses: const [],
        hasPendingProposal: true,
      ),
      HousemouseMotion.considering,
    );
  });
}
