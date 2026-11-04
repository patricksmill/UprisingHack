//! the one process that puts a game-owned Screen's drawing and input on the
//! top screen, and the registry of the Screens it knows.
//!
//! Pause, its dialog and the 26 Scripted Menus are here. The Cauldron joins
//! them, then the layout stages move from index branching to placements; see
//! docs/agents/screen-transform.md.

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
#[derive(Clone, Copy, Debug)]
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

/// one Screen's placements for a completed title frame, in `QuadRole` order.
/// A rule that was hidden has no placement, so neither drawing nor
/// hit-testing reaches it. The frame is the title's own: a picture in
/// between two titles' frames has no number of its own, so it reuses the
/// newer real frame's placements rather than blending them.
pub(super) struct ScreenLayout {
    frame: u64,
    placements: [Option<Placement>; 5],
}

impl ScreenLayout {
    pub fn place(&self, role: QuadRole) -> Option<Placement> {
        self.placements[role as usize]
    }

    /// the completed title frame these placements were seated from.
    pub fn frame(&self) -> u64 {
        self.frame
    }
}

/// how one target rect follows the viewport: fixed size, narrowed by
/// `widen` about its anchor, or stretched across the top screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuleScale {
    #[allow(dead_code)]
    Fixed,
    Narrow,
    Stretch,
}

/// a rule is shown unless it was hidden, which layouts saved before rules
/// could be hidden have no word for.
fn shown() -> bool {
    true
}

/// one editable placement rule: the source box, the target box at 400 by
/// 240, the anchor it narrows about, how it follows the viewport, and
/// whether it is shown.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlacementRule {
    pub from: [f32; 4],
    pub to: [f32; 4],
    pub anchor: f32,
    pub scale: RuleScale,
    /// a hidden rule's group is drawn nowhere and takes no touches.
    #[serde(default = "shown")]
    pub visible: bool,
}

impl PlacementRule {
    pub fn placement(self, widen: f32) -> Placement {
        let widen = if widen.is_finite() && widen > 0.0 { widen } else { 1.0 };
        let (x, width) = match self.scale {
            RuleScale::Fixed => (self.to[0], self.to[2]),
            RuleScale::Narrow => {
                let width = self.to[2] / widen;
                (self.to[0] + self.anchor * (self.to[2] - width), width)
            }
            RuleScale::Stretch => (0.0, 400.0),
        };
        Placement { from: self.from, to: [x, self.to[1], width, self.to[3]] }
    }
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
        PauseRules {
            span: PlacementRule { from, to: [0.0, 0.0, 400.0, 240.0], anchor: 0.0, scale: RuleScale::Stretch, visible: true },
            center: PlacementRule { from, to: [PAUSE_MENU_OFFSET, 0.0, 320.0, 240.0], anchor: 0.5, scale: RuleScale::Narrow, visible: true },
            left: PlacementRule { from, to: [0.0, 0.0, 320.0, 240.0], anchor: 0.0, scale: RuleScale::Narrow, visible: true },
            right: PlacementRule { from, to: [2.0 * PAUSE_MENU_OFFSET, 0.0, 320.0, 240.0], anchor: 1.0, scale: RuleScale::Narrow, visible: true },
            corner: PlacementRule { from, to: [2.0 * PAUSE_MENU_OFFSET, 0.0, 320.0, 240.0], anchor: 1.0, scale: RuleScale::Narrow, visible: true },
        }
    }
}

impl PauseRules {
    /// the Rust row a layout change would land as.
    pub fn rust_fragment(&self) -> String {
        let rule = |rule: PlacementRule| {
            format!("PlacementRule {{ from: {:?}, to: {:?}, anchor: {:?}, scale: RuleScale::{:?}, visible: {:?} }}",
                rule.from, rule.to, rule.anchor, rule.scale, rule.visible)
        };
        format!(
            "PauseRules {{\n    span: {},\n    center: {},\n    left: {},\n    right: {},\n    corner: {},\n}}\n",
            rule(self.span), rule(self.center), rule(self.left), rule(self.right), rule(self.corner)
        )
    }

    /// what changes from the default Pause rules, one line per rule and one
    /// more for a rule hidden or shown.
    pub fn change_request(&self) -> String {
        let default = PauseRules::default();
        let rules = [
            ("span", default.span, self.span),
            ("center", default.center, self.center),
            ("left", default.left, self.left),
            ("right", default.right, self.right),
            ("corner", default.corner, self.corner),
        ];
        let mut lines = Vec::new();
        let mut hid = false;
        for (name, before, after) in rules {
            if before.visible != after.visible {
                hid = true;
                lines.push(format!(
                    "{} the {name} rule on Pause.",
                    if after.visible { "Show" } else { "Hide" }
                ));
            }
            if before.from == after.from && before.to == after.to && before.anchor == after.anchor
                && before.scale == after.scale
            {
                continue;
            }
            lines.push(format!(
                "Move the {name} rule on Pause: from {:?} to {:?}, anchor {:?} -> {:?}, {:?} -> {:?}.",
                before.from, after.from, before.anchor, after.anchor, before.scale, after.scale
            ));
        }
        if lines.is_empty() {
            return "No changes from the default Pause rules.\n".to_owned();
        }
        lines.push("Acceptance: on a Pause capture the five rules resolve to the exported rectangles, and hit-testing lights the Control inside each.".to_owned());
        if hid {
            lines.push("Hidden rules draw nothing on the top screen and take no touches.".to_owned());
        }
        lines.join("\n") + "\n"
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
            ],
        }
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
    /// the pause screen's editable placement rules.
    pause_rules: PauseRules,
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
            pause_rules: PauseRules::default(),
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

    /// seats the Pause placements for a completed title frame. Drawing and
    /// hit-testing both call this with the same real frame; a picture in
    /// between has no frame of its own and reads the newest real one, never
    /// a blended placement.
    pub fn layout(&self, frame: u64, widen: f32) -> ScreenLayout {
        self.pause_rules.layout(frame, widen)
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

    /// whether the picture at `frame` must be the title's own: the Screen
    /// shown then is capped at 60. The host's presenter asks this before it
    /// shows a picture in between.
    pub fn real_frames_only(&self, frame: u64) -> bool {
        self.active(frame).is_some_and(|screen| self.capped(screen))
    }

    /// Clears what the last frames recorded. The host's editable Pause rules
    /// and its per-Screen caps are not the console's state, so a load keeps
    /// them.
    pub fn reset(&mut self) {
        let pause_rules = self.pause_rules;
        let rates = self.rates;
        *self = Self::default();
        self.pause_rules = pause_rules;
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

    pub fn mark_cauldron(&mut self, frame: u64, task: u32, pouring: bool) {
        self.cauldron_drawn = Some(frame);
        self.cauldron_task = Some(task);
        self.cauldron_pouring = pouring;
        self.mark_active(Screen::FiendsCauldron, frame);
    }

    pub fn cauldron_shown(&self, frame: u64) -> bool {
        self.cauldron_drawn.is_some_and(|drawn| frame <= drawn + 3)
    }

    pub fn cauldron_pouring(&self, frame: u64) -> bool {
        self.cauldron_shown(frame) && self.cauldron_pouring
    }

    pub fn cauldron_task(&self) -> Option<u32> {
        self.cauldron_task
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
    /// editable Pause rules are not part of the console's state.
    #[test]
    fn reset_keeps_editor_rules() {
        let mut rules = PauseRules::default();
        rules.center.to[0] = 60.0;
        rules.left.scale = RuleScale::Fixed;
        let mut transform = ScreenTransform::default();
        transform.set_pause_rules(rules);
        transform.mark_drawn(12, 0);
        transform.reset();
        assert_eq!(transform.pause_rules(), rules);
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
