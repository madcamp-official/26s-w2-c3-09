import 'package:flutter/material.dart';
import 'package:mousekeeper_character_assets/character_assets.dart';

import '../../core/models/character_state.dart';

class MouseKeeperMotionImage extends StatelessWidget {
  const MouseKeeperMotionImage({
    super.key,
    required this.motion,
    this.width,
    this.height,
  });

  final MouseKeeperMotion motion;
  final double? width;
  final double? height;

  @override
  Widget build(BuildContext context) => AnimatedSwitcher(
    duration: const Duration(milliseconds: 240),
    transitionBuilder: (child, animation) => FadeTransition(
      opacity: animation,
      child: ScaleTransition(
        scale: Tween<double>(begin: 0.96, end: 1).animate(animation),
        child: child,
      ),
    ),
    child: Image.asset(
      mousekeeperMotionAsset(motion),
      key: ValueKey(motion),
      package: mousekeeperMascotPackage,
      width: width,
      height: height,
      fit: BoxFit.contain,
      filterQuality: FilterQuality.none,
      semanticLabel: 'MouseKeeper ${motion.name}',
    ),
  );
}

MouseKeeperMotion? mousekeeperMotionForCharacterKind(CharacterState? kind) =>
    switch (kind) {
      CharacterState.analyzing ||
      CharacterState.waitingApproval => MouseKeeperMotion.considering,
      CharacterState.working => MouseKeeperMotion.working,
      CharacterState.success => MouseKeeperMotion.clean,
      CharacterState.error => MouseKeeperMotion.fighting,
      CharacterState.userWorking => MouseKeeperMotion.walk,
      CharacterState.offline => MouseKeeperMotion.sleeping,
      CharacterState.connecting => MouseKeeperMotion.walk,
      CharacterState.idle => MouseKeeperMotion.stand,
      null => null,
    };

MouseKeeperMotion mousekeeperMotionForHome({
  required bool isOffline,
  required Iterable<String> presences,
  required Iterable<String?> executionStatuses,
  required bool hasPendingProposal,
  CharacterState? realtimeCharacterKind,
}) {
  final presenceSet = presences.toSet();
  if (isOffline ||
      (presenceSet.isNotEmpty &&
          presenceSet.every((item) => item == 'OFFLINE'))) {
    return MouseKeeperMotion.sleeping;
  }
  if (presenceSet.contains('DEGRADED')) return MouseKeeperMotion.fighting;
  if (presenceSet.contains('ONLINE_EXECUTING')) {
    return MouseKeeperMotion.working;
  }
  if (presenceSet.contains('ONLINE_SCANNING')) {
    return MouseKeeperMotion.considering;
  }

  final liveMotion = mousekeeperMotionForCharacterKind(realtimeCharacterKind);
  if (liveMotion != null) return liveMotion;

  final statuses = executionStatuses.whereType<String>().toSet();
  if (statuses.any(
    (status) => const {
      'PARTIALLY_SUCCEEDED',
      'FAILED',
      'STALE',
      'ROLLED_BACK',
    }.contains(status),
  )) {
    return MouseKeeperMotion.fighting;
  }
  if (statuses.contains('EXECUTING')) return MouseKeeperMotion.working;
  if (hasPendingProposal ||
      statuses.any(
        (status) => const {
          'ANALYZING',
          'PROPOSAL_READY',
          'WAITING_APPROVAL',
        }.contains(status),
      )) {
    return MouseKeeperMotion.considering;
  }
  if (statuses.contains('SUCCEEDED')) return MouseKeeperMotion.clean;
  if (statuses.any(
    (status) => const {'QUEUED', 'DELIVERED', 'APPROVED'}.contains(status),
  )) {
    return MouseKeeperMotion.walk;
  }
  if (presenceSet.isEmpty) return MouseKeeperMotion.sleeping;
  return MouseKeeperMotion.stand;
}
