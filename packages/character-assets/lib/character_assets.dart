enum MouseKeeperMotion {
  clean,
  considering,
  fighting,
  hello,
  sleeping,
  stand,
  walk,
  working,
}

const mousekeeperCleanAsset = 'mouse_clean.png';
const mousekeeperConsideringAsset = 'mouse_considering.png';
const mousekeeperFightingAsset = 'mouse_fighting.png';
const mousekeeperHelloAsset = 'mouse_hello.png';
const mousekeeperSleepingAsset = 'mouse_sleeping.png';
const mousekeeperStandAsset = 'mouse_stand.png';
const mousekeeperWalkAsset = 'mouse_walk.png';
const mousekeeperWorkingAsset = 'mouse_working.png';

// Stable default for screens that do not have a runtime state yet.
const mousekeeperMascotAsset = mousekeeperStandAsset;
const mousekeeperMascotPackage = 'mousekeeper_character_assets';

const mousekeeperHomeBackgroundAssets = <String>[
  'backgrounds/background_1.png',
  'backgrounds/background_2.png',
  'backgrounds/background_3.png',
  'backgrounds/background_4.png',
  'backgrounds/background_5.png',
];

String mousekeeperMotionAsset(MouseKeeperMotion motion) => switch (motion) {
  MouseKeeperMotion.clean => mousekeeperCleanAsset,
  MouseKeeperMotion.considering => mousekeeperConsideringAsset,
  MouseKeeperMotion.fighting => mousekeeperFightingAsset,
  MouseKeeperMotion.hello => mousekeeperHelloAsset,
  MouseKeeperMotion.sleeping => mousekeeperSleepingAsset,
  MouseKeeperMotion.stand => mousekeeperStandAsset,
  MouseKeeperMotion.walk => mousekeeperWalkAsset,
  MouseKeeperMotion.working => mousekeeperWorkingAsset,
};
