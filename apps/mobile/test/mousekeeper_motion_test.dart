import 'package:flutter_test/flutter_test.dart';
import 'package:mousekeeper/core/models/character_state.dart';
import 'package:mousekeeper/features/character/mousekeeper_motion.dart';
import 'package:mousekeeper_character_assets/character_assets.dart';

void main() {
  test('server character kinds map to the intended PNG motion', () {
    expect(
      mousekeeperMotionForCharacterKind(CharacterState.waitingApproval),
      MouseKeeperMotion.considering,
    );
    expect(
      mousekeeperMotionForCharacterKind(CharacterState.success),
      MouseKeeperMotion.clean,
    );
    expect(
      mousekeeperMotionForCharacterKind(CharacterState.error),
      MouseKeeperMotion.fighting,
    );
  });

  test('offline takes priority over stale execution results', () {
    final motion = mousekeeperMotionForHome(
      isOffline: false,
      presences: const ['OFFLINE'],
      executionStatuses: const ['SUCCEEDED'],
      hasPendingProposal: false,
      realtimeCharacterKind: CharacterState.success,
    );
    expect(motion, MouseKeeperMotion.sleeping);
  });

  test('active execution and pending approval use live state motions', () {
    expect(
      mousekeeperMotionForHome(
        isOffline: false,
        presences: const ['ONLINE_EXECUTING'],
        executionStatuses: const [],
        hasPendingProposal: false,
      ),
      MouseKeeperMotion.working,
    );
    expect(
      mousekeeperMotionForHome(
        isOffline: false,
        presences: const ['ONLINE_IDLE'],
        executionStatuses: const [],
        hasPendingProposal: true,
      ),
      MouseKeeperMotion.considering,
    );
  });
}
