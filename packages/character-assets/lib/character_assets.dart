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
const mousekeeperPairingIconAsset = 'mouse_icon.png';

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

const mousekeeperMouseDanglingGif = 'new_mouse/gif/mouse_dangling_preview.gif';
const mousekeeperMouseIdleGif = 'new_mouse/gif/mouse_idle_preview.gif';
const mousekeeperMouseMadGif = 'new_mouse/gif/mouse_mad_preview.gif';
const mousekeeperMouseOrganizeGif = 'new_mouse/gif/mouse_organize_preview.gif';
const mousekeeperMousePatheticGif = 'new_mouse/gif/mouse_pathetic_preview.gif';
const mousekeeperMouseSleepGif = 'new_mouse/gif/mouse_sleep_preview.gif';
const mousekeeperMouseWalkGif = 'new_mouse/gif/mouse_walk_preview.gif';
const mousekeeperMouseWorkGif = 'new_mouse/gif/mouse_work_preview.gif';

const mousekeeperRestingMouseGifs = <String>[
  mousekeeperMouseDanglingGif,
  mousekeeperMouseIdleGif,
  mousekeeperMouseMadGif,
  mousekeeperMouseOrganizeGif,
  mousekeeperMousePatheticGif,
  mousekeeperMouseSleepGif,
  mousekeeperMouseWorkGif,
];

String mousekeeperHomeBackgroundAssetForIndex(int index) {
  if (mousekeeperHomeBackgroundAssets.isEmpty) return '';
  final safeIndex = index
      .clamp(0, mousekeeperHomeBackgroundAssets.length - 1)
      .toInt();
  return mousekeeperHomeBackgroundAssets[safeIndex];
}

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
