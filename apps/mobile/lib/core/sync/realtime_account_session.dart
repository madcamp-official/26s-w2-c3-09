class RealtimeAccountSession {
  const RealtimeAccountSession({
    required this.ownerUid,
    required this.generation,
  });

  final String ownerUid;
  final int generation;
}

/// Invalidates every captured callback whenever the authenticated account or
/// transport generation changes.
class RealtimeAccountSessionGuard {
  String? _ownerUid;
  int _generation = 0;
  bool _initialized = false;

  String? get ownerUid => _ownerUid;

  bool bind(String? ownerUid) {
    if (_initialized && _ownerUid == ownerUid) return false;
    _initialized = true;
    _ownerUid = ownerUid;
    _generation++;
    return true;
  }

  RealtimeAccountSession? beginConnection() {
    final ownerUid = _ownerUid;
    if (ownerUid == null) return null;
    return RealtimeAccountSession(
      ownerUid: ownerUid,
      generation: ++_generation,
    );
  }

  RealtimeAccountSession? get current {
    final ownerUid = _ownerUid;
    return ownerUid == null
        ? null
        : RealtimeAccountSession(ownerUid: ownerUid, generation: _generation);
  }

  void invalidate() => _generation++;

  bool isCurrent(RealtimeAccountSession session, String? currentOwnerUid) =>
      session.ownerUid == _ownerUid &&
      session.ownerUid == currentOwnerUid &&
      session.generation == _generation;
}
