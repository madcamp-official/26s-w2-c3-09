/// Typed representation of the nine SCREAMING_SNAKE_CASE values shared by
/// the server, desktop, and mobile character-state contract.
enum CharacterState {
  idle('IDLE'),
  connecting('CONNECTING'),
  analyzing('ANALYZING'),
  waitingApproval('WAITING_APPROVAL'),
  working('WORKING'),
  success('SUCCESS'),
  error('ERROR'),
  userWorking('USER_WORKING'),
  offline('OFFLINE');

  const CharacterState(this.wireValue);

  final String wireValue;
}

/// Parses only exact contract values. Unknown casing, types, or future states
/// stay fail-closed until this client explicitly supports them.
CharacterState? parseCharacterState(Object? value) => switch (value) {
  'IDLE' => CharacterState.idle,
  'CONNECTING' => CharacterState.connecting,
  'ANALYZING' => CharacterState.analyzing,
  'WAITING_APPROVAL' => CharacterState.waitingApproval,
  'WORKING' => CharacterState.working,
  'SUCCESS' => CharacterState.success,
  'ERROR' => CharacterState.error,
  'USER_WORKING' => CharacterState.userWorking,
  'OFFLINE' => CharacterState.offline,
  _ => null,
};
