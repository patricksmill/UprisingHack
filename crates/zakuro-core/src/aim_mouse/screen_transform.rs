//! the one process that puts a game-owned Screen's drawing and input on the
//! top screen, and the registry of the Screens it knows.
//!
//! Pause, its dialog, the 29 Scripted Menus, the Fiend's Cauldron and the
//! Midstage Loading are here; see docs/agents/screen-transform.md.

use serde::{Deserialize, Serialize};

use super::{
    outline_altar, outline_armory, outline_buttons, outline_chapter, outline_controls, outline_dialog, outline_file,
    outline_idol, outline_none, outline_other, outline_practice, outline_profile, outline_queued, outline_solo, outline_subtitles,
    outline_treasure, outline_vault, outline_volume, outline_yes_no, Found, MenuCall, Recorded, System, ToTop,
    CAULDRON, PAUSE_MENU, PAUSE_MENU_OFFSET,
};

/// a Screen from the glossary's Screens table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    FileSelect,
    ChapterSelect,
    MainMenu,
    Options,
    Other,
    Controls,
    ReticleCamera,
    Subtitles,
    Volume,
    Profile,
    ProfileYesNo,
    SpotPassDialog,
    SoloHub,
    ArmsAltar,
    ArmoryGear,
    ArmoryRedeemHearts,
    ArmoryConvertToHearts,
    ArmoryFuseWeapons,
    ArmoryPowers,
    ArmoryWeapons,
    ArmoryCategories,
    PracticeRange,
    IdolTossSolo,
    TreasureHunt,
    Vault,
    IdolTossVault,
    Idols,
    PowerPortrait,
    ChapterResults,
    FiendsCauldron,
    Pause,
    RetreatDialog,
    MidstageLoading,
}

/// the fastest a Screen may be drawn in High Refresh Mode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ScreenRate {
    /// every host refresh, like the 3D scene.
    #[default]
    Host,
    /// the title's own frames only: 60 a second, no picture in between.
    /// A Screen found to misbehave under interpolation takes this instead
    /// of being excluded from High Refresh Mode by category.
    Sixty,
}

/// which subsystem owns a Screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Family {
    InGame,
    Scripted,
    Cauldron,
}

/// how a Screen activates a Control.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum InputMode {
    TouchesOnly,
    PressA(#[allow(dead_code)] u32),
    ClickAsTouch,
}

/// what a Screen is detected by.
#[derive(Clone, Copy, Debug)]
pub(super) enum Identity {
    CallSites(&'static [ToTop]),
    Vtable { vtable: u32, update: u32 },
    TaskProbe { show: [u32; 2] },
}

/// how a click uses the outlined Control.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Activate {
    /// the game function A uses.
    Call(u32),
    /// no A: the menu owns its list input.
    None,
    /// the click becomes a native touch where it is.
    Touch,
}

pub(super) type OutlineFn = fn(&mut System, u32, Found) -> Option<Vec<MenuCall>>;

/// Hubs have individual animated models; a list or board stays one group.
const SOLO: &[(&str, [f32; 4])] = &[
    ("MenSgHomeBt0001", [0.0, 92.0, 88.0, 112.0]),
    ("MenSgHomeBt0003", [188.0, 68.0, 132.0, 124.0]),
    ("MenSgHomeBt0004", [226.0, 0.0, 94.0, 76.0]),
    ("MenSgHomeBt0005", [178.0, 160.0, 142.0, 80.0]),
    ("MenSgHomeBt0006", [84.0, 174.0, 96.0, 66.0]),
    ("MenSgHomeBt9999", [0.0, 194.0, 48.0, 46.0]),
    ("MenSgHomeBt0002", [12.0, 0.0, 242.0, 164.0]),
];
pub(super) const ALTAR: &[(&str, [f32; 4])] = &[
    ("MenAlterBt0002", [140.0, 28.0, 180.0, 132.0]),
    ("MenAlterBt0003", [0.0, 88.0, 206.0, 122.0]),
    ("MenAlterBt0004", [130.0, 150.0, 190.0, 90.0]),
    ("MenAlterBt9999", [0.0, 192.0, 58.0, 48.0]),
    ("MenAlterBt0001", [0.0, 0.0, 198.0, 108.0]),
];
/// The Solo Hub's top screen: the hall. `MenCmnUpFace0000` draws nothing
/// here; the heart counter is its own part already.
const SOLO_TOP: &[TopLayer] = &[
    ("Hall", "MenSgHomeUp0001", &[]),
];
/// The Main Menu's top: paper, ink wash, ornaments, the Solo title, its flourish and Pit's silhouette.
const MAIN_TOP: &[TopLayer] = &[
    ("Paper background", "MenTopUp0010", &["DN_U0000"]),
    ("Ink wash", "MenTopUp0000", &["NN_U0006"]),
    ("Ornaments", "MenTopUp00*", &[]),
    ("Title flourish", "MenTopUp0010", &["NN_U0008"]),
    ("Page title", "MenTopUp0000", &["NN_U0007"]),
    ("Silhouette", "MenTopUp0000", &["NN_U0009"]),
];
/// Options pages share one backdrop mesh (`...Up0000`) and a page picture (`...Up0001`).
const OPTIONS_TOP: &[TopLayer] = &[
    ("Paper background", "MenOptionTopUp0000", &["NN_U1000"]),
    ("Corner flourish", "MenOptionTopUp0000", &["NN_U0100"]),
    ("Lower ornaments", "MenOptionTopUp0000", &["NN_U0005"]),
    ("Ornaments", "MenOptionTopUp0000", &[]),
    ("Picture", "MenOptionTopUp0001", &[]),
];
const OTHER_TOP: &[TopLayer] = &[
    ("Paper background", "MenOptionTopUp0000", &["NN_U1000"]),
    ("Corner flourish", "MenOptionTopUp0000", &["NN_U0100"]),
    ("Lower ornaments", "MenOptionTopUp0000", &["NN_U0005"]),
    ("Ornaments", "MenOptionTopUp0000", &[]),
    ("Picture", "MenOptionOtherUp0001", &[]),
];
const CONTROLS_TOP: &[TopLayer] = &[
    ("Paper background", "MenOptionControlUp0000", &["NN_U1000"]),
    ("Corner flourish", "MenOptionControlUp0000", &["NN_U0100"]),
    ("Lower ornaments", "MenOptionControlUp0000", &["NN_U0005"]),
    ("Ornaments", "MenOptionControlUp0000", &[]),
    ("Picture", "MenOptionControlUp0001", &[]),
];
const RETICLE_CAMERA_TOP: &[TopLayer] = &[
    ("Paper background", "MenOptionCtlTpnUp0000", &["NN_U1000"]),
    ("Corner flourish", "MenOptionCtlTpnUp0000", &["NN_U0100"]),
    ("Lower ornaments", "MenOptionCtlTpnUp0000", &["NN_U0005"]),
    ("Ornaments", "MenOptionCtlTpnUp0000", &[]),
    ("Picture", "MenOptionCtlTpnUp0001", &[]),
];
const SUBTITLES_TOP: &[TopLayer] = &[
    ("Picture", "MenOptionOtherCaptionUp0001", &[]),
];
const VOLUME_TOP: &[TopLayer] = &[
    ("Paper background", "MenOptionSoundUp0000", &["NN_U1000"]),
    ("Corner flourish", "MenOptionSoundUp0000", &["NN_U0100"]),
    ("Lower ornaments", "MenOptionSoundUp0000", &["NN_U0005"]),
    ("Ornaments", "MenOptionSoundUp0000", &[]),
    ("Picture", "MenOptionSoundUp0001", &[]),
];
const PROFILE_TOP: &[TopLayer] = &[
    ("Paper background", "MenOptionTopUp0000", &["NN_U1000"]),
    ("Corner flourish", "MenOptionTopUp0000", &["NN_U0100"]),
    ("Lower ornaments", "MenOptionTopUp0000", &["NN_U0005"]),
    ("Ornaments", "MenOptionTopUp0000", &[]),
    ("Picture", "MenOptionNameUp0001", &[]),
];
/// Profile's question over the file card: lined paper, rainbow, the black
/// ink splat behind the words, ornaments, the file's numeral, red border.
const PROFILE_CHOICE_TOP: &[TopLayer] = &[
    ("Paper background", "MenSelFileUp0001", &["NN_U0000"]),
    ("Rainbow", "MenSelFileUp0001", &["NN_U0001"]),
    ("Ink splat", "MenSelFileUp0001", &["NN_B0004"]),
    ("Ornaments", "MenSelFileUp0001", &[]),
    ("File number", "MenSelFileUp0001", &["FileNum01", "FileNum02", "FileNum03"]),
    ("Red border", "MenSelFileUp0002", &[]),
];
const SPOTPASS_TOP: &[TopLayer] = &[
    ("Paper background", "MenOptionTopUp0000", &["NN_U1000"]),
    ("Corner flourish", "MenOptionTopUp0000", &["NN_U0100"]),
    ("Lower ornaments", "MenOptionTopUp0000", &["NN_U0005"]),
    ("Ornaments", "MenOptionTopUp0000", &[]),
    ("Picture", "MenOptionOtherUp0005", &[]),
];
/// The Arms Altar: the hall, its ink frame and the floating weapons.
const ALTAR_TOP: &[TopLayer] = &[
    ("Hall", "MenAlterUp0000", &["NN_U0100"]),
    ("Ink frame", "MenAlterUp0000", &[]),
    ("Weapons", "MenAlterUp0001", &[]),
];
/// Redeem Hearts' card by model.
const REDEEM_TOP: &[TopLayer] = &[
    ("Paper background", "MenAlterbuyUp0100", &[]),
    ("Weapon showcase", "MenAlterbuyUp0101", &[]),
    ("Hearts", "MenAlterbuyUp0102", &[]),
    ("Price tag", "MenAlterbuyUp0103", &[]),
];
/// Convert to Hearts' card by model.
const CONVERT_TOP: &[TopLayer] = &[
    ("Paper background", "MenAltersoldUp0100", &[]),
    ("Weapon", "MenAltersoldUp0101", &[]),
    ("Value card", "MenAltersoldUp0103", &[]),
    ("Heart bar", "MenAltersoldUp0104", &[]),
    ("Heart total", "MenAltersoldUp0105", &[]),
    ("Bonus board", "MenAltersoldUp0106", &[]),
];
/// Armory Categories: paper, category emblem, weapon shadow, weapon.
const CATEGORIES_TOP: &[TopLayer] = &[
    ("Paper background", "MenEquipUp0100", &[]),
    ("Category emblem", "MenEquipUp0101", &["NN_U0000"]),
    ("Weapon shadow", "MenEquipUp0101", &["NN_U0002"]),
    ("Weapon", "MenEquipUp0101", &["NN_U0001"]),
];
/// Practice Range: paper, emblem, arrows and the title plate.
const PRACTICE_TOP: &[TopLayer] = &[
    ("Paper background", "MenTrymissionUp0000", &["NN_U0000"]),
    ("Emblem", "MenTrymissionUp0000", &["NN_U0001", "NN_U0003"]),
    ("Arrows", "MenTrymissionUp0000", &["NN_U0002"]),
    ("Title plate", "MenTrymissionUp0002", &[]),
];
/// Treasure Hunt: frame, Palutena, description paper, Power bar and reward icon.
const TREASURE_TOP: &[TopLayer] = &[
    ("Frame", "MenStockroomUp0000", &[]),
    ("Palutena", "MenStockroomUp0014", &["NN_U0000"]),
    ("Description paper", "MenStockroomUp0002", &[]),
    ("Power bar", "MenStockroomUp0001", &[]),
    ("Reward icon", "MenStockroomUp0014", &[]),
];
/// The Vault: background, ornaments, Pit's silhouette and the title.
const VAULT_TOP: &[TopLayer] = &[
    ("Paper background", "MenCollectionTopUp0000", &["back_00"]),
    ("Ornaments", "MenCollectionTopUp0000", &[]),
    ("Silhouette", "MenCollectionTopUp0001", &[]),
    ("Page title", "MenCollectionTopUp0005", &[]),
];
/// Chapter Results (MENUID 34, `MenOnlineRsltte*`): the results after a
/// chapter, then each reward with Take. Probed from the user's desktop
/// slot 5 (a Power's reward page). Its other pages' models (the score
/// board) stay together in Results board on both screens.
const RESULTS_TOP: &[TopLayer] = &[
    ("Paper background", "MenOnlineRsltteUp0001", &["bg_play", "kazari"]),
    ("Results board", "MenOnlineRsltteUp*", &[]),
    ("Name bar", "MenOnlineRsltteUp0006", &["MN_U6001", "nameBase"]),
    ("Seal", "MenOnlineRsltteUp0007", &["hanko", "hanko2"]),
    ("Reward icon", "MenOnlineRsltteUp0005", &["NN_U5002"]),
];

/// Chapter Results' bottom, back to front; the score board's models are the
/// rest of `MenOnlineRsltteBt*`. The named parts list their nodes so the
/// prefix owner, earlier in the table, does not take them.
const RESULTS_BOTTOM: &[TopLayer] = &[
    ("Background", "MenOnlineRsltteBt0000", &["back", "kazari", "moyou"]),
    ("Results board", "MenOnlineRsltteBt*", &[]),
    ("Message bar", "MenOnlineRsltteBt0008", &["bace2", "base2"]),
    ("Message", "MenOnlineRsltteBt0010", &["MN_B1010"]),
    ("Take", "MenOnlineRsltteBt0001", &["base", "cursor"]),
    ("Window", "MenCmnWindowBt0000", &[]),
];

/// Chapter Results' bottom Board with native boxes.
const RESULTS: &[(&str, [f32; 4])] = &[
    ("Background", [0.0, 0.0, 320.0, 240.0]),
    ("Results board", [0.0, 0.0, 320.0, 240.0]),
    ("Message bar", [8.0, 18.0, 304.0, 68.0]),
    ("Message", [8.0, 18.0, 304.0, 68.0]),
    ("Take", [176.0, 196.0, 116.0, 44.0]),
    ("Window", [0.0, 0.0, 320.0, 240.0]),
];
/// Idol Toss's top screen (both rows): the backdrop (the sky and rays in a
/// reveal) and the new Idol (`Dol*`), drawn only while it is revealed.
/// `MenColVisionUp0001`, `...Up0003` and `...Up0004` drew nothing idle or
/// in a reveal. `MenColVisionUp0001`
/// draws nothing.
const IDOL_TOSS_TOP: &[TopLayer] = &[
    ("Backdrop", "MenColVisionUp0000", &[]),
    ("Idol", "Dol*", &[]),
];
/// Redeem Hearts' bottom Board, back to front, with native boxes: the
/// backdrops, the weapon list, its icons and Cursor, the header, Back and
/// the Buy dialog.
const REDEEM: &[(&str, [f32; 4])] = &[
    ("MenAlterbuyBt0100", [0.0, 0.0, 320.0, 240.0]),
    ("MenAlterbuyBt0101", [0.0, 0.0, 320.0, 240.0]),
    ("MenAlterbuyBt0120", [0.0, 22.0, 320.0, 170.0]),
    ("MenAlterbuyBt0131", [0.0, 22.0, 320.0, 170.0]),
    ("MenAlterbuyBt0125", [0.0, 22.0, 320.0, 170.0]),
    ("MenAlterbuyBt0123", [0.0, 0.0, 320.0, 22.0]),
    ("MenAlterbuyBt8888", [0.0, 0.0, 320.0, 240.0]),
    ("MenAlterbuyBt9999", [0.0, 190.0, 60.0, 50.0]),
    ("MenAlterbuyBt0130", [0.0, 0.0, 320.0, 240.0]),
];
/// Convert to Hearts' bottom Board, as Redeem Hearts', with its scroll bar.
const CONVERT: &[(&str, [f32; 4])] = &[
    ("MenAltersoldBt0100", [0.0, 0.0, 320.0, 240.0]),
    ("MenAltersoldBt0101", [0.0, 0.0, 320.0, 240.0]),
    ("MenAltersoldBt0120", [0.0, 22.0, 320.0, 170.0]),
    ("MenAltersoldBt0124", [0.0, 22.0, 320.0, 170.0]),
    ("MenAltersoldBt0121", [0.0, 22.0, 320.0, 170.0]),
    ("MenAltersoldBt0125", [0.0, 22.0, 320.0, 170.0]),
    ("MenAltersoldBt0132", [296.0, 22.0, 24.0, 170.0]),
    ("MenAltersoldBt0123", [0.0, 0.0, 320.0, 22.0]),
    ("MenAltersoldBt9999", [0.0, 190.0, 60.0, 50.0]),
    ("MenAltersoldBt0130", [0.0, 0.0, 320.0, 240.0]),
];
/// Treasure Hunt's bottom, one part per model: tile board, the
/// tiles, feathers, Back and the reward dialog. The board's pieces share
/// the whole screen's box so they stay together until moved. The bottom's
/// backdrop (`MenStockroomBt0000`, with a black matte and a red block
/// off its edges) is left out, as the other hubs' backdrops are.
const TREASURE: &[(&str, [f32; 4])] = &[
    ("MenStockroomBt0001", [0.0, 0.0, 320.0, 240.0]),
    ("MenStockroomBt0021", [0.0, 0.0, 320.0, 240.0]),
    ("MenStockroomBt0003", [0.0, 0.0, 320.0, 240.0]),
    ("MenStockroomBt0002", [284.0, 0.0, 36.0, 80.0]),
    ("MenStockroomBt0004", [0.0, 0.0, 320.0, 240.0]),
    ("MenStockroomBt9999", [0.0, 196.0, 50.0, 44.0]),
    ("MenStockroomBt0010", [0.0, 0.0, 320.0, 240.0]),
    ("MenCmnWindowBt0000", [0.0, 0.0, 320.0, 240.0]),
];
const MAIN: &[(&str, [f32; 4])] = &[
    ("MenTopBt0000", [260.0, 178.0, 60.0, 62.0]),
    ("MenTopBt0002", [120.0, 82.0, 200.0, 98.0]),
    ("MenTopBt0003", [0.0, 82.0, 130.0, 98.0]),
    ("MenTopBt0004", [198.0, 0.0, 122.0, 86.0]),
    ("MenTopBt0005", [40.0, 176.0, 188.0, 64.0]),
    ("MenTopBt0006", [4.0, 190.0, 62.0, 50.0]),
    ("MenTopBt0007", [220.0, 178.0, 50.0, 62.0]),
    ("MenTopBt0001", [0.0, 0.0, 210.0, 100.0]),
];
const OPTIONS: &[(&str, [f32; 4])] = &[
    ("MenOptionTopBt0001", [58.0, 0.0, 262.0, 104.0]),
    ("MenOptionTopBt0002", [0.0, 78.0, 266.0, 102.0]),
    ("MenOptionTopBt0003", [100.0, 148.0, 220.0, 92.0]),
    ("MenOptionTopBt9999", [0.0, 198.0, 50.0, 42.0]),
];
const OTHER: &[(&str, [f32; 4])] = &[
    ("MenOptionOtherBt0001", [0.0, 0.0, 320.0, 60.0]),
    ("MenOptionOtherBt0002", [0.0, 60.0, 320.0, 60.0]),
    ("MenOptionOtherBt0004", [0.0, 120.0, 320.0, 60.0]),
    ("MenOptionOtherBt9999", [0.0, 196.0, 52.0, 44.0]),
];
const CONTROLS: &[(&str, [f32; 4])] = &[
    ("MenOptionControlBt0001", [0.0, 12.0, 320.0, 62.0]),
    ("MenOptionControlBt0002", [0.0, 74.0, 320.0, 62.0]),
    ("MenOptionControlBt0006", [0.0, 136.0, 320.0, 62.0]),
    ("MenOptionControlBt9999", [0.0, 196.0, 52.0, 44.0]),
    ("MenOptionControlBt0003", [0.0, 154.0, 320.0, 46.0]),
    ("MenOptionControlBt0004", [0.0, 0.0, 320.0, 58.0]),
    ("MenOptionControlBt0007", [0.0, 106.0, 320.0, 48.0]),
    ("MenOptionControlBt0008", [0.0, 58.0, 320.0, 48.0]),
    ("MenOptionControlBt0005", [80.0, 198.0, 220.0, 42.0]),
];
const RETICLE_CAMERA: &[(&str, [f32; 4])] = &[
    ("MenOptionCtlTpenBt0001", [0.0, 24.0, 166.0, 160.0]),
    ("MenOptionCtlTpenBt0002", [166.0, 24.0, 154.0, 160.0]),
    ("MenOptionCtlTpenBt9999", [0.0, 196.0, 52.0, 44.0]),
];
const SUBTITLES: &[(&str, [f32; 4])] = &[
    ("MenOptionOtherCaptionBt0001", [0.0, 68.0, 180.0, 64.0]),
    ("MenOptionOtherCaptionBt0002", [0.0, 140.0, 180.0, 64.0]),
    ("MenOptionOtherCaptionBt0003", [188.0, 104.0, 132.0, 64.0]),
];
const VOLUME: &[(&str, [f32; 4])] = &[
    ("MenOptionSoundBt0002", [0.0, 32.0, 288.0, 44.0]),
    ("MenOptionSoundBt0003", [0.0, 76.0, 288.0, 40.0]),
    ("MenOptionSoundBt0004", [0.0, 116.0, 288.0, 44.0]),
    ("MenOptionSoundBt0005", [120.0, 196.0, 200.0, 44.0]),
    ("MenOptionSoundBt9999", [0.0, 196.0, 52.0, 44.0]),
];
const PROFILE: &[(&str, [f32; 4])] = &[
    ("MenOptionNameBt0001", [0.0, 12.0, 320.0, 172.0]),
    ("MenOptionNameBt9999", [0.0, 196.0, 52.0, 44.0]),
];
const PROFILE_CHOICE: &[(&str, [f32; 4])] = &[
    ("MenSelnameBt0001", [72.0, 12.0, 224.0, 92.0]),
    ("MenSelnameBt0002", [12.0, 108.0, 248.0, 100.0]),
    ("MenSelnameBt9999", [0.0, 196.0, 52.0, 44.0]),
];
const SPOTPASS_CHOICE: &[(&str, [f32; 4])] = &[
    ("MenCmnWindowBt0000", [0.0, 0.0, 320.0, 170.0]),
    ("MenCmnWindowBt0000", [0.0, 170.0, 160.0, 70.0]),
    ("MenCmnWindowBt0000", [160.0, 170.0, 160.0, 70.0]),
];
const PRACTICE: &[(&str, [f32; 4])] = &[
    ("MenTrymissionBt0002", [46.0, 94.0, 274.0, 122.0]),
    ("MenTrymissionBt0003", [46.0, 192.0, 274.0, 48.0]),
    ("MenTrymissionBt9999", [0.0, 196.0, 50.0, 44.0]),
    ("MenTrymissionBt0001", [0.0, 0.0, 294.0, 116.0]),
];
const VAULT: &[(&str, [f32; 4])] = &[
    ("MenCollectionTopBt0002", [168.0, 0.0, 130.0, 110.0]),
    ("MenCollectionTopBt0003", [100.0, 184.0, 156.0, 56.0]),
    ("MenCollectionTopBt0005", [0.0, 98.0, 110.0, 92.0]),
    ("MenCollectionTopBt0006", [108.0, 98.0, 110.0, 92.0]),
    ("MenCollectionTopBt0007", [212.0, 98.0, 108.0, 92.0]),
    ("CmnBack0000", [0.0, 196.0, 50.0, 44.0]),
    ("MenCollectionTopBt0009", [260.0, 194.0, 60.0, 46.0]),
    ("MenCollectionTopBt0001", [0.0, 0.0, 170.0, 110.0]),
];

const FILE_SELECT: &[(&str, [f32; 4])] = &[
    ("MenSelfileBt0000", [0.0, 0.0, 125.0, 190.0]),
    ("MenSelfileBt0001", [100.0, 20.0, 115.0, 170.0]),
    ("MenSelfileBt0002", [205.0, 20.0, 115.0, 170.0]),
    ("MenSelfileBt0003", [0.0, 195.0, 50.0, 45.0]),
    ("MenSelfileBt0004", [185.0, 192.0, 135.0, 48.0]),
];

/// The owner naming the game's `CircleShadow` sprites (drawn by 0x0080B100
/// in every scene pass) rather than a model: only that layer's replay draws
/// them.
pub(super) const CIRCLE_SHADOW: &str = "CircleShadow";

/// Editor label, owning model and mesh nodes; an empty list owns the rest.
/// An owner ending in `*` owns every model whose name starts with the rest.
pub(super) type TopLayer = (&'static str, &'static str, &'static [&'static str]);

/// Armory Gear's top screen, split into editor layers by mesh node:
/// (layer name, model, nodes). Ornaments are the rest of the backdrop
/// model. Anything else on the top screen, such as the Powers grid, stays
/// in the Details card.
const GEAR_TOP: &[TopLayer] = &[
    ("Paper background", "MenEquipUp0000", &["NN_U0000", "NN_U0001"]),
    ("Silhouette", "MenEquipUp0000", &["NN_U0027"]),
    ("Crest", "MenEquipUp0000", &["NN_U0002"]),
    ("Ornaments", "MenEquipUp0000", &[]),
    ("Page title", "MenEquipUp0001", &["NN_U0000", "NN_U0001"]),
    ("Weapon", "MenEquipUp0001", &["NN_U0002", "NN_U00022"]),
];

/// Armory Weapons' top screen, split into editor layers back to front.
/// Identified by moving one node at a time. `MenEquipUp0100` is the paper
/// with its crest, ornaments and ink frame in one mesh. The weapon is a
/// node of the board model, so it is a layer too; the Details card keeps
/// the words. `NN_1111` and `pPlane2` draw nothing here and stay in it.
/// `MenEquipUp0500` is the equip flash.
const WEAPONS_TOP: &[TopLayer] = &[
    ("Paper background", "MenEquipUp0100", &[]),
    ("Description board", "MenEquipUp0121", &["NN_U0005", "NN_U0007"]),
    ("Category emblem", "MenEquipUp0121", &["NN_U0000"]),
    ("Stats card", "MenEquipUp0121", &["NN_U0003", "pPlane1"]),
    ("Weapon shadow", "MenEquipUp0121", &["NN_U0002"]),
    ("Weapon", "MenEquipUp0121", &["NN_U0001"]),
    ("Equip flash", "MenEquipUp0500", &[]),
];

const POWERS_TOP: &[TopLayer] = &[
    ("Paper background", "MenEquipUp0200", &[]),
    ("Power set board", "MenEquipUp0202", &[]),
    ("Power preview", "MenEquipUp0210", &["NN_U0000"]),
    ("Power shape", "MenEquipUp0210", &["ML_B0000"]),
    ("Description background", "MenEquipUp0210", &["Wbase"]),
];

/// Armory — Fuse Weapons' top screen, back to front. Identified by moving
/// one node at a time (`MenAltermixUp0000`). `NN_U0000` is the paper with
/// both weapons' coloured headers in one mesh. Each weapon's bonus slots
/// are dark bars over one shared mesh of yellow filled bars (`NN_U0009`),
/// which shows where a dark bar is hidden. The sparkles are a particle
/// model. `light2` and `light3` are the white rays of the fusion's flash:
/// left to the Details card, they drew over the new weapon. `NN_U0006`,
/// `NN_U0007`, `Seed1`, `Seed2`, `MenAltermixUp0001` and `MenAltermixUp0102`
/// draw nothing on this Screen or in its fusion and stay in the Details
/// card, which keeps the words.
const FUSE_TOP: &[TopLayer] = &[
    ("Paper background", "MenAltermixUp0000", &["NN_U0000"]),
    ("Ornament", "MenAltermixUp0000", &["NN_U01"]),
    ("Weapon A", "MenAltermixUp0000", &["NN_U0003"]),
    ("Weapon B", "MenAltermixUp0000", &["NN_U0005"]),
    ("Filled bonus slots", "MenAltermixUp0000", &["NN_U0009"]),
    ("Weapon A bonus slots", "MenAltermixUp0000", &["MN_U0001", "MN_U0002", "MN_U0003", "MN_U0004", "MN_U0005", "MN_U0006"]),
    ("Weapon B bonus slots", "MenAltermixUp0000", &["MN_U0007", "MN_U0008", "MN_U0009", "MN_U0010", "MN_U0011", "MN_U0012"]),
    ("Result bar", "MenAltermixUp0000", &["NN_U0008"]),
    ("Result rings", "MenAltermixUp0000", &["cir", "cir1"]),
    ("Result rays", "MenAltermixUp0000", &["light", "light1", "light2", "light3"]),
    ("Result shadow", "MenAltermixUp0000", &["NN_U0001"]),
    ("Result weapon", "MenAltermixUp0000", &["NN_U0002"]),
    ("Sparkles", "PtcCmnMenuAlterMix", &[]),
];

/// Armory — Fuse Weapons' bottom models split by mesh node, probed from
/// the user's desktop slot 4. `MenAltermixBt0000` holds the two lists'
/// backgrounds and a grey mask the Frame covers; `MenAltermixBt0002` the
/// Frame (corner art, A and B tabs, bottom bar), both scroll bars and Back.
/// Weapon A's list is the row along the top, Weapon B's the column at the
/// left. The three Cursors follow the Outline. `MenAltermixBt0003` (the
/// fusion's flash), `MenAltermixBt0010` (Fuse and Cancel) and the Window
/// (Yes and No) draw nothing while the Board is idle.
const FUSE_BOTTOM: &[TopLayer] = &[
    ("Weapon A background", "MenAltermixBt0000", &["NN_B0002"]),
    ("Weapon B background", "MenAltermixBt0000", &["NN_B0003"]),
    ("Frame mask", "MenAltermixBt0000", &["NN_U0000"]),
    ("Frame", "MenAltermixBt0002", &["NN_B0000"]),
    ("Weapon A scroll bar", "MenAltermixBt0002", &["MN_B0000_", "MN_B0004", "MN_B0005", "NN_B0004"]),
    ("Weapon B scroll bar", "MenAltermixBt0002", &["MN_B0001_", "MN_B0002", "MN_B0003"]),
    ("Back", "MenAltermixBt0002", &["NN_B0005"]),
    ("Fusion flash", "MenAltermixBt0003", &[]),
    ("Weapon A Cursor", "MenAltermixBt0006", &[]),
    ("Weapon B Cursor", "MenAltermixBt0007", &[]),
    ("Grid Cursor", "MenAltermixBt0008", &[]),
    ("Sort order", "MenAltermixBt0009", &[]),
    ("Fusion grid", "MenAltermixBt0001", &[]),
    ("Weapon A list", "MenAltermixBt0004", &[]),
    ("Weapon B list", "MenAltermixBt0005", &[]),
    ("Fusion dialog", "MenAltermixBt0010", &[]),
    ("Window", "MenCmnWindowBt0000", &[]),
];

/// The Fuse Weapons Board, back to front, with native boxes. Node layers
/// are cut at their boxes: a Cursor's box covers every place it can go, a
/// list's box is its window.
const FUSE: &[(&str, [f32; 4])] = &[
    ("Weapon A background", [56.0, 0.0, 264.0, 46.0]),
    ("Weapon B background", [0.0, 26.0, 72.0, 214.0]),
    ("Frame mask", [48.0, 30.0, 272.0, 210.0]),
    ("Frame", [0.0, 0.0, 320.0, 240.0]),
    ("Weapon A scroll bar", [118.0, 44.0, 184.0, 12.0]),
    ("Weapon B scroll bar", [70.0, 80.0, 14.0, 130.0]),
    ("Back", [8.0, 212.0, 48.0, 28.0]),
    ("Fusion flash", [0.0, 0.0, 320.0, 240.0]),
    ("Weapon A Cursor", [84.0, 0.0, 236.0, 214.0]),
    ("Weapon B Cursor", [0.0, 56.0, 320.0, 158.0]),
    ("Grid Cursor", [86.0, 58.0, 234.0, 156.0]),
    ("Sort order", [136.0, 214.0, 168.0, 26.0]),
    ("Fusion grid", [86.0, 58.0, 234.0, 156.0]),
    ("Weapon A list", [84.0, 0.0, 220.0, 44.0]),
    ("Weapon B list", [0.0, 56.0, 72.0, 154.0]),
    ("Fusion dialog", [0.0, 0.0, 320.0, 240.0]),
    ("Window", [0.0, 0.0, 320.0, 240.0]),
];

/// Top layers drawn under the hint's words, the bar its words sit on
/// natively: placed with the hint by default, not in the Details card.
pub(super) const HINT_BACKING: &[&str] = &["Result bar"];

/// The Vault's Idols, back to front: the backdrop, the figure's shadow and
/// the figure. Each idol is its own `Dol*` model, so that owner is a prefix.
/// The floor shadow is the game's `CircleShadow` sprite, not a model, so
/// its owner is `CIRCLE_SHADOW`; `maru_kage` draws nothing visible here.
const IDOLS_TOP: &[TopLayer] = &[
    ("Backdrop", "MenColViewerUp0000", &[]),
    ("Shadow", CIRCLE_SHADOW, &[]),
    ("Idol", "Dol*", &[]),
];

/// A layer that is a texture inside another layer's mesh, not a mesh of
/// its own: editor label, owning model, and the box (top-screen pixels of
/// the model's native drawing) the texture fills at its identity transform.
/// It is placed by the material's texture Scale and Translate, so it shows
/// only inside that box: the mesh's texture coordinates are not linear
/// outside it.
pub(super) type TextureLayer = (&'static str, &'static str, [f32; 4]);

/// The Idols backdrop's category emblem. The game binds one texture per
/// category (`MenColVwUp_Dynamic00NN`) to the backdrop's only material, so
/// the emblem shares the backdrop's one mesh node. Measured from
/// `trace/idols-viewer.state` with a black border: Scale divides the box
/// about its bottom left corner; Translate moves it by its own size.
const IDOLS_TEXTURE: &[TextureLayer] = &[
    ("Emblem", "MenColViewerUp0000", [248.0, 25.0, 114.0, 113.0]),
];

/// The Idols chrome, `MenColViewerBt0000`, split by mesh node. Probed from
/// `trace/idols-viewer.state` and its List (Back from there). `base` is the
/// description page's white panel and `NN_B0002` its words' node; neither
/// draws on the List.
const IDOLS_BOTTOM: &[TopLayer] = &[
    ("Background", "MenColViewerBt0000", &["bg"]),
    ("Controls hint", "MenColViewerBt0000", &["sousa", "sousa_base"]),
    ("Title bar", "MenColViewerBt0000", &["title", "L_button", "R_button"]),
    ("Zoom bar", "MenColViewerBt0000", &["bar"]),
    ("Description", "MenColViewerBt0000", &[]),
    ("Page counter", "MenColViewerBt0000", &["pbase"]),
];

/// The Idols bottom Board, back to front within each model: the chrome
/// layers, the List's frame, Rows (with their Cursor and mask) and Scroll
/// bar, then Back and AR. Both pages, the List and one idol's description,
/// are this Screen (MENUID 46); models a page lacks draw nothing.
const IDOLS: &[(&str, [f32; 4])] = &[
    ("Background", [0.0, 0.0, 320.0, 240.0]),
    ("Controls hint", [0.0, 0.0, 300.0, 18.0]),
    ("Title bar", [0.0, 18.0, 300.0, 32.0]),
    ("Zoom bar", [296.0, 0.0, 24.0, 196.0]),
    ("Description", [0.0, 50.0, 300.0, 176.0]),
    ("Page counter", [100.0, 196.0, 120.0, 30.0]),
    ("MenColViewlistBt0000", [0.0, 0.0, 300.0, 230.0]),
    ("MenColViewlistBt0003", [0.0, 0.0, 286.0, 196.0]),
    ("MenCmnScbar", [286.0, 0.0, 34.0, 196.0]),
    ("MenColViewerBt9999", [0.0, 196.0, 52.0, 44.0]),
    ("MenColViewerBt0002", [260.0, 194.0, 60.0, 46.0]),
];

/// The Vault's Power Portrait, `MenColPazUp0000`, back to front: the frame
/// with its green board (`NN_U1101`), the checkerboard (`NN_U1102`) and the
/// 60 portrait pieces (`NN_U0001`..`NN_U0060`, shown as powers are earned).
/// The pieces own the rest, `MN_U1103`, `complete` and `flash` included,
/// which draw nothing at 15% completion. Probed from slot-2 state
/// `trace/powerportrait.state`.
const POWER_PORTRAIT_TOP: &[TopLayer] = &[
    ("Frame", "MenColPazUp0000", &["NN_U1101"]),
    ("Checkerboard", "MenColPazUp0000", &["NN_U1102"]),
    ("Portrait pieces", "MenColPazUp0000", &[]),
];

/// The Power Portrait's bottom models split by mesh node: `NN_B1000` is the
/// Current Completion banner and its ring in one mesh, `NN_B1001` the
/// percentage's digits. `pPlane1` and `comp` draw nothing; the Message
/// layer owns `pPlane1` so the line of words has a layer of its own.
const POWER_PORTRAIT_BOTTOM: &[TopLayer] = &[
    ("Background", "MenCmnBg0000", &[]),
    ("Message", "MenColPazBt0001", &["pPlane1"]),
    ("Completion badge", "MenColPazBt0001", &["NN_B1000", "comp"]),
    ("Completion number", "MenColPazBt0001", &["NN_B1001"]),
];

/// The Power Portrait's bottom Board (MENUID 47), back to front, then Back.
/// Node layers are cut at their boxes, so each box covers its artwork.
const POWER_PORTRAIT: &[(&str, [f32; 4])] = &[
    ("Background", [0.0, 0.0, 320.0, 240.0]),
    ("Message", [0.0, 0.0, 320.0, 56.0]),
    ("Completion badge", [8.0, 78.0, 280.0, 108.0]),
    ("Completion number", [184.0, 100.0, 96.0, 76.0]),
    ("CmnBack0000", [0.0, 190.0, 60.0, 50.0]),
];

/// Armory Controls are cut from the shared Board: their names, values,
/// rows and highlights are text and models drawn across the whole screen.
const GEAR: &[(&str, [f32; 4])] = &[
    ("MenEquipBt0001", [0.0, 0.0, 320.0, 142.0]),
    ("MenEquipBt0002", [128.0, 136.0, 192.0, 104.0]),
    ("MenEquipBt9999", [0.0, 190.0, 60.0, 50.0]),
];
/// Category tiles in native order: All, Blade, ?, Claws, Bow, ?, Club, ?, ?, Arm.
const CATEGORIES: &[(&str, [f32; 4])] = &[
    ("MenEquipBt0102", [56.0, 8.0, 132.0, 40.0]),
    ("MenEquipBt0103", [184.0, 8.0, 126.0, 40.0]),
    ("MenEquipBt0104", [56.0, 44.0, 132.0, 38.0]),
    ("MenEquipBt0105", [184.0, 44.0, 126.0, 38.0]),
    ("MenEquipBt0106", [56.0, 78.0, 132.0, 38.0]),
    ("MenEquipBt0107", [184.0, 78.0, 126.0, 38.0]),
    ("MenEquipBt0108", [56.0, 112.0, 132.0, 38.0]),
    ("MenEquipBt0109", [184.0, 112.0, 126.0, 38.0]),
    ("MenEquipBt0110", [56.0, 146.0, 132.0, 38.0]),
    ("MenEquipBt0111", [184.0, 146.0, 126.0, 38.0]),
    ("MenEquipBt9999", [0.0, 190.0, 60.0, 50.0]),
];
const WEAPON_LIST: &[(&str, [f32; 4])] = &[
    ("MenEquipBt0123", [0.0, 0.0, 146.0, 22.0]),
    ("MenEquipBt0121", [146.0, 0.0, 174.0, 22.0]),
    ("MenEquipBt0120", [0.0, 22.0, 320.0, 130.0]),
    ("MenEquipBt0122", [100.0, 190.0, 220.0, 50.0]),
    ("MenEquipBt9999", [0.0, 190.0, 60.0, 50.0]),
];
const POWERS: &[(&str, [f32; 4])] = &[
    ("MenEquipBt0204", [0.0, 0.0, 146.0, 24.0]),
    ("MenEquipBt0212", [146.0, 0.0, 174.0, 24.0]),
    ("MenEquipBt0203", [0.0, 24.0, 148.0, 170.0]),
    ("MenEquipBt0211", [146.0, 24.0, 174.0, 170.0]),
    ("MenEquipBt0205", [60.0, 192.0, 260.0, 48.0]),
    ("MenEquipBt9999", [0.0, 190.0, 60.0, 50.0]),
];

const HINT_BOTTOM: Option<[f32; 4]> = Some([0.0, 210.0, 400.0, 30.0]);
const HINT_TOP: Option<[f32; 4]> = Some([0.0, 0.0, 400.0, 30.0]);
const HINT_PRACTICE: Option<[f32; 4]> = Some([0.0, 0.0, 400.0, 108.0]);
const HINT_IDOL: Option<[f32; 4]> = Some([0.0, 170.0, 400.0, 70.0]);
const HINT_TREASURE: Option<[f32; 4]> = Some([0.0, 190.0, 400.0, 50.0]);
const HINT_VAULT: Option<[f32; 4]> = Some([0.0, 200.0, 400.0, 40.0]);

/// how one Screen places its scene models, named rather than indexed.
#[derive(Clone, Copy)]
pub(super) enum LayoutKind {
    /// no layout stage yet: the in-game Screens.
    None,
    /// chapter artwork above its controls.
    Chapter,
    /// animated 3D books, Controls and a separately editable information Board.
    FileSelect,
    /// the game's fixed board of models.
    Group { group: &'static [(&'static str, [f32; 4])], hint: Option<[f32; 4]>, details: bool, heart: bool, top_layers: &'static [TopLayer] },
    /// Controls cut from the whole native Board, beside the details card.
    Board { group: &'static [(&'static str, [f32; 4])], hint: Option<[f32; 4]>, top_layers: &'static [TopLayer], bottom_layers: &'static [TopLayer], texture_layers: &'static [TextureLayer] },
    /// Idol Toss's counters, Back and launcher, placed apart.
    Idol,
}

impl LayoutKind {
    pub fn hint(self) -> Option<[f32; 4]> {
        match self {
            LayoutKind::None | LayoutKind::Chapter | LayoutKind::FileSelect => None,
            LayoutKind::Group { hint, .. } | LayoutKind::Board { hint, .. } => hint,
            LayoutKind::Idol => HINT_IDOL,
        }
    }

    pub fn details(self) -> bool {
        match self {
            LayoutKind::Group { details, .. } => details,
            LayoutKind::FileSelect | LayoutKind::Board { .. } => true,
            LayoutKind::None | LayoutKind::Chapter | LayoutKind::Idol => false,
        }
    }

    pub fn group(self) -> &'static [(&'static str, [f32; 4])] {
        match self {
            LayoutKind::Group { group, .. } | LayoutKind::Board { group, .. } => group,
            LayoutKind::FileSelect => FILE_SELECT,
            _ => &[],
        }
    }

    pub fn top_layers(self) -> &'static [TopLayer] {
        match self {
            LayoutKind::Board { top_layers, .. } | LayoutKind::Group { top_layers, .. } => top_layers,
            LayoutKind::Idol => IDOL_TOSS_TOP,
            _ => &[],
        }
    }

    /// top layers drawn by moving a texture inside its model's mesh.
    pub fn texture_layers(self) -> &'static [TextureLayer] {
        match self { LayoutKind::Board { texture_layers, .. } => texture_layers, _ => &[] }
    }

    /// node layers of the bottom Board, named in `group` by layer name.
    pub fn bottom_layers(self) -> &'static [TopLayer] {
        match self { LayoutKind::Board { bottom_layers, .. } => bottom_layers, _ => &[] }
    }

    pub fn heart(self) -> bool {
        matches!(self, LayoutKind::Group { heart: true, .. })
    }
}

/// one Screen's row. Fields the current slices do not read yet are kept so
/// the row shape is the same when the remaining stages join.
pub(super) struct ScreenSpec {
    pub id: Screen,
    #[allow(dead_code)]
    pub family: Family,
    pub identity: Identity,
    /// where this Screen's calls start in the shared call table.
    pub first_call: usize,
    #[allow(dead_code)]
    pub input: InputMode,
    #[allow(dead_code)]
    pub fallback: &'static str,
    pub outline: Option<OutlineFn>,
    pub activate: Activate,
    pub layout: LayoutKind,
    /// the declared High Refresh Mode rate, overridable per run by
    /// `ScreenTransform::set_rate`.
    pub rate: ScreenRate,
}

impl ScreenSpec {
    pub fn calls(&self) -> &'static [ToTop] {
        match self.identity {
            Identity::CallSites(calls) => &calls[self.first_call..],
            Identity::Vtable { .. } | Identity::TaskProbe { .. } => &[],
        }
    }

    pub fn update(&self) -> Option<u32> {
        match self.identity {
            Identity::Vtable { update, .. } => Some(update),
            Identity::CallSites(_) | Identity::TaskProbe { .. } => None,
        }
    }

    pub fn vtable(&self) -> Option<u32> {
        match self.identity {
            Identity::Vtable { vtable, .. } => Some(vtable),
            Identity::CallSites(_) | Identity::TaskProbe { .. } => None,
        }
    }
}

/// the update function of each Scripted Menu, in registry order. Kept
/// separately so `PLACES` can read it in a const context.
pub(super) const SCRIPTED_UPDATES: [u32; 29] = [
    0x002A3EC4, 0x002A3FCC, 0x002A3F9C, 0x00224BA0, 0x001ECB4C, 0x00224C18, 0x001B4C74, 0x001ECBBC, 0x00224BD4,
    0x001ECBF4, 0x0016756C, 0x002A4070, 0x002ECCF4, 0x002A1FD8, 0x002B6DB8, 0x00238CBC, 0x001C9734, 0x00230B5C,
    0x002BB14C, 0x0019CDC8, 0x0019BDC4, 0x0023A9FC, 0x001B4D58, 0x0018AF44, 0x00224B80, 0x001B4D4C, 0x001B4D70,
    0x0018AEB8, 0x00260868,
];

pub(super) const SCRIPTED_COUNT: usize = SCRIPTED_UPDATES.len();

impl Activate {
    const fn input(self) -> InputMode {
        match self {
            Activate::Call(address) => InputMode::PressA(address),
            Activate::Touch => InputMode::ClickAsTouch,
            Activate::None => InputMode::TouchesOnly,
        }
    }
}

macro_rules! scripted {
    ($index:expr, $id:ident, $vtable:expr, $outline:path, $activate:expr, $layout:expr) => {{
        let activate = $activate;
        ScreenSpec {
            id: Screen::$id,
            family: Family::Scripted,
            identity: Identity::Vtable { vtable: $vtable, update: SCRIPTED_UPDATES[$index] },
            first_call: 0,
            input: activate.input(),
            fallback: "ZAKURO_MENUS_TOP",
            outline: Some($outline),
            activate,
            layout: $layout,
            rate: ScreenRate::Host,
        }
    }};
}

pub(super) static SCREENS: &[ScreenSpec] = &[
    scripted!(0, FileSelect, 0x00CEC3F4, outline_file, Activate::Call(0x003374D0), LayoutKind::FileSelect),
    scripted!(1, ChapterSelect, 0x00CF243C, outline_chapter, Activate::Call(0x003675C0), LayoutKind::Chapter),
    scripted!(2, MainMenu, 0x00CFBD9C, outline_buttons, Activate::Call(0x0036F59C), LayoutKind::Group { group: MAIN, hint: HINT_BOTTOM, details: false, heart: false, top_layers: MAIN_TOP }),
    scripted!(3, Options, 0x00CEAE98, outline_queued, Activate::Call(0x002FB9E0), LayoutKind::Group { group: OPTIONS, hint: HINT_BOTTOM, details: false, heart: false, top_layers: OPTIONS_TOP }),
    scripted!(4, Other, 0x00CF2368, outline_other, Activate::Call(0x002D50FC), LayoutKind::Group { group: OTHER, hint: HINT_BOTTOM, details: false, heart: false, top_layers: OTHER_TOP }),
    scripted!(5, Controls, 0x00CF84DC, outline_controls, Activate::Call(0x003186E8), LayoutKind::Group { group: CONTROLS, hint: HINT_BOTTOM, details: false, heart: false, top_layers: CONTROLS_TOP }),
    scripted!(6, ReticleCamera, 0x00CF9B60, outline_queued, Activate::Call(0x0029B26C), LayoutKind::Group { group: RETICLE_CAMERA, hint: HINT_BOTTOM, details: false, heart: false, top_layers: RETICLE_CAMERA_TOP }),
    scripted!(7, Subtitles, 0x00CFA034, outline_subtitles, Activate::Call(0x002E5448), LayoutKind::Group { group: SUBTITLES, hint: None, details: true, heart: false, top_layers: SUBTITLES_TOP }),
    scripted!(8, Volume, 0x00CF2390, outline_volume, Activate::Call(0x002D56CC), LayoutKind::Group { group: VOLUME, hint: HINT_BOTTOM, details: false, heart: false, top_layers: VOLUME_TOP }),
    scripted!(9, Profile, 0x00CF8E8C, outline_profile, Activate::Call(0x002E2F4C), LayoutKind::Group { group: PROFILE, hint: HINT_BOTTOM, details: false, heart: false, top_layers: PROFILE_TOP }),
    scripted!(10, ProfileYesNo, 0x00CEC41C, outline_yes_no, Activate::Call(0x001C16D4), LayoutKind::Group { group: PROFILE_CHOICE, hint: HINT_BOTTOM, details: false, heart: false, top_layers: PROFILE_CHOICE_TOP }),
    scripted!(11, SpotPassDialog, 0x00CF8270, outline_dialog, Activate::Call(0x00314DF8), LayoutKind::Group { group: SPOTPASS_CHOICE, hint: HINT_BOTTOM, details: false, heart: false, top_layers: SPOTPASS_TOP }),
    scripted!(12, SoloHub, 0x00CFC3D0, outline_solo, Activate::Call(0x00370300), LayoutKind::Group { group: SOLO, hint: HINT_BOTTOM, details: false, heart: true, top_layers: SOLO_TOP }),
    scripted!(13, ArmsAltar, 0x00CFCA28, outline_altar, Activate::Call(0x00322C34), LayoutKind::Group { group: ALTAR, hint: HINT_BOTTOM, details: false, heart: false, top_layers: ALTAR_TOP }),
    scripted!(14, ArmoryGear, 0x00CEC2B8, outline_armory, Activate::None, LayoutKind::Board { group: GEAR, hint: HINT_TOP, top_layers: GEAR_TOP, bottom_layers: &[], texture_layers: &[] }),
    scripted!(15, ArmoryRedeemHearts, 0x00CEFCA8, outline_armory, Activate::None, LayoutKind::Board { group: REDEEM, hint: None, top_layers: REDEEM_TOP, bottom_layers: &[], texture_layers: &[] }),
    scripted!(16, ArmoryConvertToHearts, 0x00CEEBFC, outline_armory, Activate::None, LayoutKind::Board { group: CONVERT, hint: None, top_layers: CONVERT_TOP, bottom_layers: &[], texture_layers: &[] }),
    scripted!(17, ArmoryFuseWeapons, 0x00CEE148, outline_armory, Activate::None, LayoutKind::Board { group: FUSE, hint: HINT_BOTTOM, top_layers: FUSE_TOP, bottom_layers: FUSE_BOTTOM, texture_layers: &[] }),
    scripted!(18, ArmoryPowers, 0x00CEC2E0, outline_armory, Activate::None, LayoutKind::Board { group: POWERS, hint: None, top_layers: POWERS_TOP, bottom_layers: &[], texture_layers: &[] }),
    scripted!(19, ArmoryWeapons, 0x00CEE1E8, outline_armory, Activate::None, LayoutKind::Board { group: WEAPON_LIST, hint: HINT_BOTTOM, top_layers: WEAPONS_TOP, bottom_layers: &[], texture_layers: &[] }),
    scripted!(20, ArmoryCategories, 0x00CEE1C0, outline_armory, Activate::None, LayoutKind::Board { group: CATEGORIES, hint: HINT_TOP, top_layers: CATEGORIES_TOP, bottom_layers: &[], texture_layers: &[] }),
    scripted!(21, PracticeRange, 0x00CF0154, outline_practice, Activate::Call(0x0030B5B4), LayoutKind::Group { group: PRACTICE, hint: HINT_PRACTICE, details: false, heart: false, top_layers: PRACTICE_TOP }),
    scripted!(22, IdolTossSolo, 0x00CF9400, outline_idol, Activate::Touch, LayoutKind::Idol),
    scripted!(23, TreasureHunt, 0x00CEEDD8, outline_treasure, Activate::Touch, LayoutKind::Group { group: TREASURE, hint: HINT_TREASURE, details: false, heart: false, top_layers: TREASURE_TOP }),
    scripted!(24, Vault, 0x00CF8248, outline_vault, Activate::Call(0x00345FDC), LayoutKind::Group { group: VAULT, hint: HINT_VAULT, details: false, heart: false, top_layers: VAULT_TOP }),
    scripted!(25, IdolTossVault, 0x00CF9400, outline_idol, Activate::Touch, LayoutKind::Idol),
    scripted!(26, Idols, 0x00CF93D8, outline_none, Activate::Touch, LayoutKind::Board { group: IDOLS, hint: None, top_layers: IDOLS_TOP, bottom_layers: IDOLS_BOTTOM, texture_layers: IDOLS_TEXTURE }),
    scripted!(27, PowerPortrait, 0x00CF9388, outline_none, Activate::Touch, LayoutKind::Board { group: POWER_PORTRAIT, hint: None, top_layers: POWER_PORTRAIT_TOP, bottom_layers: POWER_PORTRAIT_BOTTOM, texture_layers: &[] }),
    scripted!(28, ChapterResults, 0x00CF7414, outline_none, Activate::Touch, LayoutKind::Board { group: RESULTS, hint: None, top_layers: RESULTS_TOP, bottom_layers: RESULTS_BOTTOM, texture_layers: &[] }),
    ScreenSpec {
        id: Screen::FiendsCauldron,
        family: Family::Cauldron,
        identity: Identity::TaskProbe { show: CAULDRON },
        first_call: 0,
        input: InputMode::TouchesOnly,
        fallback: "ZAKURO_CAULDRON_TOP",
        outline: None,
        activate: Activate::Touch,
        layout: LayoutKind::None,
        rate: ScreenRate::Host,
    },
    ScreenSpec {
        id: Screen::Pause,
        family: Family::InGame,
        identity: Identity::CallSites(&PAUSE_MENU),
        first_call: 0,
        input: InputMode::TouchesOnly,
        fallback: "ZAKURO_PAUSE_TOP",
        outline: None,
        activate: Activate::Touch,
        layout: LayoutKind::None,
        rate: ScreenRate::Host,
    },
    ScreenSpec {
        id: Screen::RetreatDialog,
        family: Family::InGame,
        identity: Identity::CallSites(&PAUSE_MENU),
        first_call: 1,
        input: InputMode::TouchesOnly,
        fallback: "ZAKURO_PAUSE_TOP",
        outline: None,
        activate: Activate::Touch,
        layout: LayoutKind::None,
        rate: ScreenRate::Host,
    },
    // the loading between an air and a land battle, drawn by the dialogs'
    // 2D context: its top pass's tip bar, Loading orb and Tip, and the
    // Midstage Progress words its bottom pass draws through the dialogs'
    // text calls, the shared call table's from DIALOG_TEXT_CALLS on
    ScreenSpec {
        id: Screen::MidstageLoading,
        family: Family::InGame,
        identity: Identity::CallSites(&PAUSE_MENU),
        first_call: DIALOG_TEXT_CALLS,
        input: InputMode::TouchesOnly,
        fallback: "ZAKURO_PAUSE_TOP",
        outline: None,
        activate: Activate::None,
        layout: LayoutKind::None,
        rate: ScreenRate::Host,
    },
];

/// where the dialogs' words start in the shared call table: the bottom
/// pass's text calls, which the Midstage Loading's words come through too.
const DIALOG_TEXT_CALLS: usize = 3;

/// a source rectangle on the game's screen and where it shows on the top
/// screen. Drawing, hit-testing and the future editor all use this mapping.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    pub from: [f32; 4],
    pub to: [f32; 4],
}

impl Placement {
    pub fn map_x(&self, x: f32) -> f32 {
        self.to[0] + (x - self.from[0]) * self.to[2] / self.from[2]
    }

    pub fn unmap_x(&self, x: f32) -> f32 {
        self.from[0] + (x - self.to[0]) * self.from[2] / self.to[2]
    }

    pub fn map_point(&self, [x, y]: [f32; 2]) -> [f32; 2] {
        [
            self.to[0] + (x - self.from[0]) * self.to[2] / self.from[2],
            self.to[1] + (y - self.from[1]) * self.to[3] / self.from[3],
        ]
    }

    pub fn unmap_point(&self, [x, y]: [f32; 2]) -> [f32; 2] {
        [
            self.from[0] + (x - self.to[0]) * self.from[2] / self.to[2],
            self.from[1] + (y - self.to[1]) * self.from[3] / self.to[3],
        ]
    }

    /// where a box of from's screen, x, y, width and height, shows.
    pub fn to_box(&self, part: [f32; 4]) -> [f32; 4] {
        let [x, y] = self.map_point([part[0], part[1]]);
        [x, y, part[2] * self.to[2] / self.from[2], part[3] * self.to[3] / self.from[3]]
    }

    /// the same mapping over the whole top screen, 400 by 240: drawn with
    /// it, a part is moved and scaled as placed, and nothing of it is cut
    /// at its box's edges.
    pub fn whole(&self) -> Placement {
        let [x, y] = self.unmap_point([0.0, 0.0]);
        Placement { from: [x, y, 400.0 * self.from[2] / self.to[2], 240.0 * self.from[3] / self.to[3]], to: [0.0, 0.0, 400.0, 240.0] }
    }
}

/// the parts of the pause screen, each with its own placement: the kinds
/// of quad its flushes draw, and the top screen's own Hint and Paused
/// label. Center is the Panel: whatever no other part owns, such as an
/// Options page's sliders. New parts go last, so saved indices keep.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuadRole {
    Span = 0,
    Center = 1,
    LeftButton = 2,
    RightButton = 3,
    Corner = 4,
    Band = 5,
    OptionsButton = 6,
    Loot = 7,
    Row1 = 8,
    Row2 = 9,
    Row3 = 10,
    Row4 = 11,
    Back = 12,
    HintBar = 13,
    Hint = 14,
    PausedLabel = 15,
}

/// how many parts the pause screen has.
pub const PAUSE_PARTS: usize = 16;

impl QuadRole {
    pub const ALL: [QuadRole; PAUSE_PARTS] = [
        QuadRole::Span, QuadRole::Center, QuadRole::LeftButton, QuadRole::RightButton, QuadRole::Corner,
        QuadRole::Band, QuadRole::OptionsButton, QuadRole::Loot, QuadRole::Row1, QuadRole::Row2,
        QuadRole::Row3, QuadRole::Row4, QuadRole::Back, QuadRole::HintBar, QuadRole::Hint, QuadRole::PausedLabel,
    ];

    /// the Options page's rows, top to bottom.
    pub const ROWS: [QuadRole; 4] = [QuadRole::Row1, QuadRole::Row2, QuadRole::Row3, QuadRole::Row4];

    /// the name in the rules' Rust, change requests and saved layouts.
    pub fn name(self) -> &'static str {
        [
            "span", "center", "left", "right", "corner", "band", "options", "loot",
            "row1", "row2", "row3", "row4", "back", "hint_bar", "hint", "label",
        ][self as usize]
    }

    /// the editor's label, from GLOSSARY.md.
    pub fn label(self) -> &'static str {
        [
            "Dimming", "Panel", "Resume", "Retreat", "Remove Display", "Band", "Options", "Loot",
            "Option row 1", "Option row 2", "Option row 3", "Option row 4", "Back", "Hint bar", "Hint", "Paused label",
        ][self as usize]
    }

    /// whether the part is drawn by the top screen's own pass, in its 400
    /// across, rather than replayed from the bottom screen's 320.
    pub fn top(self) -> bool {
        matches!(self, QuadRole::HintBar | QuadRole::Hint | QuadRole::PausedLabel)
    }

    /// whether a click on the part touches the bottom screen there.
    pub fn control(self) -> bool {
        matches!(
            self,
            QuadRole::LeftButton | QuadRole::RightButton | QuadRole::Corner | QuadRole::OptionsButton
                | QuadRole::Row1 | QuadRole::Row2 | QuadRole::Row3 | QuadRole::Row4 | QuadRole::Back
        )
    }
}

impl From<QuadRole> for usize {
    fn from(role: QuadRole) -> usize {
        role as usize
    }
}

/// the most roles one Screen's layout seats: Pause's sixteen.
const ROLES: usize = PAUSE_PARTS;

/// how many roles the Fiend's Cauldron has.
const CAULDRON_ROLES: usize = 9;

/// one Screen's placements for a completed title frame, indexed by its
/// role enum (`QuadRole`, `CauldronRole`). A rule that was hidden has no
/// placement, so neither drawing nor hit-testing reaches it. The frame is
/// the title's own: a picture in between two titles' frames has no number
/// of its own, so it reuses the newer real frame's placements rather than
/// blending them.
#[derive(Clone, Copy, Debug)]
pub(super) struct ScreenLayout {
    frame: u64,
    placements: [Option<Placement>; ROLES],
}

impl ScreenLayout {
    pub fn place(&self, role: impl Into<usize>) -> Option<Placement> {
        self.placements[role.into()]
    }

    /// the completed title frame these placements were seated from.
    pub fn frame(&self) -> u64 {
        self.frame
    }
}

/// how one target rect follows the viewport: fixed size, narrowed by
/// `widen` about its anchor, stretched across the top screen, or fitted:
/// laid out for a screen `design_width` wide as shown, made evenly smaller
/// on a narrower one, by the edges its anchors say.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum RuleScale {
    #[allow(dead_code)]
    Fixed,
    Narrow,
    Stretch,
    Fit { design_width: f32 },
}

/// a rule is shown unless it was hidden, which layouts saved before rules
/// could be hidden have no word for.
fn shown() -> bool {
    true
}

/// one editable placement rule: the source box, the target box at 400 by
/// 240 (for `Fit`, at its design width as shown), the anchors it narrows
/// or fits about, how it follows the viewport, and whether it is shown.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlacementRule {
    pub from: [f32; 4],
    pub to: [f32; 4],
    pub anchor: f32,
    /// for `Fit`, where the room a smaller rule leaves goes: 0 keeps it by
    /// the top, 1 by the bottom. Layouts saved before it read 0.
    #[serde(default)]
    pub anchor_y: f32,
    pub scale: RuleScale,
    /// a hidden rule's group is drawn nowhere and takes no touches.
    #[serde(default = "shown")]
    pub visible: bool,
}

impl PlacementRule {
    pub fn placement(self, widen: f32) -> Placement {
        let widen = if widen.is_finite() && widen > 0.0 { widen } else { 1.0 };
        let [to_x, to_y, to_width, to_height] = self.to;
        let to = match self.scale {
            RuleScale::Fixed => self.to,
            RuleScale::Narrow => {
                let width = to_width / widen;
                [to_x + self.anchor * (to_width - width), to_y, width, to_height]
            }
            RuleScale::Stretch => [0.0, to_y, 400.0, to_height],
            RuleScale::Fit { design_width } => {
                // laid out on the screen as shown, 400 times widen across,
                // then squeezed back into the top screen's 400
                let shown = 400.0 * widen;
                let scale = (shown / design_width).min(1.0);
                let x = to_x * scale + self.anchor * (shown - design_width * scale);
                let y = to_y * scale + self.anchor_y * 240.0 * (1.0 - scale);
                [x / widen, y, to_width * scale / widen, to_height * scale]
            }
        };
        Placement { from: self.from, to }
    }

    /// the target box that places this rule at `to`, in the top screen's
    /// 400 by 240, at `widen`: what an editor drag writes back.
    pub fn target_for(self, widen: f32, to: [f32; 4]) -> [f32; 4] {
        let widen = if widen.is_finite() && widen > 0.0 { widen } else { 1.0 };
        let [x, y, width, height] = to;
        match self.scale {
            RuleScale::Fixed => to,
            RuleScale::Narrow => [x - self.anchor * (width * widen - width), y, width * widen, height],
            RuleScale::Stretch => [self.to[0], y, self.to[2], height],
            RuleScale::Fit { design_width } => {
                let shown = 400.0 * widen;
                let scale = (shown / design_width).min(1.0);
                let x = (x * widen - self.anchor * (shown - design_width * scale)) / scale;
                let y = (y - self.anchor_y * 240.0 * (1.0 - scale)) / scale;
                [x, y, width * widen / scale, height / scale]
            }
        }
    }
}

/// one rule as the Rust that writes it.
fn rule_rust(rule: PlacementRule) -> String {
    format!(
        "PlacementRule {{ from: {:?}, to: {:?}, anchor: {:?}, anchor_y: {:?}, scale: RuleScale::{:?}, visible: {:?} }}",
        rule.from, rule.to, rule.anchor, rule.anchor_y, rule.scale, rule.visible
    )
}

/// what changes from a Screen's default rules, named, one line per rule
/// and one more for a rule hidden or shown, then the acceptance.
fn change_request(screen: &str, rules: &[(&str, PlacementRule, PlacementRule)], acceptance: &str) -> String {
    let mut lines = Vec::new();
    let mut hid = false;
    for &(name, before, after) in rules {
        if before.visible != after.visible {
            hid = true;
            lines.push(format!("{} the {name} rule on {screen}.", if after.visible { "Show" } else { "Hide" }));
        }
        if before.from == after.from && before.to == after.to && before.anchor == after.anchor
            && before.anchor_y == after.anchor_y && before.scale == after.scale
        {
            continue;
        }
        lines.push(format!(
            "Move the {name} rule on {screen}: source {:?} -> {:?}, target {:?} -> {:?}, anchor {:?} -> {:?}, anchor_y {:?} -> {:?}, {:?} -> {:?}.",
            before.from, after.from, before.to, after.to, before.anchor, after.anchor, before.anchor_y, after.anchor_y, before.scale, after.scale
        ));
    }
    if lines.is_empty() {
        return format!("No changes from the default {screen} rules.\n");
    }
    lines.push(acceptance.to_owned());
    if hid {
        lines.push("Hidden rules draw nothing on the top screen and take no touches.".to_owned());
    }
    lines.join("\n") + "\n"
}

/// the pause screen's editable rules, one per part (`QuadRole`). Rules
/// saved before a part was split out load with its default; Resume's,
/// Retreat's and Remove Display's old whole-screen defaults load as their
/// boxes, which place them the same.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(from = "SavedPauseRules")]
pub struct PauseRules {
    pub span: PlacementRule,
    pub center: PlacementRule,
    pub left: PlacementRule,
    pub right: PlacementRule,
    pub corner: PlacementRule,
    pub band: PlacementRule,
    pub options: PlacementRule,
    pub loot: PlacementRule,
    pub row1: PlacementRule,
    pub row2: PlacementRule,
    pub row3: PlacementRule,
    pub row4: PlacementRule,
    pub back: PlacementRule,
    pub hint_bar: PlacementRule,
    pub hint: PlacementRule,
    pub label: PlacementRule,
}

/// a saved list of Pause rules, any of them missing.
#[derive(Deserialize)]
struct SavedPauseRules {
    span: Option<PlacementRule>,
    center: Option<PlacementRule>,
    left: Option<PlacementRule>,
    right: Option<PlacementRule>,
    corner: Option<PlacementRule>,
    band: Option<PlacementRule>,
    options: Option<PlacementRule>,
    loot: Option<PlacementRule>,
    row1: Option<PlacementRule>,
    row2: Option<PlacementRule>,
    row3: Option<PlacementRule>,
    row4: Option<PlacementRule>,
    back: Option<PlacementRule>,
    hint_bar: Option<PlacementRule>,
    hint: Option<PlacementRule>,
    label: Option<PlacementRule>,
}

impl From<SavedPauseRules> for PauseRules {
    fn from(saved: SavedPauseRules) -> PauseRules {
        let default = PauseRules::default();
        // a whole-screen rule as Resume, Retreat and Remove Display had
        // before they had boxes: the same place, from their box
        let boxed = |saved: Option<PlacementRule>, default: PlacementRule, offset: f32| match saved {
            Some(rule) if rule == whole_pause_rule(offset, rule.anchor, rule.visible) => PlacementRule { visible: rule.visible, ..default },
            Some(rule) => rule,
            None => default,
        };
        PauseRules {
            span: saved.span.unwrap_or(default.span),
            center: saved.center.unwrap_or(default.center),
            left: boxed(saved.left, default.left, 0.0),
            right: boxed(saved.right, default.right, 2.0 * PAUSE_MENU_OFFSET),
            corner: boxed(saved.corner, default.corner, 2.0 * PAUSE_MENU_OFFSET),
            band: saved.band.unwrap_or(default.band),
            options: saved.options.unwrap_or(default.options),
            loot: saved.loot.unwrap_or(default.loot),
            row1: saved.row1.unwrap_or(default.row1),
            row2: saved.row2.unwrap_or(default.row2),
            row3: saved.row3.unwrap_or(default.row3),
            row4: saved.row4.unwrap_or(default.row4),
            back: saved.back.unwrap_or(default.back),
            hint_bar: saved.hint_bar.unwrap_or(default.hint_bar),
            hint: saved.hint.unwrap_or(default.hint),
            label: saved.label.unwrap_or(default.label),
        }
    }
}

/// the rule Resume, Retreat and Remove Display had before they had boxes.
fn whole_pause_rule(offset: f32, anchor: f32, visible: bool) -> PlacementRule {
    PlacementRule { from: [0.0, 0.0, 320.0, 240.0], to: [offset, 0.0, 320.0, 240.0], anchor, anchor_y: 0.0, scale: RuleScale::Narrow, visible }
}

/// the boxes of the bottom screen's 320 by 240 the pause screen's parts are
/// drawn in: Resume, Retreat and Remove Display by its edges, the Options
/// button on the first page, the loot below it, the Options page's four
/// rows and its Back.
pub(super) const RESUME_BOX: [f32; 4] = [0.0, 55.0, 148.0, 56.0];
pub(super) const RETREAT_BOX: [f32; 4] = [172.0, 55.0, 148.0, 56.0];
pub(super) const REMOVE_DISPLAY_BOX: [f32; 4] = [193.0, 204.0, 127.0, 36.0];
const OPTIONS_BOX: [f32; 4] = [26.0, 119.0, 268.0, 30.0];
const LOOT_BOX: [f32; 4] = [3.0, 157.0, 71.0, 36.0];
/// the Options page's first row; the next are ROW_STEP lower.
const ROW_BOX: [f32; 4] = [26.0, 47.0, 268.0, 30.0];
pub(super) const ROW_STEP: f32 = 27.0;
const BACK_BOX: [f32; 4] = [2.0, 197.0, 47.0, 41.0];
/// and the top screen's own: the Hint's bar across its foot, the Hint's
/// words on it, and the Paused label in the top left corner.
const HINT_BAR_BOX: [f32; 4] = [0.0, 186.0, 400.0, 54.0];
const HINT_BOX: [f32; 4] = [90.0, 205.0, 220.0, 16.0];
const PAUSED_LABEL_BOX: [f32; 4] = [0.0, 5.0, 172.0, 34.0];

impl Default for PauseRules {
    fn default() -> PauseRules {
        let from = [0.0, 0.0, 320.0, 240.0];
        let rule = |to, anchor| PlacementRule { from, to, anchor, anchor_y: 0.0, scale: RuleScale::Narrow, visible: true };
        // a box of the bottom screen, moved right by `offset` and narrowed
        // about the bottom screen's x `about`, as the whole screen was
        let part = |from: [f32; 4], offset: f32, about: f32| PlacementRule {
            from,
            to: [from[0] + offset, from[1], from[2], from[3]],
            anchor: (about - from[0]) / from[2],
            anchor_y: 0.0,
            scale: RuleScale::Narrow,
            visible: true,
        };
        let middle = |from| part(from, PAUSE_MENU_OFFSET, 160.0);
        let row = |index: f32| middle([ROW_BOX[0], ROW_BOX[1] + index * ROW_STEP, ROW_BOX[2], ROW_BOX[3]]);
        // the top screen's own parts stay where the game draws them, and
        // narrow on a wider one as its other 2D does: the Hint about the
        // middle, the Paused label by the left edge, the bar not at all
        let top = |from: [f32; 4], about: f32, scale| PlacementRule { from, to: from, anchor: (about - from[0]) / from[2], anchor_y: 0.0, scale, visible: true };
        PauseRules {
            span: PlacementRule { scale: RuleScale::Stretch, ..rule([0.0, 0.0, 400.0, 240.0], 0.0) },
            center: rule([PAUSE_MENU_OFFSET, 0.0, 320.0, 240.0], 0.5),
            left: part(RESUME_BOX, 0.0, 0.0),
            right: part(RETREAT_BOX, 2.0 * PAUSE_MENU_OFFSET, 320.0),
            corner: part(REMOVE_DISPLAY_BOX, 2.0 * PAUSE_MENU_OFFSET, 320.0),
            band: PlacementRule { scale: RuleScale::Stretch, ..rule([0.0, 0.0, 400.0, 240.0], 0.0) },
            options: middle(OPTIONS_BOX),
            loot: middle(LOOT_BOX),
            row1: row(0.0),
            row2: row(1.0),
            row3: row(2.0),
            row4: row(3.0),
            back: middle(BACK_BOX),
            hint_bar: top(HINT_BAR_BOX, 0.0, RuleScale::Fixed),
            hint: top(HINT_BOX, 200.0, RuleScale::Narrow),
            label: top(PAUSED_LABEL_BOX, 0.0, RuleScale::Narrow),
        }
    }
}

impl PauseRules {
    pub fn rule(&self, role: QuadRole) -> PlacementRule {
        let mut rules = *self;
        *rules.rule_mut(role)
    }

    pub fn rule_mut(&mut self, role: QuadRole) -> &mut PlacementRule {
        match role {
            QuadRole::Span => &mut self.span,
            QuadRole::Center => &mut self.center,
            QuadRole::LeftButton => &mut self.left,
            QuadRole::RightButton => &mut self.right,
            QuadRole::Corner => &mut self.corner,
            QuadRole::Band => &mut self.band,
            QuadRole::OptionsButton => &mut self.options,
            QuadRole::Loot => &mut self.loot,
            QuadRole::Row1 => &mut self.row1,
            QuadRole::Row2 => &mut self.row2,
            QuadRole::Row3 => &mut self.row3,
            QuadRole::Row4 => &mut self.row4,
            QuadRole::Back => &mut self.back,
            QuadRole::HintBar => &mut self.hint_bar,
            QuadRole::Hint => &mut self.hint,
            QuadRole::PausedLabel => &mut self.label,
        }
    }

    /// the Rust row a layout change would land as.
    pub fn rust_fragment(&self) -> String {
        let rules: String = QuadRole::ALL.iter().map(|&role| format!("    {}: {},\n", role.name(), rule_rust(self.rule(role)))).collect();
        format!("PauseRules {{\n{rules}}}\n")
    }

    /// what changes from the default Pause rules, one line per rule and one
    /// more for a rule hidden or shown.
    pub fn change_request(&self) -> String {
        let default = PauseRules::default();
        let rules: Vec<_> = QuadRole::ALL.iter().map(|&role| (role.label(), default.rule(role), self.rule(role))).collect();
        change_request(
            "Pause",
            &rules,
            "Acceptance: on a Pause capture and on its Options page each part draws in its exported rectangle, and a click on a moved Control (Resume, Retreat, Options, an Option row, Back) picks it.",
        )
    }

    /// seats the rules for a completed title frame. `frame` is the title's
    /// own: in-between pictures never seat a second, blended layout.
    pub(super) fn layout(self, frame: u64, widen: f32) -> ScreenLayout {
        let mut placements = [None; ROLES];
        for role in QuadRole::ALL {
            let rule = self.rule(role);
            placements[role as usize] = rule.visible.then(|| rule.placement(widen));
        }
        ScreenLayout { frame, placements }
    }
}

/// the Retreat Dialog's editable rules: the dimming across the screen, and
/// the panel, its box, buttons and words, in the middle by default.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct DialogRules {
    pub span: PlacementRule,
    pub panel: PlacementRule,
}

impl Default for DialogRules {
    fn default() -> DialogRules {
        let pause = PauseRules::default();
        DialogRules { span: pause.span, panel: pause.center }
    }
}

impl DialogRules {
    /// the Rust row a layout change would land as.
    pub fn rust_fragment(&self) -> String {
        format!("DialogRules {{\n    span: {},\n    panel: {},\n}}\n", rule_rust(self.span), rule_rust(self.panel))
    }

    /// what changes from the default Retreat Dialog rules.
    pub fn change_request(&self) -> String {
        let default = DialogRules::default();
        change_request(
            "Retreat Dialog",
            &[("span", default.span, self.span), ("panel", default.panel, self.panel)],
            "Acceptance: on a Retreat Dialog capture the box, No, Yes and the question are drawn in the panel's rectangle, and a click on No or Yes picks it.",
        )
    }

    /// seats the rules for a completed title frame, in QuadRole order: the
    /// dimming spans, the panel is the middle, and there are no edges.
    pub(super) fn layout(self, frame: u64, widen: f32) -> ScreenLayout {
        let place = |rule: PlacementRule| rule.visible.then(|| rule.placement(widen));
        let mut placements = [None; ROLES];
        placements[QuadRole::Span as usize] = place(self.span);
        placements[QuadRole::Center as usize] = place(self.panel);
        ScreenLayout { frame, placements }
    }
}

/// the Fiend's Cauldron's parts, each in a place of its own: the top's
/// picture, its intensity panel, the bottom's bar, the bar's dial, Back,
/// Move Out, the rest (the pot, the pan and the hearts bet), the bottom
/// screen's ring background, drawn behind everything else, and the top's
/// hint line. New roles go last, so saved lists keep their order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CauldronRole {
    Top,
    Panel,
    Bar,
    Dial,
    Back,
    MoveOut,
    Rest,
    Ring,
    Hint,
}

impl CauldronRole {
    pub const ALL: [CauldronRole; CAULDRON_ROLES] = [
        CauldronRole::Top,
        CauldronRole::Panel,
        CauldronRole::Bar,
        CauldronRole::Dial,
        CauldronRole::Back,
        CauldronRole::MoveOut,
        CauldronRole::Rest,
        CauldronRole::Ring,
        CauldronRole::Hint,
    ];

    pub fn name(self) -> &'static str {
        ["top", "panel", "bar", "dial", "back", "moveout", "rest", "ring", "hint"][self as usize]
    }

    pub fn from_name(name: &str) -> Option<CauldronRole> {
        CauldronRole::ALL.into_iter().find(|role| role.name() == name)
    }
}

impl From<CauldronRole> for usize {
    fn from(role: CauldronRole) -> usize {
        role as usize
    }
}

/// the width, as shown, the Cauldron's layout is made for: 16:9.
const CAULDRON_WIDTH: f32 = 427.0;
/// where on the bottom screen the dial points for an intensity of 0 and
/// how far it goes for each 1, the box around it under the bar from its
/// arrow's tip, and how wide and high that is.
const DIAL_ZERO: f32 = 33.0;
const DIAL_STEP: f32 = 28.3;
const DIAL_TOP: f32 = 30.0;
const DIAL_SIZE: [f32; 2] = [54.0, 54.0];
/// the dial's size on the top screen against the bar's.
const DIAL_SCALE: f32 = 0.55;
/// where on the bottom screen a press of the dial goes, down the screen:
/// the dial's middle.
const DIAL_GRAB: f32 = 60.0;
/// Move Out on the bottom screen, x, y, width and height: its place has
/// room around it for it to grow when lit, and only this is touched.
const MOVE_OUT_BUTTON: [f32; 4] = [55.0, 182.0, 210.0, 58.0];
/// where the top picture's intensity panel's middle is for 0 and how far
/// it goes for each 1, in top screen pixels: it slides with the value.
const PANEL_ZERO: f32 = 64.0;
const PANEL_STEP: f32 = 30.0;

/// what the mouse does over one of the Cauldron's parts: the cursor name
/// it lights (an index into `CAULDRON_CURSOR_NAMES`), the part of the
/// role's source box that takes touches (all of it for none), and, for
/// the dial, the height of the bottom screen a press goes to, along the
/// bar.
pub(super) struct CauldronControl {
    pub role: CauldronRole,
    pub cursor: Option<usize>,
    pub hit: Option<[f32; 4]>,
    pub grab: Option<f32>,
}

/// the Cauldron's Controls in the order they are drawn over each other,
/// the dial first. The top's picture and its panel take no touches.
pub(super) const CAULDRON_CONTROLS: [CauldronControl; 5] = [
    CauldronControl { role: CauldronRole::Dial, cursor: Some(0), hit: None, grab: Some(DIAL_GRAB) },
    CauldronControl { role: CauldronRole::Bar, cursor: Some(0), hit: None, grab: None },
    CauldronControl { role: CauldronRole::Back, cursor: Some(2), hit: None, grab: None },
    CauldronControl { role: CauldronRole::MoveOut, cursor: Some(1), hit: Some(MOVE_OUT_BUTTON), grab: None },
    CauldronControl { role: CauldronRole::Rest, cursor: None, hit: None, grab: None },
];

/// the Cauldron's editable rules, one per `CauldronRole`. The panel's and
/// the dial's are where they are for an intensity of 0; seating slides
/// them along the bar with the pot's value. A saved list from before a
/// role was added gets that role's default.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(from = "Vec<PlacementRule>")]
pub struct CauldronRules([PlacementRule; CAULDRON_ROLES]);

impl From<Vec<PlacementRule>> for CauldronRules {
    fn from(saved: Vec<PlacementRule>) -> CauldronRules {
        let mut rules = CauldronRules::default();
        for (rule, saved) in rules.0.iter_mut().zip(saved) {
            *rule = saved;
        }
        rules
    }
}

impl Default for CauldronRules {
    /// the top's picture and under it the panel, the bar, its dial and Back
    /// on the left, the pot over Move Out on the right with the ring
    /// behind it, the hint across the top, made for 16:9 and smaller on a
    /// narrower screen.
    fn default() -> CauldronRules {
        let rule = |to, from, anchor, anchor_y| PlacementRule {
            from,
            to,
            anchor,
            anchor_y,
            scale: RuleScale::Fit { design_width: CAULDRON_WIDTH },
            visible: true,
        };
        let fit = |to, from, right: bool, down: bool| rule(to, from, if right { 1.0 } else { 0.0 }, if down { 1.0 } else { 0.0 });
        let (bar_to, bar_from) = ([40.0, 197.0, 220.0, 23.0], [14.0, 0.0, 312.0, 32.0]);
        let (along, down) = (bar_to[2] / bar_from[2], bar_to[3] / bar_from[3]);
        // at 0, the dial's arrow tip on the bar's lower edge, the panel's
        // middle over the dial
        let dial_middle = bar_to[0] + (DIAL_ZERO - bar_from[0]) * along;
        let dial = [DIAL_SIZE[0] * along * DIAL_SCALE, DIAL_SIZE[1] * down * DIAL_SCALE];
        let panel_width = 91.0;
        CauldronRules([
            fit([16.0, 24.0, 258.0, 172.0], [36.0, 22.0, 328.0, 218.0], false, false),
            fit([dial_middle - panel_width * 0.5, 150.0, panel_width, 47.0], [PANEL_ZERO - 70.0, 170.0, 140.0, 72.0], false, true),
            fit(bar_to, bar_from, false, true),
            fit(
                [dial_middle - dial[0] * 0.5, bar_to[1] + (DIAL_TOP - bar_from[1]) * down, dial[0], dial[1]],
                [DIAL_ZERO - DIAL_SIZE[0] * 0.5, DIAL_TOP, DIAL_SIZE[0], DIAL_SIZE[1]],
                false,
                true,
            ),
            fit([9.0, 207.0, 33.0, 30.0], [6.0, 202.0, 42.0, 38.0], false, true),
            fit([244.9, 192.9, 171.1, 47.5], [16.0, 160.0, 288.0, 80.0], true, true),
            fit([188.0, 47.0, 237.4, 178.1], [0.0, 0.0, 320.0, 240.0], true, true),
            fit([188.0, 47.0, 237.4, 178.1], [0.0, 0.0, 320.0, 240.0], true, true),
            // the hint's line as the game draws it, centred across the top;
            // its box is the line's, so it does not cover the other parts
            rule([CAULDRON_WIDTH * 0.5 - 164.0, 2.0, 328.0, 22.0], [36.0, 2.0, 328.0, 22.0], 0.5, 0.0),
        ])
    }
}

impl CauldronRules {
    pub fn rule(&self, role: CauldronRole) -> PlacementRule {
        self.0[role as usize]
    }

    pub fn rule_mut(&mut self, role: CauldronRole) -> &mut PlacementRule {
        &mut self.0[role as usize]
    }

    /// every role's placement at the pot's value, hidden ones too: the
    /// panel's and the dial's source slide with it, and both sit centred
    /// on the bar where it shows the value, wherever the bar is.
    pub fn boxes(&self, widen: f32, value: f32) -> [Placement; CAULDRON_ROLES] {
        let mut boxes = CauldronRole::ALL.map(|role| self.rule(role).placement(widen));
        let point = boxes[CauldronRole::Bar as usize].map_x(DIAL_ZERO + value * DIAL_STEP);
        for (role, from_step) in [(CauldronRole::Panel, PANEL_STEP), (CauldronRole::Dial, DIAL_STEP)] {
            let place = &mut boxes[role as usize];
            place.from[0] += value * from_step;
            place.to[0] = point - place.to[2] * 0.5;
        }
        boxes
    }

    /// moves a role so that it is placed at `to` in the top screen's 400 by
    /// 240; the panel and the dial keep their place across, on the bar.
    pub fn set_box(&mut self, role: CauldronRole, widen: f32, to: [f32; 4]) {
        let rule = self.rule_mut(role);
        rule.to = rule.target_for(widen, to);
    }

    /// seats the rules for a completed title frame at the pot's value: the
    /// panel and the dial slide along the bar with it, and have no place
    /// without one.
    pub(super) fn layout(&self, frame: u64, widen: f32, value: Option<f32>) -> ScreenLayout {
        let boxes = self.boxes(widen, value.unwrap_or(0.0));
        let mut placements = [None; ROLES];
        for role in CauldronRole::ALL {
            let slides = matches!(role, CauldronRole::Panel | CauldronRole::Dial);
            placements[role as usize] = (self.rule(role).visible && (value.is_some() || !slides)).then_some(boxes[role as usize]);
        }
        ScreenLayout { frame, placements }
    }

    /// the Rust a layout change would land as, for `Default`.
    pub fn rust_fragment(&self) -> String {
        let rules: String = CauldronRole::ALL.iter().map(|&role| format!("    // {}\n    {},\n", role.name(), rule_rust(self.rule(role)))).collect();
        format!("CauldronRules([\n{rules}])\n")
    }

    /// what changes from the default Cauldron rules, one line per rule
    /// and one more for a rule hidden or shown.
    pub fn change_request(&self) -> String {
        let default = CauldronRules::default();
        let rules: Vec<_> = CauldronRole::ALL.iter().map(|&role| (role.name(), default.rule(role), self.rule(role))).collect();
        change_request(
            "Fiend's Cauldron",
            &rules,
            "Acceptance: on a Fiend's Cauldron capture at 5:3, 16:9 and 21:9 the seven rules resolve to the exported rectangles, and hovering each Control lights it.",
        )
    }
}

/// the Midstage Loading's parts, each with a rule of its own: the grey bar
/// the tip is on, the tip's words, the spinning Loading orb, and the
/// bottom screen's Midstage Progress words. New roles go last, so saved
/// lists keep their order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoadingRole {
    TipBar,
    Tip,
    Orb,
    Progress,
}

/// how many roles the Midstage Loading has.
const LOADING_ROLES: usize = 4;

impl LoadingRole {
    pub const ALL: [LoadingRole; LOADING_ROLES] = [LoadingRole::TipBar, LoadingRole::Tip, LoadingRole::Orb, LoadingRole::Progress];

    /// the name in the rules' Rust and change requests.
    pub fn name(self) -> &'static str {
        ["tip bar", "tip", "Loading orb", "Midstage Progress"][self as usize]
    }

    /// the editor's label, from GLOSSARY.md.
    pub fn label(self) -> &'static str {
        ["Tip bar", "Tip", "Loading orb", "Midstage Progress"][self as usize]
    }
}

impl From<LoadingRole> for usize {
    fn from(role: LoadingRole) -> usize {
        role as usize
    }
}

/// the tip bar on the top screen as the game draws it: across the bottom,
/// 59 high. The tip's words are drawn inside it.
const TIP_BAR: [f32; 4] = [0.0, 181.0, 400.0, 59.0];
/// the Loading orb's one turning quad, 64 square about its middle at
/// (196.6, 105.3) on the top screen.
const LOADING_ORB: [f32; 4] = [164.6, 73.3, 64.0, 64.0];
/// the Midstage Progress words on the bottom screen: the heading at the top
/// left to the last line's foot.
const MIDSTAGE_PROGRESS: [f32; 4] = [0.0, 0.0, 320.0, 230.0];

/// the Midstage Loading's editable rules, one per `LoadingRole`. A saved
/// list from before a role was added gets that role's default.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(from = "Vec<PlacementRule>")]
pub struct LoadingRules([PlacementRule; LOADING_ROLES]);

impl From<Vec<PlacementRule>> for LoadingRules {
    fn from(saved: Vec<PlacementRule>) -> LoadingRules {
        let mut rules = LoadingRules::default();
        for (rule, saved) in rules.0.iter_mut().zip(saved) {
            *rule = saved;
        }
        rules
    }
}

impl Default for LoadingRules {
    /// the tip bar and its tip where the game draws them; Midstage Progress
    /// above them at four fifths of its size, in the middle, clear of the
    /// bar; the Loading orb by the right edge, just above the bar.
    fn default() -> LoadingRules {
        let rule = |from, to, anchor, scale| PlacementRule { from, to, anchor, anchor_y: 0.0, scale, visible: true };
        let progress = 0.8;
        let progress_size = [MIDSTAGE_PROGRESS[2] * progress, MIDSTAGE_PROGRESS[3] * progress];
        LoadingRules([
            rule(TIP_BAR, TIP_BAR, 0.0, RuleScale::Stretch),
            rule(TIP_BAR, TIP_BAR, 0.5, RuleScale::Narrow),
            rule(LOADING_ORB, [336.0, 125.0, 48.0, 48.0], 1.0, RuleScale::Narrow),
            rule(MIDSTAGE_PROGRESS, [200.0 - progress_size[0] * 0.5, 0.0, progress_size[0], progress_size[1]], 0.5, RuleScale::Narrow),
        ])
    }
}

impl LoadingRules {
    pub fn rule(&self, role: LoadingRole) -> PlacementRule {
        self.0[role as usize]
    }

    pub fn rule_mut(&mut self, role: LoadingRole) -> &mut PlacementRule {
        &mut self.0[role as usize]
    }

    /// every role's placement, hidden ones too.
    pub fn boxes(&self, widen: f32) -> [Placement; LOADING_ROLES] {
        LoadingRole::ALL.map(|role| self.rule(role).placement(widen))
    }

    /// moves a role so that it is placed at `to` in the top screen's 400 by
    /// 240.
    pub fn set_box(&mut self, role: LoadingRole, widen: f32, to: [f32; 4]) {
        let rule = self.rule_mut(role);
        rule.to = rule.target_for(widen, to);
    }

    /// seats the rules for a completed title frame, by `LoadingRole`.
    pub(super) fn layout(&self, frame: u64, widen: f32) -> ScreenLayout {
        let mut placements = [None; ROLES];
        for role in LoadingRole::ALL {
            placements[role as usize] = self.rule(role).visible.then(|| self.rule(role).placement(widen));
        }
        ScreenLayout { frame, placements }
    }

    /// the Rust a layout change would land as, for `Default`.
    pub fn rust_fragment(&self) -> String {
        let rules: String = LoadingRole::ALL.iter().map(|&role| format!("    // {}\n    {},\n", role.name(), rule_rust(self.rule(role)))).collect();
        format!("LoadingRules([\n{rules}])\n")
    }

    /// what changes from the default Midstage Loading rules, one line per
    /// rule and one more for a rule hidden or shown.
    pub fn change_request(&self) -> String {
        let default = LoadingRules::default();
        let rules: Vec<_> = LoadingRole::ALL.iter().map(|&role| (role.name(), default.rule(role), self.rule(role))).collect();
        change_request(
            "Midstage Loading",
            &rules,
            "Acceptance: on the loading between an air and a land battle at 5:3 and 16:9, the tip bar, the tip, the Loading orb and Midstage Progress draw in their rules' rectangles, and none covers another.",
        )
    }
}

/// the process's state for the Screens it knows: the calls captured in the
/// bottom pass, the frame a Screen was last drawn on the top, and each
/// Screen's High Refresh Mode rate.
pub(super) struct ScreenTransform {
    recorded: [Option<Recorded>; PAUSE_MENU.len()],
    drawn: Option<u64>,
    /// whether a Hover took the Retreat Dialog's light off its buttons.
    dialog_cleared: bool,
    /// the frame the Fiend's Cauldron was last shown, whether its pot was
    /// pouring then, and the pot's task.
    cauldron_drawn: Option<u64>,
    cauldron_pouring: bool,
    cauldron_task: Option<u32>,
    /// the Cauldron's editable rules, and their placements seated on the
    /// frame it was last shown, which drawing and hit-testing both read,
    /// with the width and the pot's value they were seated at.
    cauldron_rules: CauldronRules,
    cauldron_layout: Option<ScreenLayout>,
    cauldron_seat: (f32, Option<f32>),
    /// the pause screen's editable placement rules, and the Retreat
    /// Dialog's over it.
    pause_rules: PauseRules,
    dialog_rules: DialogRules,
    /// the Midstage Loading's editable rules, and the frame its top pass
    /// last drew it.
    loading_rules: LoadingRules,
    loading_drawn: Option<u64>,
    /// each Screen's High Refresh Mode rate, declared on its row and
    /// overridable while the title runs. A Screen capped at 60 shows real
    /// frames only, instead of being excluded from the mode.
    rates: [ScreenRate; SCREENS.len()],
    /// the Screen last drawn on the top screen and the completed frame it
    /// was found on. A picture in between has no frame of its own, so it
    /// reads this one: it never seats placements of its own.
    active: Option<(Screen, u64)>,
}

impl Default for ScreenTransform {
    fn default() -> Self {
        ScreenTransform {
            recorded: [None; PAUSE_MENU.len()],
            drawn: None,
            dialog_cleared: false,
            cauldron_drawn: None,
            cauldron_pouring: false,
            cauldron_task: None,
            cauldron_rules: CauldronRules::default(),
            cauldron_layout: None,
            cauldron_seat: (1.0, None),
            pause_rules: PauseRules::default(),
            dialog_rules: DialogRules::default(),
            loading_rules: LoadingRules::default(),
            loading_drawn: None,
            rates: std::array::from_fn(|index| SCREENS[index].rate),
            active: None,
        }
    }
}

impl ScreenTransform {
    pub fn spec(id: Screen) -> &'static ScreenSpec {
        SCREENS.iter().find(|spec| spec.id == id).expect("every Screen is registered")
    }

    pub fn scripted(index: usize) -> &'static ScreenSpec {
        &SCREENS[index]
    }

    pub fn scripted_len() -> usize {
        SCRIPTED_COUNT
    }

    pub fn scripted_at_update(pc: u32) -> Option<usize> {
        SCREENS[..SCRIPTED_COUNT].iter().position(|spec| spec.update() == Some(pc))
    }

    /// seats the Pause placements for a completed title frame, or the
    /// Retreat Dialog's while it is shown. Drawing and hit-testing both
    /// call this with the same real frame; a picture in between has no
    /// frame of its own and reads the newest real one, never a blended
    /// placement.
    pub fn layout(&self, frame: u64, widen: f32) -> ScreenLayout {
        self.screen_layout(if self.active(frame) == Some(Screen::RetreatDialog) { Screen::RetreatDialog } else { Screen::Pause }, frame, widen)
    }

    /// the placements of one of the shared call table's Screens, Pause or
    /// the Retreat Dialog.
    pub fn screen_layout(&self, screen: Screen, frame: u64, widen: f32) -> ScreenLayout {
        match screen {
            Screen::RetreatDialog => self.dialog_rules.layout(frame, widen),
            Screen::MidstageLoading => self.loading_rules.layout(frame, widen),
            _ => self.pause_rules.layout(frame, widen),
        }
    }

    /// the layout for the picture the presenter shows: a picture in between
    /// two title frames has no frame of its own, so the newest real frame's
    /// placements are used and its t never reaches geometry. Only a real
    /// frame ever seats a Placement.
    pub fn layout_for(&self, frame: u64, widen: f32, shown: crate::PictureKind) -> ScreenLayout {
        let _ = shown;
        self.layout(frame, widen)
    }

    pub fn pause_rules(&self) -> PauseRules {
        self.pause_rules
    }

    #[allow(dead_code)]
    pub fn set_pause_rules(&mut self, rules: PauseRules) {
        self.pause_rules = rules;
    }

    pub fn dialog_rules(&self) -> DialogRules {
        self.dialog_rules
    }

    pub fn set_dialog_rules(&mut self, rules: DialogRules) {
        self.dialog_rules = rules;
    }

    pub fn loading_rules(&self) -> LoadingRules {
        self.loading_rules
    }

    pub fn set_loading_rules(&mut self, rules: LoadingRules) {
        self.loading_rules = rules;
    }

    /// the Midstage Loading's top pass drew it on a completed frame.
    pub fn mark_loading(&mut self, frame: u64) {
        self.loading_drawn = Some(frame);
        self.mark_active(Screen::MidstageLoading, frame);
    }

    /// whether the Midstage Loading was drawn lately, within the same grace
    /// as the other Screens.
    pub fn loading_shown(&self, frame: u64) -> bool {
        self.loading_drawn.is_some_and(|drawn| frame <= drawn + 3)
    }

    fn index(screen: Screen) -> usize {
        SCREENS.iter().position(|spec| spec.id == screen).expect("every Screen is registered")
    }

    /// the Screen's High Refresh Mode rate: its row's, unless overridden.
    pub fn rate(&self, screen: Screen) -> ScreenRate {
        self.rates[Self::index(screen)]
    }

    /// caps a Screen at 60 (real frames only), or returns it to the host's
    /// rate. Every Screen, from any family, may be capped this way.
    pub fn set_rate(&mut self, screen: Screen, rate: ScreenRate) {
        self.rates[Self::index(screen)] = rate;
    }

    /// whether a Screen may be drawn in between. Only a Screen found to
    /// misbehave by category takes the 60 cap.
    pub fn capped(&self, screen: Screen) -> bool {
        self.rate(screen) == ScreenRate::Sixty
    }

    /// marks the Screen the top screen drew, on a completed frame.
    pub fn mark_active(&mut self, screen: Screen, frame: u64) {
        self.active = Some((screen, frame));
    }

    /// the Screen shown at `frame`, within the same grace as the drawn
    /// calls, so a cap lapses with the Screen.
    pub fn active(&self, frame: u64) -> Option<Screen> {
        self.active.filter(|&(_, at)| frame <= at + 3).map(|(screen, _)| screen)
    }

    /// the Screen last drawn on the top screen, however long ago: the one
    /// the layout editor edits.
    pub fn last_active(&self) -> Option<Screen> {
        self.active.map(|(screen, _)| screen)
    }

    /// whether the picture at `frame` must be the title's own: the Screen
    /// shown then is capped at 60. The host's presenter asks this before it
    /// shows a picture in between.
    pub fn real_frames_only(&self, frame: u64) -> bool {
        self.active(frame).is_some_and(|screen| self.capped(screen))
    }

    /// Clears what the last frames recorded. The host's editable rules and
    /// its per-Screen caps are not the console's state, so a load keeps
    /// them.
    pub fn reset(&mut self) {
        let (pause_rules, dialog_rules, cauldron_rules, loading_rules, rates) =
            (self.pause_rules, self.dialog_rules, self.cauldron_rules, self.loading_rules, self.rates);
        *self = Self::default();
        self.pause_rules = pause_rules;
        self.dialog_rules = dialog_rules;
        self.loading_rules = loading_rules;
        self.cauldron_rules = cauldron_rules;
        self.rates = rates;
    }

    pub fn pause_calls() -> &'static [ToTop] {
        Self::spec(Screen::Pause).calls()
    }

    pub fn pause_call(index: usize) -> ToTop {
        Self::pause_calls()[index]
    }

    pub fn pause_site(pc: u32) -> bool {
        Self::pause_calls().iter().any(|call| call.site == pc || call.place == pc)
    }

    pub fn pause_call_at(pc: u32) -> Option<usize> {
        Self::pause_calls().iter().position(|call| call.site == pc)
    }

    pub fn pause_place_at(pc: u32) -> Option<usize> {
        Self::pause_calls().iter().position(|call| call.place == pc)
    }

    pub fn record(&mut self, index: usize, call: Option<Recorded>) {
        self.recorded[index] = call;
    }

    pub fn has(&self, index: usize) -> bool {
        self.recorded[index].is_some()
    }

    pub fn take(&mut self, index: usize) -> Option<Recorded> {
        self.recorded[index].take()
    }

    /// the Pause screen or its dialog drew on a completed frame, at `index`
    /// in the shared call table: 0 is Pause, the rest are its dialogs.
    pub fn mark_drawn(&mut self, frame: u64, index: usize) {
        self.drawn = Some(frame);
        self.mark_active(Self::pause_screen(index), frame);
    }

    /// the Screen a call table entry belongs to: Pause itself, or the dialog
    /// over it.
    pub fn pause_screen(index: usize) -> Screen {
        if index == 0 { Screen::Pause } else { Screen::RetreatDialog }
    }

    pub fn shown(&self, frame: u64) -> bool {
        self.drawn.is_some_and(|drawn| frame <= drawn + 3)
    }

    pub fn set_dialog_cleared(&mut self, cleared: bool) {
        self.dialog_cleared = cleared;
    }

    pub fn take_dialog_cleared(&mut self) -> bool {
        std::mem::take(&mut self.dialog_cleared)
    }

    /// whether `pc` is the Cauldron's shown or pouring probe.
    pub fn cauldron_site(pc: u32) -> Option<bool> {
        match Self::spec(Screen::FiendsCauldron).identity {
            Identity::TaskProbe { show } if show[0] == pc => Some(false),
            Identity::TaskProbe { show } if show[1] == pc => Some(true),
            _ => None,
        }
    }

    /// the pot's task ran on a completed frame: the Cauldron is shown, and
    /// its placements are seated for the pot's value, none when it has no
    /// sensible one.
    pub fn mark_cauldron(&mut self, frame: u64, task: u32, pouring: bool, widen: f32, value: Option<f32>) {
        self.cauldron_drawn = Some(frame);
        self.cauldron_task = Some(task);
        self.cauldron_pouring = pouring;
        self.cauldron_seat = (widen, value);
        self.cauldron_layout = Some(self.cauldron_rules.layout(frame, widen, value));
        self.mark_active(Screen::FiendsCauldron, frame);
    }

    pub fn cauldron_shown(&self, frame: u64) -> bool {
        self.cauldron_drawn.is_some_and(|drawn| frame <= drawn + 3)
    }

    pub fn cauldron_pouring(&self, frame: u64) -> bool {
        self.cauldron_shown(frame) && self.cauldron_pouring
    }

    /// the Cauldron's placements seated on the frame it was last shown,
    /// while it is.
    pub fn cauldron_layout(&self, frame: u64) -> Option<ScreenLayout> {
        self.cauldron_layout.filter(|_| self.cauldron_shown(frame))
    }

    pub fn cauldron_rules(&self) -> CauldronRules {
        self.cauldron_rules
    }

    /// the width and the pot's value the Cauldron was last seated at.
    pub fn cauldron_seat(&self) -> (f32, Option<f32>) {
        self.cauldron_seat
    }

    /// seats the Cauldron again at once, at the same frame, width and value,
    /// so drawing and hit-testing follow an edit before its next frame.
    pub fn set_cauldron_rules(&mut self, rules: CauldronRules) {
        self.cauldron_rules = rules;
        let (widen, value) = self.cauldron_seat;
        if let Some(layout) = &mut self.cauldron_layout {
            *layout = rules.layout(layout.frame, widen, value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// the Midstage Loading's default parts keep clear of each other at
    /// 5:3, 16:9 and 21:9: Midstage Progress above the tip bar, the Loading
    /// orb beside the words, the tip inside its bar.
    #[test]
    fn loading_parts_keep_apart() {
        let overlap = |a: [f32; 4], b: [f32; 4]| a[0] < b[0] + b[2] && b[0] < a[0] + a[2] && a[1] < b[1] + b[3] && b[1] < a[1] + a[3];
        for widen in [1.0, 427.0 / 400.0, 533.0 / 400.0] {
            let layout = LoadingRules::default().layout(10, widen);
            let place = |role| layout.place(role).expect("every part is shown by default").to;
            let (bar, tip, orb, progress) = (place(LoadingRole::TipBar), place(LoadingRole::Tip), place(LoadingRole::Orb), place(LoadingRole::Progress));
            // the words' own extent: the heading's left to the stats' right
            let words = Placement { from: MIDSTAGE_PROGRESS, to: progress }.to_box([8.0, 8.0, 250.0, 214.0]);
            assert!(!overlap(words, bar) && !overlap(orb, bar) && !overlap(words, orb), "{widen}: {words:?} {orb:?} {bar:?}");
            assert!(tip[1] >= bar[1] && tip[1] + tip[3] <= bar[1] + bar[3] + 0.001, "{widen}: the tip stays in its bar");
            assert!(orb[0] + orb[2] <= 400.001 && progress[0] >= 0.0, "{widen}: on the screen");
        }
    }

    /// a hidden Midstage Loading rule has no placement, and a list saved
    /// before a role was added gives it its default.
    #[test]
    fn loading_rules_hide_and_load_older_lists() {
        let mut rules = LoadingRules::default();
        rules.rule_mut(LoadingRole::Orb).visible = false;
        assert!(rules.layout(10, 1.0).place(LoadingRole::Orb).is_none());
        assert!(rules.layout(10, 1.0).place(LoadingRole::Progress).is_some());
        let saved: Vec<PlacementRule> = LoadingRole::ALL[..2].iter().map(|&role| rules.rule(role)).collect();
        let loaded = LoadingRules::from(saved);
        assert_eq!(loaded.rule(LoadingRole::Orb), LoadingRules::default().rule(LoadingRole::Orb));
        assert!(LoadingRules::default().change_request().contains("No changes from the default Midstage Loading rules"));
        assert!(rules.change_request().contains("Hide the Loading orb rule on Midstage Loading."));
    }

    /// moving a part in the editor lands where it was put, at any width,
    /// and a reset keeps the rules across a console state load.
    #[test]
    fn loading_rules_follow_the_editor_and_survive_a_reset() {
        let mut transform = ScreenTransform::default();
        let mut rules = LoadingRules::default();
        let widen = 427.0 / 400.0;
        rules.set_box(LoadingRole::Progress, widen, [20.0, 10.0, 200.0, 150.0]);
        let moved = rules.layout(10, widen).place(LoadingRole::Progress).unwrap().to;
        assert!(moved.iter().zip([20.0, 10.0, 200.0, 150.0]).all(|(a, b)| (a - b).abs() < 0.001), "{moved:?}");
        transform.set_loading_rules(rules);
        transform.mark_loading(10);
        assert_eq!(transform.active(11), Some(Screen::MidstageLoading));
        assert!(transform.loading_shown(13) && !transform.loading_shown(14));
        transform.reset();
        assert_eq!(transform.loading_rules(), rules);
        assert!(!transform.loading_shown(10));
    }

    #[test]
    fn every_screen_is_registered_once() {
        for (index, spec) in SCREENS.iter().enumerate() {
            assert!(SCREENS[index + 1..].iter().all(|other| other.id != spec.id));
            match spec.identity {
                Identity::CallSites(_) => assert!(!spec.calls().is_empty()),
                Identity::Vtable { vtable, update } => {
                    assert_ne!(vtable, 0);
                    assert_ne!(update, 0);
                    assert!(spec.outline.is_some());
                }
                Identity::TaskProbe { show } => assert!(show.iter().all(|&site| site != 0)),
            }
        }
    }

    #[test]
    fn scripted_rows_keep_their_order() {
        assert_eq!(SCRIPTED_COUNT, 29);
        for (index, update) in SCRIPTED_UPDATES.iter().enumerate() {
            assert_eq!(SCREENS[index].update(), Some(*update));
        }
    }

    #[test]
    fn pause_rules_follow_the_width_about_their_anchor() {
        let layout = PauseRules::default().layout(10, 1.4);
        let place = |role| layout.place(role).unwrap();
        let near = |value: f32, expected: f32| (value - expected).abs() < 0.001;
        assert!(near(place(QuadRole::Center).to[0], 200.0 - 160.0 / 1.4));
        assert!(near(place(QuadRole::Center).to[2], 320.0 / 1.4));
        assert!(near(place(QuadRole::RightButton).map_x(320.0), 400.0));
        assert!(near(place(QuadRole::RightButton).map_x(172.0), 400.0 - 148.0 / 1.4));
        assert!(near(place(QuadRole::LeftButton).map_x(0.0), 0.0));
        // a split part narrows about the same middle as the Panel
        for role in [QuadRole::Row2, QuadRole::Back, QuadRole::Loot, QuadRole::OptionsButton] {
            for x in [30.0, 160.0, 290.0] {
                assert!(near(place(role).map_x(x), place(QuadRole::Center).map_x(x)), "{role:?} at {x}");
            }
        }
        assert_eq!(place(QuadRole::Span).to[2], 400.0);
    }

    /// the default rules keep the Cauldron where its hand-made layout put
    /// it: made for a 16:9 window as shown, evenly smaller on a narrower
    /// one, by the left or right and the top or bottom edge, then squeezed
    /// into the top screen's 400. the panel and the dial slide along the
    /// bar with the pot's value.
    #[test]
    fn cauldron_rules_fit_the_window_and_follow_the_value() {
        let near = |a: [f32; 4], b: [f32; 4]| a.iter().zip(b).all(|(a, b)| (a - b).abs() < 0.001);
        for widen in [1.0, 427.0 / 400.0, 533.0 / 400.0, 300.0 / 400.0] {
            let shown = 400.0 * widen;
            let scale = (shown / 427.0_f32).min(1.0);
            let fitted = |to: [f32; 4], right: bool, down: bool| {
                let x = to[0] * scale + if right { shown - 427.0 * scale } else { 0.0 };
                let y = to[1] * scale + if down { 240.0 * (1.0 - scale) } else { 0.0 };
                [x / widen, y, to[2] * scale / widen, to[3] * scale]
            };
            let layout = CauldronRules::default().layout(10, widen, Some(4.0));
            let to = |role| layout.place(role).unwrap().to;
            assert!(near(to(CauldronRole::Top), fitted([16.0, 24.0, 258.0, 172.0], false, false)), "{widen}");
            assert!(near(to(CauldronRole::Bar), fitted([40.0, 197.0, 220.0, 23.0], false, true)), "{widen}");
            assert!(near(to(CauldronRole::Back), fitted([9.0, 207.0, 33.0, 30.0], false, true)), "{widen}");
            assert!(near(to(CauldronRole::MoveOut), fitted([244.9, 192.9, 171.1, 47.5], true, true)), "{widen}");
            assert!(near(to(CauldronRole::Rest), fitted([188.0, 47.0, 237.4, 178.1], true, true)), "{widen}");
            // the dial's arrow on the bar where the bottom screen's points,
            // 33 + 28.3 per 1, its tip on the bar's lower edge
            let bar = layout.place(CauldronRole::Bar).unwrap();
            let dial = layout.place(CauldronRole::Dial).unwrap();
            let tip = DIAL_ZERO + 4.0 * DIAL_STEP;
            assert!((dial.from[0] + dial.from[2] * 0.5 - tip).abs() < 0.001);
            assert!((dial.to[0] + dial.to[2] * 0.5 - bar.map_x(tip)).abs() < 0.001);
            assert!((dial.to[1] - bar.map_point([0.0, DIAL_TOP])[1]).abs() < 0.001);
            // the panel's middle over it, showing the top's panel for 4
            let panel = layout.place(CauldronRole::Panel).unwrap();
            assert!((panel.to[0] + panel.to[2] * 0.5 - bar.map_x(tip)).abs() < 0.001);
            assert!((panel.from[0] + panel.from[2] * 0.5 - (PANEL_ZERO + 4.0 * PANEL_STEP)).abs() < 0.001);
        }
        // with no value there is nothing to point at
        let layout = CauldronRules::default().layout(10, 1.0, None);
        assert!(layout.place(CauldronRole::Dial).is_none() && layout.place(CauldronRole::Panel).is_none());
        assert!(layout.place(CauldronRole::Bar).is_some());
    }

    /// a moved or resized bar, or a dial and panel moved across, keeps both
    /// centred on the bar's point for the value as it slides.
    #[test]
    fn the_dial_and_panel_stay_on_an_edited_bar() {
        let mut rules = CauldronRules::default();
        rules.rule_mut(CauldronRole::Bar).to = [60.0, 190.0, 250.0, 26.0];
        rules.rule_mut(CauldronRole::Dial).to[0] += 7.0;
        rules.rule_mut(CauldronRole::Panel).to[0] -= 5.0;
        for widen in [1.0, 533.0 / 400.0] {
            for value in [0.0, 2.1, 8.4, 9.0] {
                let layout = rules.layout(10, widen, Some(value));
                let point = layout.place(CauldronRole::Bar).unwrap().map_x(DIAL_ZERO + value * DIAL_STEP);
                for role in [CauldronRole::Dial, CauldronRole::Panel] {
                    let place = layout.place(role).unwrap();
                    assert!((place.to[0] + place.to[2] * 0.5 - point).abs() < 0.001, "{role:?} at {value}, widen {widen}");
                }
            }
        }
    }

    /// a placement drawn over the whole screen maps every point as the
    /// placement does, so a part is moved and scaled but never cut.
    #[test]
    fn a_whole_screen_placement_keeps_the_mapping() {
        let dial = CauldronRules::default().layout(10, 427.0 / 400.0, Some(8.4)).place(CauldronRole::Dial).unwrap();
        let whole = dial.whole();
        assert_eq!(whole.to, [0.0, 0.0, 400.0, 240.0]);
        for point in [[0.0, 0.0], [dial.from[0], dial.from[1]], [250.0, 60.0], [-40.0, 300.0]] {
            let [a, b] = [whole.map_point(point), dial.map_point(point)];
            assert!((a[0] - b[0]).abs() < 0.001 && (a[1] - b[1]).abs() < 0.001, "{point:?}");
        }
    }

    #[test]
    fn a_hidden_cauldron_role_has_no_placement() {
        let mut rules = CauldronRules::default();
        rules.rule_mut(CauldronRole::Back).visible = false;
        let layout = rules.layout(10, 1.0, Some(2.0));
        assert!(layout.place(CauldronRole::Back).is_none());
        assert!(CauldronRole::ALL.into_iter().filter(|&role| role != CauldronRole::Back).all(|role| layout.place(role).is_some()));
    }

    #[test]
    fn rules_export_rust_and_a_change_request() {
        let rules = PauseRules::default();
        assert!(rules.rust_fragment().contains("center: PlacementRule"));
        assert!(rules.change_request().contains("No changes from the default Pause rules"));

        let mut edited = rules;
        edited.center.to[0] = 60.0;
        let request = edited.change_request();
        assert!(request.contains("Move the Panel rule on Pause"));
        assert!(request.contains("Acceptance:"));
    }

    #[test]
    fn pause_rules_are_editable_data() {
        let mut rules = PauseRules::default();
        rules.center.to[0] = 60.0;
        let mut transform = ScreenTransform::default();
        transform.set_pause_rules(rules);
        assert_eq!(transform.layout(10, 1.0).place(QuadRole::Center).unwrap().map_x(160.0), 220.0);
    }

    /// Loading a saved state clears what a frame recorded, but the host's
    /// editable Pause and Retreat Dialog rules are not part of the
    /// console's state.
    #[test]
    fn reset_keeps_editor_rules() {
        let mut rules = PauseRules::default();
        rules.center.to[0] = 60.0;
        rules.left.scale = RuleScale::Fixed;
        let mut dialog = DialogRules::default();
        dialog.panel.visible = false;
        let mut transform = ScreenTransform::default();
        transform.set_pause_rules(rules);
        transform.set_dialog_rules(dialog);
        transform.mark_drawn(12, 0);
        transform.reset();
        assert_eq!(transform.pause_rules(), rules);
        assert_eq!(transform.dialog_rules(), dialog);
        assert!(!transform.shown(12));
    }

    #[test]
    fn placements_match_the_old_offset_and_edges() {
        let layout = PauseRules::default().layout(10, 1.0);
        let place = |role| layout.place(role).unwrap();
        assert_eq!(place(QuadRole::Center).map_x(0.0), 40.0);
        assert_eq!(place(QuadRole::Center).map_x(160.0), 200.0);
        assert_eq!(place(QuadRole::Center).unmap_x(200.0), 160.0);
        assert_eq!(place(QuadRole::LeftButton).map_x(100.0), 100.0);
        assert_eq!(place(QuadRole::RightButton).map_x(0.0), 80.0);
        assert_eq!(place(QuadRole::Span).map_x(320.0), 400.0);
    }

    /// each rule hides its own group from the layout, at its own index, and
    /// showing it puts the group back.
    #[test]
    fn hidden_pause_rules_leave_the_layout_and_return() {
        let roles = [
            QuadRole::Span,
            QuadRole::Center,
            QuadRole::LeftButton,
            QuadRole::RightButton,
            QuadRole::Corner,
        ];
        for (hidden, role) in roles.iter().enumerate() {
            let mut rules = PauseRules::default();
            match hidden {
                0 => rules.span.visible = false,
                1 => rules.center.visible = false,
                2 => rules.left.visible = false,
                3 => rules.right.visible = false,
                _ => rules.corner.visible = false,
            }
            let layout = rules.layout(10, 1.0);
            for (index, other) in roles.iter().enumerate() {
                assert_eq!(layout.place(*other).is_some(), index != hidden, "{other:?}");
            }
            match hidden {
                0 => rules.span.visible = true,
                1 => rules.center.visible = true,
                2 => rules.left.visible = true,
                3 => rules.right.visible = true,
                _ => rules.corner.visible = true,
            }
            assert!(rules.layout(10, 1.0).place(*role).is_some(), "{role:?}");
        }
    }

    /// every Screen, of every family, can take the 60 cap: while it is the
    /// one shown its pictures are the title's own, and taking the cap off
    /// lets it draw in between again.
    #[test]
    fn every_screen_can_be_capped_to_sixty() {
        for spec in SCREENS {
            let mut transform = ScreenTransform::default();
            assert_eq!(transform.rate(spec.id), ScreenRate::Host, "{:?} starts on the host's rate", spec.id);
            assert!(!transform.capped(spec.id));
            transform.mark_active(spec.id, 10);
            assert!(!transform.real_frames_only(10), "{:?} draws in between", spec.id);

            transform.set_rate(spec.id, ScreenRate::Sixty);
            assert!(transform.capped(spec.id));
            assert!(transform.real_frames_only(10), "{:?} capped at 60", spec.id);

            transform.set_rate(spec.id, ScreenRate::Host);
            assert!(!transform.real_frames_only(10), "{:?} off the cap", spec.id);
        }
    }

    /// the cap is per Screen: another Screen's cap leaves the shown one
    /// drawing in between, and a cap lapses once its Screen is gone.
    #[test]
    fn a_cap_covers_only_its_screen_and_lapses_with_it() {
        let mut transform = ScreenTransform::default();
        transform.set_rate(Screen::Pause, ScreenRate::Sixty);
        transform.mark_active(Screen::MainMenu, 10);
        assert!(!transform.real_frames_only(10));

        transform.mark_active(Screen::Pause, 11);
        assert_eq!(transform.active(11), Some(Screen::Pause));
        assert!(transform.real_frames_only(11));
        assert!(transform.real_frames_only(14), "the cap holds while the Screen is shown");
        assert_eq!(transform.active(15), None);
        assert!(!transform.real_frames_only(15), "and lapses once it is gone");
    }

    /// loading a state clears what a frame recorded, but the host's caps are
    /// not the console's state.
    #[test]
    fn reset_keeps_screen_caps() {
        let mut transform = ScreenTransform::default();
        transform.set_rate(Screen::Vault, ScreenRate::Sixty);
        transform.mark_active(Screen::Vault, 12);
        transform.reset();
        assert_eq!(transform.rate(Screen::Vault), ScreenRate::Sixty);
        assert_eq!(transform.active(12), None);
        assert!(!transform.real_frames_only(12));
    }

    /// placements are seated from a completed title frame, and a picture in
    /// between reads that same layout: its t never reaches geometry.
    #[test]
    fn in_between_pictures_never_seat_a_blended_layout() {
        let transform = ScreenTransform::default();
        let real = transform.layout(12, 1.4);
        assert_eq!(real.frame(), 12);
        let roles = [QuadRole::Span, QuadRole::Center, QuadRole::LeftButton, QuadRole::RightButton, QuadRole::Corner];
        for t in [0.0, 0.25, 0.5, 0.75, 1.0] {
            let between = transform.layout_for(12, 1.4, crate::PictureKind::InBetween(t));
            assert_eq!(between.frame(), 12, "t {t} must not seat a frame of its own");
            for role in roles {
                let (real, between) = (real.place(role), between.place(role));
                assert_eq!(real.map(|place| (place.from, place.to)), between.map(|place| (place.from, place.to)), "{role:?} at t {t}");
            }
        }
    }
}
