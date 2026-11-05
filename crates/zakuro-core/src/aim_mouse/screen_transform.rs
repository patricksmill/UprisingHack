//! the one process that puts a game-owned Screen's drawing and input on the
//! top screen, and the registry of the Screens it knows.
//!
//! Pause, its dialog, the 26 Scripted Menus and the Fiend's Cauldron are
//! here; see docs/agents/screen-transform.md.

use serde::{Deserialize, Serialize};

use super::{
    outline_altar, outline_armory, outline_buttons, outline_chapter, outline_controls, outline_dialog, outline_file,
    outline_idol, outline_other, outline_practice, outline_profile, outline_queued, outline_solo, outline_subtitles,
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
    FiendsCauldron,
    Pause,
    RetreatDialog,
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
    /// one part fitted to the viewport, scaled by the menu factor.
    Generic { hint: Option<[f32; 4]>, details: bool },
    /// the game's fixed board of models.
    Group { group: &'static [(&'static str, [f32; 4])], hint: Option<[f32; 4]>, details: bool, heart: bool },
    /// Controls cut from the whole native Board, beside the details card.
    Board { group: &'static [(&'static str, [f32; 4])], hint: Option<[f32; 4]> },
    /// Idol Toss's counters, Back and launcher, placed apart.
    Idol,
}

impl LayoutKind {
    pub fn hint(self) -> Option<[f32; 4]> {
        match self {
            LayoutKind::None | LayoutKind::Chapter | LayoutKind::FileSelect => None,
            LayoutKind::Generic { hint, .. } | LayoutKind::Group { hint, .. } | LayoutKind::Board { hint, .. } => hint,
            LayoutKind::Idol => HINT_IDOL,
        }
    }

    pub fn details(self) -> bool {
        match self {
            LayoutKind::Generic { details, .. } | LayoutKind::Group { details, .. } => details,
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
pub(super) const SCRIPTED_UPDATES: [u32; 26] = [
    0x002A3EC4, 0x002A3FCC, 0x002A3F9C, 0x00224BA0, 0x001ECB4C, 0x00224C18, 0x001B4C74, 0x001ECBBC, 0x00224BD4,
    0x001ECBF4, 0x0016756C, 0x002A4070, 0x002ECCF4, 0x002A1FD8, 0x002B6DB8, 0x00238CBC, 0x001C9734, 0x00230B5C,
    0x002BB14C, 0x0019CDC8, 0x0019BDC4, 0x0023A9FC, 0x001B4D58, 0x0018AF44, 0x00224B80, 0x001B4D4C,
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
    scripted!(2, MainMenu, 0x00CFBD9C, outline_buttons, Activate::Call(0x0036F59C), LayoutKind::Group { group: MAIN, hint: HINT_BOTTOM, details: false, heart: false }),
    scripted!(3, Options, 0x00CEAE98, outline_queued, Activate::Call(0x002FB9E0), LayoutKind::Group { group: OPTIONS, hint: HINT_BOTTOM, details: false, heart: false }),
    scripted!(4, Other, 0x00CF2368, outline_other, Activate::Call(0x002D50FC), LayoutKind::Group { group: OTHER, hint: HINT_BOTTOM, details: false, heart: false }),
    scripted!(5, Controls, 0x00CF84DC, outline_controls, Activate::Call(0x003186E8), LayoutKind::Group { group: CONTROLS, hint: HINT_BOTTOM, details: false, heart: false }),
    scripted!(6, ReticleCamera, 0x00CF9B60, outline_queued, Activate::Call(0x0029B26C), LayoutKind::Group { group: RETICLE_CAMERA, hint: HINT_BOTTOM, details: false, heart: false }),
    scripted!(7, Subtitles, 0x00CFA034, outline_subtitles, Activate::Call(0x002E5448), LayoutKind::Group { group: SUBTITLES, hint: None, details: true, heart: false }),
    scripted!(8, Volume, 0x00CF2390, outline_volume, Activate::Call(0x002D56CC), LayoutKind::Group { group: VOLUME, hint: HINT_BOTTOM, details: false, heart: false }),
    scripted!(9, Profile, 0x00CF8E8C, outline_profile, Activate::Call(0x002E2F4C), LayoutKind::Group { group: PROFILE, hint: HINT_BOTTOM, details: false, heart: false }),
    scripted!(10, ProfileYesNo, 0x00CEC41C, outline_yes_no, Activate::Call(0x001C16D4), LayoutKind::Group { group: PROFILE_CHOICE, hint: HINT_BOTTOM, details: false, heart: false }),
    scripted!(11, SpotPassDialog, 0x00CF8270, outline_dialog, Activate::Call(0x00314DF8), LayoutKind::Group { group: SPOTPASS_CHOICE, hint: HINT_BOTTOM, details: false, heart: false }),
    scripted!(12, SoloHub, 0x00CFC3D0, outline_solo, Activate::Call(0x00370300), LayoutKind::Group { group: SOLO, hint: HINT_BOTTOM, details: false, heart: true }),
    scripted!(13, ArmsAltar, 0x00CFCA28, outline_altar, Activate::Call(0x00322C34), LayoutKind::Group { group: ALTAR, hint: HINT_BOTTOM, details: false, heart: false }),
    scripted!(14, ArmoryGear, 0x00CEC2B8, outline_armory, Activate::None, LayoutKind::Board { group: GEAR, hint: HINT_TOP }),
    scripted!(15, ArmoryRedeemHearts, 0x00CEFCA8, outline_armory, Activate::None, LayoutKind::Generic { hint: None, details: true }),
    scripted!(16, ArmoryConvertToHearts, 0x00CEEBFC, outline_armory, Activate::None, LayoutKind::Generic { hint: None, details: true }),
    scripted!(17, ArmoryFuseWeapons, 0x00CEE148, outline_armory, Activate::None, LayoutKind::Generic { hint: HINT_BOTTOM, details: true }),
    scripted!(18, ArmoryPowers, 0x00CEC2E0, outline_armory, Activate::None, LayoutKind::Board { group: POWERS, hint: None }),
    scripted!(19, ArmoryWeapons, 0x00CEE1E8, outline_armory, Activate::None, LayoutKind::Board { group: WEAPON_LIST, hint: HINT_BOTTOM }),
    scripted!(20, ArmoryCategories, 0x00CEE1C0, outline_armory, Activate::None, LayoutKind::Board { group: CATEGORIES, hint: HINT_TOP }),
    scripted!(21, PracticeRange, 0x00CF0154, outline_practice, Activate::Call(0x0030B5B4), LayoutKind::Group { group: PRACTICE, hint: HINT_PRACTICE, details: false, heart: false }),
    scripted!(22, IdolTossSolo, 0x00CF9400, outline_idol, Activate::Touch, LayoutKind::Idol),
    scripted!(23, TreasureHunt, 0x00CEEDD8, outline_treasure, Activate::Touch, LayoutKind::Generic { hint: HINT_TREASURE, details: false }),
    scripted!(24, Vault, 0x00CF8248, outline_vault, Activate::Call(0x00345FDC), LayoutKind::Group { group: VAULT, hint: HINT_VAULT, details: false, heart: false }),
    scripted!(25, IdolTossVault, 0x00CF9400, outline_idol, Activate::Touch, LayoutKind::Idol),
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
];

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
}

/// the kinds of quad the pause screen flushes, each with its own placement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum QuadRole {
    Span = 0,
    Center = 1,
    LeftButton = 2,
    RightButton = 3,
    Corner = 4,
}

impl From<QuadRole> for usize {
    fn from(role: QuadRole) -> usize {
        role as usize
    }
}

/// the most roles one Screen's layout seats: the Cauldron's eight.
const ROLES: usize = 8;

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

/// the pause screen's editable rules, one per quad role.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PauseRules {
    pub span: PlacementRule,
    pub center: PlacementRule,
    pub left: PlacementRule,
    pub right: PlacementRule,
    pub corner: PlacementRule,
}

impl Default for PauseRules {
    fn default() -> PauseRules {
        let from = [0.0, 0.0, 320.0, 240.0];
        let rule = |to, anchor| PlacementRule { from, to, anchor, anchor_y: 0.0, scale: RuleScale::Narrow, visible: true };
        PauseRules {
            span: PlacementRule { scale: RuleScale::Stretch, ..rule([0.0, 0.0, 400.0, 240.0], 0.0) },
            center: rule([PAUSE_MENU_OFFSET, 0.0, 320.0, 240.0], 0.5),
            left: rule([0.0, 0.0, 320.0, 240.0], 0.0),
            right: rule([2.0 * PAUSE_MENU_OFFSET, 0.0, 320.0, 240.0], 1.0),
            corner: rule([2.0 * PAUSE_MENU_OFFSET, 0.0, 320.0, 240.0], 1.0),
        }
    }
}

impl PauseRules {
    /// the Rust row a layout change would land as.
    pub fn rust_fragment(&self) -> String {
        format!(
            "PauseRules {{\n    span: {},\n    center: {},\n    left: {},\n    right: {},\n    corner: {},\n}}\n",
            rule_rust(self.span), rule_rust(self.center), rule_rust(self.left), rule_rust(self.right), rule_rust(self.corner)
        )
    }

    /// what changes from the default Pause rules, one line per rule and one
    /// more for a rule hidden or shown.
    pub fn change_request(&self) -> String {
        let default = PauseRules::default();
        change_request(
            "Pause",
            &[
                ("span", default.span, self.span),
                ("center", default.center, self.center),
                ("left", default.left, self.left),
                ("right", default.right, self.right),
                ("corner", default.corner, self.corner),
            ],
            "Acceptance: on a Pause capture the five rules resolve to the exported rectangles, and hit-testing lights the Control inside each.",
        )
    }

    /// seats the rules for a completed title frame. `frame` is the title's
    /// own: in-between pictures never seat a second, blended layout.
    pub(super) fn layout(self, frame: u64, widen: f32) -> ScreenLayout {
        let place = |rule: PlacementRule| rule.visible.then(|| rule.placement(widen));
        // in QuadRole order
        ScreenLayout {
            frame,
            placements: [
                place(self.span),
                place(self.center),
                place(self.left),
                place(self.right),
                place(self.corner),
                None,
                None,
                None,
            ],
        }
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
/// Move Out, the rest (the pot, the pan and the hearts bet), and the
/// bottom screen's ring background, drawn behind everything else.
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
}

impl CauldronRole {
    pub const ALL: [CauldronRole; 8] = [
        CauldronRole::Top,
        CauldronRole::Panel,
        CauldronRole::Bar,
        CauldronRole::Dial,
        CauldronRole::Back,
        CauldronRole::MoveOut,
        CauldronRole::Rest,
        CauldronRole::Ring,
    ];

    pub fn name(self) -> &'static str {
        ["top", "panel", "bar", "dial", "back", "moveout", "rest", "ring"][self as usize]
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
pub struct CauldronRules([PlacementRule; 8]);

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
    /// behind it, made for 16:9 and smaller on a narrower screen.
    fn default() -> CauldronRules {
        let fit = |to, from, right: bool, down: bool| PlacementRule {
            from,
            to,
            anchor: if right { 1.0 } else { 0.0 },
            anchor_y: if down { 1.0 } else { 0.0 },
            scale: RuleScale::Fit { design_width: CAULDRON_WIDTH },
            visible: true,
        };
        let (bar_to, bar_from) = ([40.0, 197.0, 220.0, 23.0], [14.0, 0.0, 312.0, 32.0]);
        let (along, down) = (bar_to[2] / bar_from[2], bar_to[3] / bar_from[3]);
        // at 0, the dial's arrow tip on the bar's lower edge, the panel's
        // middle over the dial
        let dial_middle = bar_to[0] + (DIAL_ZERO - bar_from[0]) * along;
        let dial = [DIAL_SIZE[0] * along * DIAL_SCALE, DIAL_SIZE[1] * down * DIAL_SCALE];
        let panel_width = 91.0;
        CauldronRules([
            fit([16.0, 24.0, 258.0, 172.0], [36.0, 22.0, 328.0, 218.0], false, false),
            fit([dial_middle - panel_width * 0.5, 148.0, panel_width, 49.0], [PANEL_ZERO - 70.0, 176.0, 140.0, 76.0], false, true),
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

    /// how far the bar's placement goes across the top screen for each
    /// pixel of the bottom screen's.
    fn along(&self, widen: f32) -> f32 {
        let bar = self.rule(CauldronRole::Bar).placement(widen);
        bar.to[2] / bar.from[2]
    }

    /// every role's placement at the pot's value, hidden ones too: the
    /// panel and the dial slid along the bar with it.
    pub fn boxes(&self, widen: f32, value: f32) -> [Placement; ROLES] {
        let mut boxes = CauldronRole::ALL.map(|role| self.rule(role).placement(widen));
        let along = self.along(widen);
        for (role, from_step) in [(CauldronRole::Panel, PANEL_STEP), (CauldronRole::Dial, DIAL_STEP)] {
            let place = &mut boxes[role as usize];
            place.from[0] += value * from_step;
            place.to[0] += value * DIAL_STEP * along;
        }
        boxes
    }

    /// moves a role so that, at the pot's value, it is placed at `to` in the
    /// top screen's 400 by 240.
    pub fn set_box(&mut self, role: CauldronRole, widen: f32, value: f32, mut to: [f32; 4]) {
        if matches!(role, CauldronRole::Panel | CauldronRole::Dial) {
            to[0] -= value * DIAL_STEP * self.along(widen);
        }
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
        let (pause_rules, dialog_rules, cauldron_rules, rates) = (self.pause_rules, self.dialog_rules, self.cauldron_rules, self.rates);
        *self = Self::default();
        self.pause_rules = pause_rules;
        self.dialog_rules = dialog_rules;
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
        assert_eq!(SCRIPTED_COUNT, 26);
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
        assert!(near(place(QuadRole::RightButton).to[0], 400.0 - 320.0 / 1.4));
        assert!(near(place(QuadRole::LeftButton).to[0], 0.0));
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
        assert!(request.contains("Move the center rule on Pause"));
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
