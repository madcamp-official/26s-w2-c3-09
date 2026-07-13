import 'package:flutter/material.dart';
import 'package:housemouse_character_assets/character_assets.dart';

class HousemouseMotionImage extends StatelessWidget {
  const HousemouseMotionImage({
    super.key,
    required this.motion,
    this.width,
    this.height,
  });

  final HousemouseMotion motion;
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
      housemouseMotionAsset(motion),
      key: ValueKey(motion),
      package: housemouseMascotPackage,
      width: width,
      height: height,
      fit: BoxFit.contain,
      filterQuality: FilterQuality.none,
      semanticLabel: 'HouseMouse ${motion.name}',
    ),
  );
}

HousemouseMotion? housemouseMotionForCharacterKind(String? kind) =>
    switch (kind) {
      'ANALYZING' || 'WAITING_APPROVAL' => HousemouseMotion.considering,
      'WORKING' => HousemouseMotion.working,
      'SUCCESS' => HousemouseMotion.clean,
      'ERROR' => HousemouseMotion.fighting,
      'USER_WORKING' => HousemouseMotion.walk,
      'OFFLINE' => HousemouseMotion.sleeping,
      'IDLE' => HousemouseMotion.stand,
      _ => null,
    };

HousemouseMotion housemouseMotionForHome({
  required bool isOffline,
  required Iterable<String> presences,
  required Iterable<String?> executionStatuses,
  required bool hasPendingProposal,
  String? realtimeCharacterKind,
}) {
  final presenceSet = presences.toSet();
  if (isOffline ||
      (presenceSet.isNotEmpty &&
          presenceSet.every((item) => item == 'OFFLINE'))) {
    return HousemouseMotion.sleeping;
  }
  if (presenceSet.contains('DEGRADED')) return HousemouseMotion.fighting;
  if (presenceSet.contains('ONLINE_EXECUTING')) {
    return HousemouseMotion.working;
  }
  if (presenceSet.contains('ONLINE_SCANNING')) {
    return HousemouseMotion.considering;
  }

  final liveMotion = housemouseMotionForCharacterKind(realtimeCharacterKind);
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
    return HousemouseMotion.fighting;
  }
  if (statuses.contains('EXECUTING')) return HousemouseMotion.working;
  if (hasPendingProposal ||
      statuses.any(
        (status) => const {
          'ANALYZING',
          'PROPOSAL_READY',
          'WAITING_APPROVAL',
        }.contains(status),
      )) {
    return HousemouseMotion.considering;
  }
  if (statuses.contains('SUCCEEDED')) return HousemouseMotion.clean;
  if (statuses.any(
    (status) => const {'QUEUED', 'DELIVERED', 'APPROVED'}.contains(status),
  )) {
    return HousemouseMotion.walk;
  }
  if (presenceSet.isEmpty) return HousemouseMotion.sleeping;
  return HousemouseMotion.stand;
}
