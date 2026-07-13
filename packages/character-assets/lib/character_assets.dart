enum HousemouseMotion {
  clean,
  considering,
  fighting,
  hello,
  sleeping,
  stand,
  walk,
  working,
}

const housemouseCleanAsset = 'mouse_clean.png';
const housemouseConsideringAsset = 'mouse_considering.png';
const housemouseFightingAsset = 'mouse_fighting.png';
const housemouseHelloAsset = 'mouse_hello.png';
const housemouseSleepingAsset = 'mouse_sleeping.png';
const housemouseStandAsset = 'mouse_stand.png';
const housemouseWalkAsset = 'mouse_walk.png';
const housemouseWorkingAsset = 'mouse_working.png';

// Stable default for screens that do not have a runtime state yet.
const housemouseMascotAsset = housemouseStandAsset;
const housemouseMascotPackage = 'housemouse_character_assets';

String housemouseMotionAsset(HousemouseMotion motion) => switch (motion) {
  HousemouseMotion.clean => housemouseCleanAsset,
  HousemouseMotion.considering => housemouseConsideringAsset,
  HousemouseMotion.fighting => housemouseFightingAsset,
  HousemouseMotion.hello => housemouseHelloAsset,
  HousemouseMotion.sleeping => housemouseSleepingAsset,
  HousemouseMotion.stand => housemouseStandAsset,
  HousemouseMotion.walk => housemouseWalkAsset,
  HousemouseMotion.working => housemouseWorkingAsset,
};
