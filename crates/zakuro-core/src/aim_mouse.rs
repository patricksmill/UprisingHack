//! mouse aiming and a smaller HUD for Kid Icarus: Uprising (USA), a test of
//! what a port's overrides will do: the mouse's movement goes straight into
//! the aim delta the game's touch handling produces, with none of its
//! smoothing or flick, and the top screen's HUD and reticle are drawn
//! smaller. only the interpreter stops where this looks, so the functions
//! these places are in run in it, see FUNCTIONS.

use crate::System;

mod menu_top;
use menu_top::MenuTop;

/// Kid Icarus: Uprising, the USA cartridge.
pub const PROGRAM_ID: u64 = 0x0004_0000_0003_0100;

/// the places below, which the interpreter has to run for it to stop at
/// them: every function whose code takes one in runs in it, the rest of the
/// game stays recompiled. one place a function does, the pause menu's top
/// places are in the top pass TOP_UI_AFTER_HUD is in, and the dialogs'
/// in one function, their bottom sites in another, and the paused input's
/// in one.
pub const PLACES: [u32; 77] = [
    0x0025EF84,
    0x00A390F0, 0x00A3872C, 0x00A38514,
    0x002A3374, 0x002A3470, 0x002A34C8, 0x002A3378, 0x002A3474, 0x002A34CC,
    AFTER_AIM_DELTA,
    BEFORE_CAMERA_TURN,
    SPRITE_FLUSH,
    PROJECTION,
    PASS_BEGIN,
    RETICLE_DRAW,
    TOP_UI_AFTER_HUD,
    UI_COMMANDS,
    PAUSE_MENU[0].site,
    PAUSE_MENU[1].site,
    PAUSE_MENU[1].place,
    MIRRORED[0].site,
    MIRRORED[1].site,
    MIRRORED[2].site,
    MIRRORED[3].site,
    MIRRORED[4].site,
    MIRRORED[5].site,
    MIRRORED[6].site,
    MIRRORED[7].site,
    BOTTOM_BATTLE_UI,
    PAUSE_KEY,
    PAUSE_OPTION_FOUND,
    OPTIONS_OPTION_FOUND,
    DIALOG_INPUT,
    CAULDRON[0],
    CAULDRON[1],
    CAULDRON_CURSOR_KEYS_DONE,
    TOP_SCENE_3D,
    TOP_TEXT,
    TOP_TEXT_DRAWN,
    TOP_SCENE_DRAWN,
    BOTTOM_SCENE_DRAW,
    BOTTOM_SCENE_3D_DRAWN,
    menu_top::SCISSOR,
    menu_top::SCISSOR_DONE,
    menu_top::BOTTOM_TEXT,
    menu_top::BOTTOM_TRANSLUCENT,
    menu_top::FONT_CALL[0],
    menu_top::START,
    menu_top::FINISHED,
    MENU_NO_TOUCH,
    MENU_SCREENS[0].update,
    MENU_SCREENS[1].update,
    MENU_SCREENS[2].update,
    MENU_SCREENS[3].update,
    MENU_SCREENS[4].update,
    MENU_SCREENS[5].update,
    MENU_SCREENS[6].update,
    MENU_SCREENS[7].update,
    MENU_SCREENS[8].update,
    MENU_SCREENS[9].update,
    MENU_SCREENS[10].update,
    MENU_SCREENS[11].update,
    MENU_SCREENS[12].update,
    MENU_SCREENS[13].update,
    MENU_SCREENS[14].update,
    MENU_SCREENS[15].update,
    MENU_SCREENS[16].update,
    MENU_SCREENS[17].update,
    MENU_SCREENS[18].update,
    MENU_SCREENS[19].update,
    MENU_SCREENS[20].update,
    MENU_SCREENS[21].update,
    MENU_SCREENS[22].update,
    MENU_SCREENS[23].update,
    MENU_SCREENS[24].update,
    MENU_SCREENS[25].update,
];

/// the Fiend's Cauldron's pot (a task, vtable 0x00CF9AEC), the update it
/// runs each frame while the bar is shown, then while it pours after Move
/// Out. it runs on that screen alone, not on the chapter select before it,
/// the next screen or in a battle.
const CAULDRON: [u32; 2] = [0x00251624, 0x00251C40];

/// the Fiend's Cauldron's cursor, the dial, Move Out or Back lit for the
/// keys to use, a task (vtable 0x00CEE170). its update, 0x0027B100, goes
/// on at 0x0027BBA0 once it has handled the keys, the cursor in r4. what
/// is lit is named at +0xD8, a hash then a string. an arrow key lights
/// another by 0x003062F0(cursor, name), then 0x00305D44(cursor, 1), which
/// moves the light and plays the cursor's sound, then setting bit 0x10 of
/// +0x191: a name is a hash, then a string, a pointer to its characters
/// and room for 0x20 of them.
const CAULDRON_CURSOR_KEYS_DONE: u32 = 0x0027BBA0;
const CAULDRON_CURSOR: u32 = 0x00CEE170;
const CAULDRON_CURSOR_LIT: u32 = 0xD8;
const SET_CAULDRON_CURSOR: u32 = 0x003062F0;
const CAULDRON_CURSOR_MOVED: u32 = 0x00305D44;
const CAULDRON_CURSOR_FLAGS: u32 = 0x191;
/// the names, and their hashes, of the dial, Move Out and Back.
const CAULDRON_CURSOR_NAMES: [(&str, u32); 3] = [("Level_Bar", 0x53D3_F0F1), ("GO", 0xCD94_A288), ("return", 0x928B_EAA4)];

/// the menus outside battle take touches through one class (vtable
/// 0x00CFC340): 0x0071C420 reads the touch each frame and, with none, calls
/// its slot 0x00A391E4, it in r0. 0x00A38324(out, it, &xy) finds what is
/// under a pixel, frontmost: out gets the layer, then the element's index,
/// -1 for none. a layer's part is at +0x38, and 0x00326D74(part, &PART_KEY)
/// +0x1C is the part's descriptor, its name at +0x18, as CAULDRON_CURSOR's.
const MENU_NO_TOUCH: u32 = 0x00A391E4;
const MENU_TOUCH: u32 = 0x00CFC340;
const MENU_HIT: u32 = 0x00A38324;
const LAYER_PART: u32 = 0x38;
const PART_LOOKUP: u32 = 0x00326D74;
const PART_KEY: u32 = 0x00D71744;
const PART_DESCRIPTOR: u32 = 0x1C;
const DESCRIPTOR_NAME: u32 = 0x18;

/// a menu outside battle that the mouse moves the outline of, as its keys
/// do, by its own code: its object's vtable, the place the task loop runs
/// its update from with it in r0, the calls of its own functions that
/// outline what the mouse is over (see Found), none when it is not one of
/// its parts, and the function A uses what is outlined with (object).
struct MenuScreen {
    vtable: u32,
    update: u32,
    outline: fn(&mut System, u32, Found) -> Option<Vec<MenuCall>>,
    use_outlined: u32,
}

/// a call of the game's: the function, r0 to r2.
type MenuCall = (u32, [u32; 3]);

/// what the mouse is over in a menu: its part's hash and where its name is
/// (a hash, then a string), and the element.
#[derive(Clone, Copy)]
struct Found {
    hash: u32,
    name: u32,
    element: u32,
    part: u32,
}

/// A menu press waits for release to click, or becomes a native touch
/// when held for 200 ms or moved four bottom-screen pixels.
struct MenuPress {
    pointer: (f32, f32),
    object: u32,
    since: u64,
    dragging: Option<u64>,
}

/// a screen's click that is its own touch, for a screen whose parts act on
/// touches and not on A (Idol Toss's bowl, its Back): the hover still
/// outlines, and the press over a part touches it there, as unmapped
/// controls do.
const CLICK_TOUCHES: u32 = u32::MAX;

const MENU_SCREENS: [MenuScreen; 26] = [
    MenuScreen { vtable: 0x00CEC3F4, update: 0x002A3EC4, outline: outline_file, use_outlined: 0x003374D0 },
    MenuScreen { vtable: 0x00CF243C, update: 0x002A3FCC, outline: outline_chapter, use_outlined: 0x003675C0 },
    MenuScreen { vtable: 0x00CFBD9C, update: 0x002A3F9C, outline: outline_buttons, use_outlined: 0x0036F59C },
    MenuScreen { vtable: 0x00CEAE98, update: 0x00224BA0, outline: outline_queued, use_outlined: 0x002FB9E0 },
    MenuScreen { vtable: 0x00CF2368, update: 0x001ECB4C, outline: outline_other, use_outlined: 0x002D50FC },
    MenuScreen { vtable: 0x00CF84DC, update: 0x00224C18, outline: outline_controls, use_outlined: 0x003186E8 },
    MenuScreen { vtable: 0x00CF9B60, update: 0x001B4C74, outline: outline_queued, use_outlined: 0x0029B26C },
    MenuScreen { vtable: 0x00CFA034, update: 0x001ECBBC, outline: outline_subtitles, use_outlined: 0x002E5448 },
    MenuScreen { vtable: 0x00CF2390, update: 0x00224BD4, outline: outline_volume, use_outlined: 0x002D56CC },
    MenuScreen { vtable: 0x00CF8E8C, update: 0x001ECBF4, outline: outline_profile, use_outlined: 0x002E2F4C },
    MenuScreen { vtable: 0x00CEC41C, update: 0x0016756C, outline: outline_yes_no, use_outlined: 0x001C16D4 },
    MenuScreen { vtable: 0x00CF8270, update: 0x002A4070, outline: outline_dialog, use_outlined: 0x00314DF8 },
    MenuScreen { vtable: 0x00CFC3D0, update: 0x002ECCF4, outline: outline_solo, use_outlined: 0x00370300 },
    MenuScreen { vtable: 0x00CFCA28, update: 0x002A1FD8, outline: outline_altar, use_outlined: 0x00322C34 },
    MenuScreen { vtable: 0x00CEC2B8, update: 0x002B6DB8, outline: outline_armory, use_outlined: 0 },
    MenuScreen { vtable: 0x00CEFCA8, update: 0x00238CBC, outline: outline_armory, use_outlined: 0 },
    MenuScreen { vtable: 0x00CEEBFC, update: 0x001C9734, outline: outline_armory, use_outlined: 0 },
    MenuScreen { vtable: 0x00CEE148, update: 0x00230B5C, outline: outline_armory, use_outlined: 0 },
    MenuScreen { vtable: 0x00CEC2E0, update: 0x002BB14C, outline: outline_armory, use_outlined: 0 },
    MenuScreen { vtable: 0x00CEE1E8, update: 0x0019CDC8, outline: outline_armory, use_outlined: 0 },
    MenuScreen { vtable: 0x00CEE1C0, update: 0x0019BDC4, outline: outline_armory, use_outlined: 0 },
    MenuScreen { vtable: 0x00CF0154, update: 0x0023A9FC, outline: outline_practice, use_outlined: 0x0030B5B4 },
    MenuScreen { vtable: 0x00CF9400, update: 0x001B4D58, outline: outline_idol, use_outlined: CLICK_TOUCHES },
    MenuScreen { vtable: 0x00CEEDD8, update: 0x0018AF44, outline: outline_treasure, use_outlined: CLICK_TOUCHES },
    MenuScreen { vtable: 0x00CF8248, update: 0x00224B80, outline: outline_vault, use_outlined: 0x00345FDC },
    MenuScreen { vtable: 0x00CF9400, update: 0x001B4D4C, outline: outline_idol, use_outlined: CLICK_TOUCHES },
];


/// The Vault's Idols, Music Gallery, Power Portrait, Offering, Idol Toss,
/// Records, AR and Back: a hash at +0x44 and its name after it.
/// 0x00345C18(object, part, 1) outlines the part where a pointer finds
/// it, as the screen's own pointer handler does (0 for a key), and A
/// 0x00345FDC(object) presses whichever part that hash is.
const OUTLINE_VAULT: u32 = 0x00345C18;

fn outline_vault(system: &mut System, object: u32, found: Found) -> Option<Vec<MenuCall>> {
    if read_word(system, object + BUTTONS_OUTLINED) == found.hash {
        return Some(Vec::new());
    }
    Some(vec![(OUTLINE_VAULT, [object, found.name, 1])])
}

/// Treasure Hunt's board and Back: the one picked a hash at +0x34 and its
/// name after it. 0x0020207C(object, part) sets both from the part, as a
/// touch on it does (its handler, 0x001CBFCC), and Back goes back.
const TREASURE_PICKED: u32 = 0x34;
const PICK_TREASURE_PART: u32 = 0x0020_207C;

fn outline_treasure(system: &mut System, object: u32, found: Found) -> Option<Vec<MenuCall>> {
    if read_word(system, object + TREASURE_PICKED) == found.hash {
        return Some(Vec::new());
    }
    Some(vec![(PICK_TREASURE_PART, [object, found.part, 0])])
}

/// Idol Toss's bowl and Back: the outlined one an index at +0x3B8 into its
/// parts, at +0x44, +0x254 and +0x1D0 of the scene its task keeps (kind
/// 0x00D71458), its hash at +0x48 and its name after it. its keys step
/// the index and copy both from there (0x0024E9F8); so does a touch, with
/// 0x0022CF38(object's +0x38, +0x48), which finds the part under it.
const IDOL_INDEX: u32 = 0x3B8;
const IDOL_OUTLINED: u32 = 0x48;
const IDOL_NAME: u32 = 0x4C;
const IDOL_PARTS: [u32; 3] = [0x44, 0x254, 0x1D0];
const IDOL_KIND: u32 = 0x00D7_1458;

fn outline_idol(system: &mut System, object: u32, found: Found) -> Option<Vec<MenuCall>> {
    if read_word(system, object + IDOL_OUTLINED) == found.hash {
        return Some(Vec::new());
    }
    let task = read_word(system, object + 0x30);
    let scene = kept_of_kind(system, task, IDOL_KIND);
    if scene == 0 {
        return None;
    }
    let index = (0..IDOL_PARTS.len()).find(|&part| read_word(system, scene + IDOL_PARTS[part]) == found.hash)?;
    system.memory.write_bytes(object + IDOL_INDEX, &(index as u32).to_le_bytes());
    system.memory.write_bytes(object + IDOL_OUTLINED, &found.hash.to_le_bytes());
    Some(vec![(COPY_NAME, [object + IDOL_NAME, found.name + 4, 0])])
}

/// Practice Range's Air Battle, Land Battle and Gear: the outlined one a
/// hash at +0x2E4, its name a string after it; a key writes the next name
/// into a queue at +0x33C and sets +0x374's bit 0, and the update's own
/// step, 0x00283DE0(object), copies it in with 0x002EA7A0(+0x2E4, +0x33C),
/// plays the cursor's sound and marks the part's animation. the same here,
/// for the part under the mouse: queue it, flag it, then apply it. A,
/// 0x0030B5B4(object), acts on the hash at +0x2E4: Air Battle and Land
/// Battle start with 0x00340ED4(object, 0 or 1), Gear opens the armory
/// with 0x0030B68C(object).
const PRACTICE_OUTLINED: u32 = 0x2E4;
const PRACTICE_QUEUE: u32 = 0x33C;
const PRACTICE_FLAGS: u32 = 0x374;
const OUTLINE_PRACTICE_PART: u32 = 0x002EA7A0;
const APPLY_PRACTICE_QUEUE: u32 = 0x00283DE0;

fn outline_practice(system: &mut System, object: u32, found: Found) -> Option<Vec<MenuCall>> {
    if read_word(system, object + PRACTICE_OUTLINED) == found.hash {
        return Some(Vec::new());
    }
    let mut flags = [0u8];
    system.memory.read_bytes(object + PRACTICE_FLAGS, &mut flags);
    system.memory.write_bytes(object + PRACTICE_FLAGS, &[flags[0] | 1]);
    Some(vec![
        (OUTLINE_PRACTICE_PART, [object + PRACTICE_QUEUE, found.name, 0]),
        (APPLY_PRACTICE_QUEUE, [object, 0, 0]),
    ])
}

/// file select's outlined part at +0x34, a hash; its parts, the three
/// books (book_01 to _03), Back (return) and How to Play (guide). its keys
/// go along its navigation table and outline the next part by 0x00302488
/// (object, &name), which also turns its book, shows its file's panel and
/// plays the cursor's sound; A (0x003374D0) opens the book, goes back or
/// shows the guide.
const FILE_OUTLINED: u32 = 0x34;
const FILE_PARTS: [u32; 5] = [0xA70D_5C65, 0xA70D_5C64, 0xA70D_5C67, 0x928B_EAA4, 0x8D79_D12E];
const OUTLINE_FILE_PART: u32 = 0x00302488;

fn outline_file(system: &mut System, object: u32, found: Found) -> Option<Vec<MenuCall>> {
    if !FILE_PARTS.contains(&found.hash) {
        return None;
    }
    let outlined = read_word(system, object + FILE_OUTLINED) == found.hash;
    Some(if outlined { Vec::new() } else { vec![(OUTLINE_FILE_PART, [object, found.name, 0])] })
}

/// chapter select's outlined part at +0x6C, a hash: the chapters' list
/// (BtThumbnail) or Back (Return). 0x00367394(object) outlines the list,
/// 0x003668E8(object) Back, as up and down do; the chapter, +0x12C from 0,
/// is picked by 0x00343FF8(object, chapter, 1), as left and right do,
/// which scrolls the list to it. each chapter is an element of the list,
/// named ML_B0001 to ML_B0026 by its number (two of them twice, _0 and
/// _1), those not open yet not found under the mouse. A, 0x003675C0, goes
/// to the chapter's Fiend's Cauldron or back.
const CHAPTER_OUTLINED: u32 = 0x6C;
const CHAPTER: u32 = 0x12C;
const CHAPTER_LIST: u32 = 0x4D6A_3DE7;
const CHAPTER_BACK: u32 = 0x9A07_E025;
const OUTLINE_CHAPTER_LIST: u32 = 0x00367394;
const OUTLINE_CHAPTER_BACK: u32 = 0x003668E8;
const PICK_CHAPTER: u32 = 0x00343FF8;

fn outline_chapter(system: &mut System, object: u32, found: Found) -> Option<Vec<MenuCall>> {
    let outlined = read_word(system, object + CHAPTER_OUTLINED);
    match found.hash {
        CHAPTER_BACK => Some(if outlined == CHAPTER_BACK { Vec::new() } else { vec![(OUTLINE_CHAPTER_BACK, [object, 0, 0])] }),
        CHAPTER_LIST => {
            let mut name = [0u8; 8];
            let at = read_word(system, found.element);
            system.memory.read_bytes(at, &mut name);
            let number: u32 = std::str::from_utf8(name.strip_prefix(b"ML_B")?).ok()?.parse().ok()?;
            let chapter = number.checked_sub(1)?;
            let mut calls = Vec::new();
            if outlined != CHAPTER_LIST {
                calls.push((OUTLINE_CHAPTER_LIST, [object, 0, 0]));
            }
            if read_word(system, object + CHAPTER) != chapter {
                calls.push((PICK_CHAPTER, [object, chapter, 1]));
            }
            Some(calls)
        }
        _ => None,
    }
}

/// the hub's button menus (the main menu's Solo, StreetPass, Vault,
/// Together, Options, AR, the guide and Back): the outlined part at +0x44,
/// a hash, and its name at +0x48 (a string). their keys step along a
/// table by 0x0034E1B8(object, direction), which writes both, copies them
/// to +0x70 and +0x74, refreshes the outline with 0x0036FB58(object, 0),
/// reading only +0x44 and +0x48, and plays the cursor's sound with
/// 0x002B435C(object's +0x30, 7). the same here, for the part under the
/// mouse; A is 0x0036F59C(object).
const BUTTONS_OUTLINED: u32 = 0x44;
const BUTTONS_LAST: u32 = 0x70;
const BUTTONS_SOUND_OWNER: u32 = 0x30;
const COPY_NAME: u32 = 0x00352A9C;
const REFRESH_BUTTONS: u32 = 0x0036FB58;
const BUTTONS_SOUND: u32 = 0x002B435C;

fn outline_buttons(system: &mut System, object: u32, found: Found) -> Option<Vec<MenuCall>> {
    if read_word(system, object + BUTTONS_OUTLINED) == found.hash {
        return Some(Vec::new());
    }
    system.memory.write_bytes(object + BUTTONS_OUTLINED, &found.hash.to_le_bytes());
    system.memory.write_bytes(object + BUTTONS_LAST, &found.hash.to_le_bytes());
    let owner = read_word(system, object + BUTTONS_SOUND_OWNER);
    Some(vec![
        (COPY_NAME, [object + BUTTONS_OUTLINED + 4, found.name + 4, 0]),
        (COPY_NAME, [object + BUTTONS_LAST + 4, found.name + 4, 0]),
        (REFRESH_BUTTONS, [object, 0, 0]),
        (BUTTONS_SOUND, [owner, 7, 0]),
    ])
}

/// Options' buttons (Controls, Volume, Other and Back), and Reticle/
/// Camera's (Air Battle, Land Battle and Back; its own copies, 0x00251D74
/// outlining, A 0x0029B26C): the outlined part
/// at +0x44 as the main menu's, but a key queues the next one: first
/// 0x002BFE50(object + 0x34, &{hash, name}), which updates the parts'
/// states for it, then its hash
/// and name copied to +0x9C by 0x002EA7A0(at, &{hash, name}) and +0xD4's
/// bit 0 set (bit 1 for a move back), and 0x002B4428(object), early in
/// the update, outlines it, posts it to the buttons' animations and plays
/// the cursor's sound. A is 0x002FB9E0(object).
const QUEUED_PART: u32 = 0x9C;
const QUEUED: u32 = 0xD4;
const QUEUE_PART: u32 = 0x002EA7A0;
const LEAVE_PART: u32 = 0x002BFE50;
const LEAVE_FROM: u32 = 0x34;

fn outline_queued(system: &mut System, object: u32, found: Found) -> Option<Vec<MenuCall>> {
    if read_word(system, object + BUTTONS_OUTLINED) == found.hash {
        return Some(Vec::new());
    }
    let mut queued = [0u8];
    system.memory.read_bytes(object + QUEUED, &mut queued);
    system.memory.write_bytes(object + QUEUED, &[queued[0] & !2 | 1]);
    Some(vec![(LEAVE_PART, [object + LEAVE_FROM, found.name, 0]), (QUEUE_PART, [object + QUEUED_PART, found.name, 0])])
}

/// Solo's Move Out, Arms Altar, Practice Range, Idol Toss, Gear,
/// Treasure Hunt and Back: keys queue the name at +0xFC and set +0x150
/// bit 0. Its update posts the outline, sound, hint and camera together,
/// then copies the name to +0x5C; A uses it through 0x00370300.
fn outline_solo(system: &mut System, object: u32, found: Found) -> Option<Vec<MenuCall>> {
    // The opening guide owns input while +0x144 is 1.
    if read_word(system, object + 0x144) == 1 {
        return None;
    }
    if read_word(system, object + 0x5C) == found.hash {
        return Some(Vec::new());
    }
    let mut flags = [0u8];
    system.memory.read_bytes(object + 0x150, &mut flags);
    system.memory.write_bytes(object + 0x150, &[flags[0] | 1]);
    Some(vec![(LEAVE_PART, [object + 0x4C, found.name, 0]), (QUEUE_PART, [object + 0xFC, found.name, 0])])
}

/// Arms Altar's buttons: keys and touches both move the outline, sound
/// and animations by 0x00322DB0(object, &name, repeat), keeping the
/// current name at +0x44. A uses it through 0x00322C34.
const OUTLINE_ALTAR: u32 = 0x00322DB0;

fn outline_altar(system: &mut System, object: u32, found: Found) -> Option<Vec<MenuCall>> {
    let outlined = read_word(system, object + BUTTONS_OUTLINED) == found.hash;
    Some(if outlined { Vec::new() } else { vec![(OUTLINE_ALTAR, [object, found.name, 0])] })
}

/// Armory lists share the game's scroll control: a row at +0xE4,
/// viewport at +0xEC, row height at +0xFC, and cursor refresh 0x0031AED8.
const GOODS_PANEL: u32 = 0x569F4534;
const POWER_LIST_HASH: u32 = 0x104FFC79;
const REFRESH_LIST: u32 = 0x0031AED8;

fn armory_list(system: &mut System, object: u32) -> Option<u32> {
    match read_word(system, object) {
        0x00CEEBFC => Some(read_word(system, object + 0x58)),
        0x00CEE1E8 => Some(read_word(system, object + 0x80)),
        0x00CEC2E0 => Some(object + 0x1B74),
        _ => None,
    }.filter(|&list| list != 0)
}

fn list_row(system: &mut System, list: u32, y: f32) -> Option<u32> {
    let top = f32::from_bits(read_word(system, list + 4));
    let height = f32::from_bits(read_word(system, list + 0xFC)) * 10.0;
    let offset = f32::from_bits(read_word(system, list + 0x11C)) * 10.0;
    if !height.is_finite() || height <= 0.0 { return None; }
    let visible = ((y - top + offset) / height).floor();
    let row = read_word(system, list + 0xEC).checked_add(visible as u32)?;
    (visible >= 0.0 && row < read_word(system, list + 0xD0)).then_some(row)
}

fn outline_armory(system: &mut System, object: u32, found: Found) -> Option<Vec<MenuCall>> {
    let vtable = read_word(system, object);
    if vtable == 0x00CEC2B8 {
        let outlined = read_word(system, object + 0x2E4) == found.hash;
        return Some(if outlined { vec![] } else { vec![(0x00334D3C, [object, found.name, 0])] });
    }
    let pointer = system.aim_mouse.as_ref()?.pointer?;
    if vtable == 0x00CEE1C0 {
        let actor = kept_of_kind(system, found.part, 0x00D7681C);
        let category = actor.checked_sub(3)?;
        if category > 10 { return None; }
        if (1..10).contains(&category) {
            let singleton = read_word(system, 0x001FC76C);
            let save = read_word(system, singleton);
            let counts = save + 0x6474 + (category - 1) * 0x30;
            if save == 0 || !(0..12).any(|item| read_word(system, counts + item * 4) != 0) { return None; }
        }
        let old = read_word(system, object + 0xDC);
        if old == category { return Some(vec![]); }
        system.memory.write_bytes(object + 0xE0, &old.to_le_bytes());
        system.memory.write_bytes(object + 0xDC, &category.to_le_bytes());
        return Some(vec![(0x001FA9EC, [object, 0, 0])]);
    }
    if let Some(list) = armory_list(system, object) {
        if !matches!(found.hash, GOODS_PANEL | POWER_LIST_HASH | 0x2352A1DA) { return None; }
        let mut calls = Vec::new();
        if vtable == 0x00CEEBFC {
            match read_word(system, object + 0x45AC) {
                1 => {},
                2 => calls.push((0x00235F98, [object, 0, 0])),
                _ => return None,
            }
        }
        if vtable == 0x00CEE1E8 {
            match read_word(system, object + 0x2794) {
                0 => {},
                6..=8 => calls.push((0x001FAE48, [object, 0, 1])),
                _ => return None,
            }
        }
        let row = list_row(system, list, pointer.1)?;
        let old = read_word(system, list + 0xE4);
        if vtable == 0x00CEC2E0 && read_word(system, object + 0x48) != found.part {
            system.memory.write_bytes(object + 0x4C, &found.part.to_le_bytes());
            calls.push((0x002FEA08, [object, 0, 0]));
        }
        if old != row {
            let first = read_word(system, list + 0xEC);
            system.memory.write_bytes(list + 0xE4, &row.to_le_bytes());
            system.memory.write_bytes(list + 0xDC, &row.saturating_sub(first).to_le_bytes());
            calls.push((REFRESH_LIST, [list, 0, 0]));
            if vtable == 0x00CEC2E0 {
                system.memory.write_bytes(object + 0x1C58, &row.to_le_bytes());
                calls.push((0x002FF834, [object, u32::MAX, 0]));
            }
        }
        return Some(calls);
    }
    if vtable == 0x00CEFCA8 && found.hash == GOODS_PANEL {
        let row = ((pointer.1 - 38.0) / 23.0).floor();
        if row < 0.0 || row >= read_word(system, object + 0x5C4) as f32 { return None; }
        let row = row as u32;
        let mut calls = Vec::new();
        if read_word(system, object + 0x5A8) != 3 {
            calls.push((0x002CC0AC, [object, 0, 0]));
        }
        if read_word(system, object + 0x5C0) != row {
            system.memory.write_bytes(object + 0x5C0, &row.to_le_bytes());
            calls.extend([(0x002CC3B0, [object, 0, 0]), (0x0027F2E4, [object, 0, 0]), (0x0027EED4, [object, 0, 0])]);
        }
        return Some(calls);
    }
    if vtable == 0x00CEE148 && matches!(found.hash, 0x6166CB9B | 0x6166CBAD) {
        if read_word(system, object + 0x2B20) != 0 { return None; }
        let count = read_word(system, object + 0x2B40);
        let horizontal = found.hash == 0x6166CB9B;
        let visible = if horizontal { ((pointer.0 - 84.0) / 32.0).floor() } else { ((pointer.1 - 60.0) / 22.0).floor() };
        if visible < 0.0 || visible >= 7.0 { return None; }
        let scroll = if horizontal { 0x2B28 } else { 0x2B30 };
        let selected = if horizontal { 0x2B38 } else { 0x2B3C };
        if read_word(system, object + scroll) + visible as u32 >= count { return None; }
        if read_word(system, object + selected) == visible as u32 { return Some(vec![]); }
        system.memory.write_bytes(object + selected, &(visible as u32).to_le_bytes());
        return Some(vec![(0x002C5588, [object, 0, 1]), (0x002C5398, [object, 0, 0])]);
    }
    None
}

fn scroll_armory(system: &mut System, object: u32, steps: [i32; 2], scratch: u32) -> Vec<MenuCall> {
    let vtable = read_word(system, object);
    if let Some(list) = armory_list(system, object) {
        if vtable == 0x00CEEBFC && read_word(system, object + 0x45AC) != 1 { return vec![]; }
        if vtable == 0x00CEE1E8 && read_word(system, object + 0x2794) != 0 { return vec![]; }
        let mut calls = Vec::new();
        if vtable == 0x00CEE1E8 {
            system.memory.write_bytes(scratch + 0x30, &1u32.to_le_bytes());
            calls.extend((0..steps[0].unsigned_abs()).map(|_| (if steps[0] > 0 { 0x001C7E64 } else { 0x001C706C }, [object, scratch + 0x30, 0])));
        }
        let scrolls = if vtable == 0x00CF_B010 {
            read_word(system, list + 0xC8) > read_word(system, list + 0xD0)
        } else {
            read_word(system, list + 0xD0) > read_word(system, list + 0xC8) + 1
        };
        if scrolls {
            calls.extend((0..steps[1].unsigned_abs()).map(|_| (0x0031AA88, [list, u32::from(steps[1] > 0), 0])));
        }
        return calls;
    }
    if vtable == 0x00CEE1C0 {
        let axis = if steps[0] != 0 { 0 } else { 1 };
        let literal = match (axis, steps[axis] > 0) {
            (0, false) => 0x0019CA54, (0, true) => 0x0019CA58,
            (_, false) => 0x0019CA5C, (_, true) => 0x0019CA60,
        };
        let table = read_word(system, literal);
        return (0..steps[axis].unsigned_abs()).flat_map(|_| [(0x001C66BC, [object, table, 0]), (0x001FA9EC, [object, 0, 0])]).collect();
    }
    if read_word(system, object) == 0x00CEE148 && read_word(system, object + 0x2B20) == 0 {
        let max = read_word(system, object + 0x2B40).saturating_sub(7);
        for (axis, offset) in [0x2B28, 0x2B30].into_iter().enumerate() {
            let value = (i64::from(read_word(system, object + offset)) + i64::from(steps[axis])).clamp(0, i64::from(max)) as u32;
            system.memory.write_bytes(object + offset, &value.to_le_bytes());
        }
        return vec![(0x002C5588, [object, 0, 1]), (0x002C5398, [object, 0, 0])];
    }
    if read_word(system, object) == 0x00CEFCA8 && read_word(system, object + 0x5A8) == 3 {
        return (0..steps[1].unsigned_abs()).map(|_| (0x002CC320, [object, u32::from(steps[1] < 0), 0])).collect();
    }
    vec![]
}

/// These input handlers consume their own A/B event records. Posting the
/// press there uses precisely the same action and validation as the keys.
fn activate_armory(system: &mut System, object: u32) {
    let Some(found) = system.aim_mouse.as_ref().and_then(|mouse| mouse.menu_part) else { return };
    let base = match read_word(system, object) {
        0x00CEC2B8 => 0x514,
        0x00CEFCA8 => 0x930,
        0x00CEEBFC => 0x47DC,
        0x00CEE148 => 0x2DEC,
        0x00CEC2E0 => 0x180,
        0x00CEE1E8 => 0x2A64,
        0x00CEE1C0 => 0x2E4,
        _ => return,
    };
    let event = object + base + if found.hash == CHAPTER_BACK { 0x24 } else { 0 };
    let mut flags = [0u8; 2];
    system.memory.read_bytes(event, &mut flags);
    system.memory.write_bytes(event, &(u16::from_le_bytes(flags) | 1).to_le_bytes());
}

/// Other's rows (Profile, Subtitles, SpotPass and Back): queued as
/// Options' are, outlined by its own copy, 0x00289244, but its keys go
/// through one function, 0x002D5430(object, &{hash, name}, direction),
/// which queues the part and, for SpotPass, marks the move as a key's
/// (+0xC8, +0xCC 2). left 0, SpotPass's switch takes it for a touch on it
/// and keeps the touches for itself (the touch object's +0x654), so none
/// finds Profile after. A is 0x002D50FC(object).
const OUTLINE_OTHER_PART: u32 = 0x002D5430;

fn outline_other(system: &mut System, object: u32, found: Found) -> Option<Vec<MenuCall>> {
    let outlined = read_word(system, object + BUTTONS_OUTLINED) == found.hash;
    Some(if outlined { Vec::new() } else { vec![(OUTLINE_OTHER_PART, [object, found.name, 0])] })
}

/// Controls' rows (Control Configuration, Reticle/Camera, More Controls
/// and Back): queued at +0x9C as Options' are, but flagged by +0x118 (1),
/// +0x108 -1 and +0x11D's bit 3 the direction, then 0x00318DD0(object,
/// &{hash, name}) sets the rows' animations for it. 0x002E1CF8, early in
/// the update, outlines it. A is 0x003186E8(object).
const CONTROLS_QUEUED: u32 = 0x118;
const CONTROLS_DIRECTION: u32 = 0x11D;
const CONTROLS_HELD: u32 = 0x108;
const CONTROLS_ROWS: u32 = 0x00318DD0;

fn outline_controls(system: &mut System, object: u32, found: Found) -> Option<Vec<MenuCall>> {
    if read_word(system, object + BUTTONS_OUTLINED) == found.hash {
        return Some(Vec::new());
    }
    let mut direction = [0u8];
    system.memory.read_bytes(object + CONTROLS_DIRECTION, &mut direction);
    system.memory.write_bytes(object + CONTROLS_DIRECTION, &[direction[0] & !8]);
    system.memory.write_bytes(object + CONTROLS_QUEUED, &[1]);
    system.memory.write_bytes(object + CONTROLS_HELD, &u32::MAX.to_le_bytes());
    Some(vec![(QUEUE_PART, [object + QUEUED_PART, found.name, 0]), (CONTROLS_ROWS, [object, found.name, 0])])
}

/// Volume's rows (Music, SFX, Voice, To Music Gallery and Back): queued at
/// +0x9C as Options' are, by 0x002D5B40(object, &keys), which also zeroes
/// the held slider's +0xE4 and +0xE8 and, onto or off Back and the
/// gallery, plays their animations (not here). 0x002D562C outlines it;
/// A is 0x002D56CC(object), To Music Gallery opening it.
const VOLUME_HELD: u32 = 0xE4;

fn outline_volume(system: &mut System, object: u32, found: Found) -> Option<Vec<MenuCall>> {
    if read_word(system, object + BUTTONS_OUTLINED) == found.hash {
        return Some(Vec::new());
    }
    let mut queued = [0u8];
    system.memory.read_bytes(object + QUEUED, &mut queued);
    system.memory.write_bytes(object + QUEUED, &[queued[0] & !2 | 1]);
    system.memory.write_bytes(object + VOLUME_HELD, &[0; 8]);
    Some(vec![(QUEUE_PART, [object + QUEUED_PART, found.name, 0])])
}

/// Profile's (Player and Back): queued as Options' are but further on, the
/// hash at +0xA4, its name (a string) at +0xA8, +0xDC's bit 0 set (bit 1
/// a move back); 0x002952F4 outlines it. A is 0x002E2F4C(object).
const PROFILE_QUEUED_PART: u32 = 0xA4;
const PROFILE_QUEUED: u32 = 0xDC;

fn outline_profile(system: &mut System, object: u32, found: Found) -> Option<Vec<MenuCall>> {
    queue_hash_and_name(system, object, found, PROFILE_QUEUED_PART, PROFILE_QUEUED)
}

/// the Yes/No a choice asks (Profile's Player): queued as Profile's, the
/// hash at +0x9C, its name at +0xA0, +0xF0's bit 0; 0x00172A50, its
/// update, outlines it. A is 0x001C16D4(object).
const YES_NO_QUEUED_PART: u32 = 0x9C;
const YES_NO_QUEUED: u32 = 0xF0;

fn outline_yes_no(system: &mut System, object: u32, found: Found) -> Option<Vec<MenuCall>> {
    queue_hash_and_name(system, object, found, YES_NO_QUEUED_PART, YES_NO_QUEUED)
}

/// queues the part for the menu to outline, as its keys do: its hash at
/// part, then its name (a string), and bit 0 of the byte at flags (bit 1,
/// a move back, cleared).
fn queue_hash_and_name(system: &mut System, object: u32, found: Found, part: u32, flags: u32) -> Option<Vec<MenuCall>> {
    if read_word(system, object + BUTTONS_OUTLINED) == found.hash {
        return Some(Vec::new());
    }
    system.memory.write_bytes(object + part, &found.hash.to_le_bytes());
    let mut queued = [0u8];
    system.memory.read_bytes(object + flags, &mut queued);
    system.memory.write_bytes(object + flags, &[queued[0] & !2 | 1]);
    Some(vec![(COPY_NAME, [object + part + 4, found.name + 4, 0])])
}

/// the hub's choices in a box (SpotPass's No and Yes): the one picked an
/// index at +0x44, each button an element of one part named fnt_02 on,
/// for 0 on. its keys set it, then light it with 0x0028F8EC(object) and
/// 0x002DC68C(object); A, and a touch, use it with 0x00314DF8(object).
const DIALOG_PICKED: u32 = 0x44;
const LIGHT_DIALOG_PICKED: [u32; 2] = [0x0028F8EC, 0x002DC68C];

fn outline_dialog(system: &mut System, object: u32, found: Found) -> Option<Vec<MenuCall>> {
    let at = read_word(system, found.element);
    let mut name = [0u8; 7];
    system.memory.read_bytes(at, &mut name);
    let picked = match name {
        [b'f', b'n', b't', b'_', b'0', digit @ b'2'..=b'4', 0] => u32::from(digit - b'2'),
        _ => return None,
    };
    if read_word(system, object + DIALOG_PICKED) == picked {
        return Some(Vec::new());
    }
    system.memory.write_bytes(object + DIALOG_PICKED, &picked.to_le_bytes());
    Some(LIGHT_DIALOG_PICKED.iter().map(|&light| (light, [object, 0, 0])).collect())
}

/// what a part (its +0x30) keeps of a kind, as 0x00326D74(part, &kind)
/// +0x1C finds it: a sorted tree at +0x1A8 (its head) and +0x1AC (its
/// root), each node's left at +8, right at +0xC, ending at the head,
/// its kind at +0x14 and what it keeps at +0. 0 for none.
fn kept_of_kind(system: &mut System, part: u32, kind: u32) -> u32 {
    let head = part + 0x1A8;
    let (mut found, mut node) = (head, read_word(system, part + 0x1AC));
    for _ in 0..64 {
        if node == 0 || node == head {
            break;
        }
        if read_word(system, node + 0x14) < kind {
            node = read_word(system, node + 0xC);
        } else {
            found = node;
            node = read_word(system, node + 8);
        }
    }
    if found == head || kind < read_word(system, found + 0x14) {
        return 0;
    }
    let kept = read_word(system, found);
    if kept == 0 || kept == read_word(system, head) { 0 } else { read_word(system, kept + 0x1C) }
}

/// Subtitles' choices (Upper, Lower, Off): the outlined one an index,
/// +0x60's byte, into its parts, each a hash at +0x18 of 0x2C bytes from
/// what its +0x30 keeps of kind 0x00D70E54. its keys outline one with
/// 0x002E5608(object, &{hash, name}, direction); A is 0x002E5448(object).
const SUBTITLES_OUTLINED: u32 = 0x60;
const SUBTITLES_KIND: u32 = 0x00D7_0E54;
const SUBTITLES_PARTS: u32 = 0x18;
const SUBTITLES_PART_SIZE: u32 = 0x2C;
const OUTLINE_SUBTITLES_PART: u32 = 0x002E5608;

fn outline_subtitles(system: &mut System, object: u32, found: Found) -> Option<Vec<MenuCall>> {
    let owner = read_word(system, object + BUTTONS_SOUND_OWNER);
    let parts = kept_of_kind(system, owner, SUBTITLES_KIND);
    if parts != 0 {
        let mut index = [0u8];
        system.memory.read_bytes(object + SUBTITLES_OUTLINED, &mut index);
        if read_word(system, parts + SUBTITLES_PARTS + u32::from(index[0]) * SUBTITLES_PART_SIZE) == found.hash {
            return Some(Vec::new());
        }
    }
    Some(vec![(OUTLINE_SUBTITLES_PART, [object, found.name, 0])])
}

/// a scene's top screen drawing (0x0072A034, vtable 0x00CEF1A0), the
/// drawing in r4, its scene at +0x40 and its viewport at +0x44, x, y,
/// width and height in pixels: about to set the viewport and the top
/// camera (0x00727804), which go on to the top's 2D too, then draw the
/// scene's 3D (0x00727C24) when its layers 0xB to 0xD have models in them,
/// and just after that, which drew it into both eyes' buffers, the left
/// one's last.
const TOP_SCENE_3D: u32 = 0x0072A07C;
const TOP_SCENE_DRAWN: u32 = 0x0072A09C;
const TOP_LAYER: u32 = 0xB;

/// in the top's 3D (0x00727C24), which goes through the layers from 0xB
/// drawing each one's models, then its text (0x00718120, the layer in r1,
/// r0 the text's manager): the call for the text, and just after it. the
/// Fiend's Cauldron's hint is layer 0xB's.
const TOP_TEXT: u32 = 0x00727F10;
const TOP_TEXT_DRAWN: u32 = 0x00727F14;

/// a scene's bottom screen drawing, 0x007C8ECC(drawing), and its vtable.
/// with the scene at +0x40 and the viewport at +0x44 as for the top's, it
/// sets the viewport, the camera +0x54 says (1, the bottom's), and when
/// +0x5D is set draws four of the scene's layers, BOTTOM_LAYERS, then the
/// bottom screen's 2D. everything on the Fiend's Cauldron's bottom screen
/// is in those layers, as models, so it can be drawn on the top screen
/// too, wherever the viewport puts it.
const BOTTOM_SCENE_DRAW: u32 = 0x007C8ECC;
const BOTTOM_SCENE: u32 = 0x00CF6D6C;
/// where it is done with the layers, about to draw the 2D, and where it
/// returns from, past that.
const BOTTOM_SCENE_3D_DRAWN: u32 = 0x007C91C4;
const BOTTOM_SCENE_RETURN: u32 = 0x007C9220;

/// a scene's layers, 0x28 bytes each from the pointer at scene+0xEC, each
/// with its models' list at +4: a pointer to them at +0x18, past them at
/// +0x1C. a model's +8 is its CGFX model (CMDL), named by the offset at
/// +0xC from there.
const SCENE_LAYERS: u32 = 0xEC;
const LAYER_SIZE: u32 = 0x28;
const BOTTOM_LAYERS: std::ops::Range<u32> = 0xF..0x13;

/// the Fiend's Cauldron's models left off the top screen: the bottom
/// screen's background.
const CAULDRON_LEFT_OFF: [&str; 1] = ["MenBetHeartBt0010"];

/// the pot's task's value on the bar, the intensity, a float, and where
/// on the bottom screen the dial points for 0 and how far it goes for
/// each 1, the box around it under the bar from its arrow's tip, and how
/// wide and high that is.
const CAULDRON_VALUE: u32 = 0x6C;
const DIAL_ZERO: f32 = 33.0;
const DIAL_STEP: f32 = 28.3;
const DIAL_TOP: f32 = 30.0;
const DIAL_SIZE: [f32; 2] = [54.0, 54.0];
/// where on the bottom screen a press of the dial drawn on the top screen
/// goes, down the screen: the dial's middle.
const DIAL_GRAB: f32 = 60.0;
/// Move Out on the bottom screen, x, y, width and height: its place has
/// room around it for it to grow when lit, and only this is touched.
const MOVE_OUT_BUTTON: [f32; 4] = [55.0, 182.0, 210.0, 58.0];

/// where the top picture's intensity panel's middle is for 0 and how far
/// it goes for each 1, in top screen pixels: it slides with the value.
const PANEL_ZERO: f32 = 64.0;
const PANEL_STEP: f32 = 30.0;

/// its models drawn in places of their own: the bar's, the bar with the
/// dial on it, the "Try This?" tag and the crown marking the intensity
/// tried last, Back's and Move Out's. the rest, the pot, the pan and the
/// hearts bet, go in a fourth.
const CAULDRON_PARTS: [&[&str]; 3] = [&["MenBetHeartBt0002", "MenBetHeartBt0005", "MenBetHeartBt0006"], &["MenBetHeartBt9999"], &["MenBetHeartBt0000"]];

/// the top screen's picture, a model, and its background, two of its mesh
/// nodes: the dark frame around the board and the hearts' box, with a
/// hole where the board shows, and the shading over it. the board is one
/// of a strip of them, the one for each intensity, which the frame hides
/// the rest of.
const TOP_PICTURE: &str = "MenBetheartUp0000";
const TOP_BACKGROUND: [&str; 2] = ["polySurface45", "polySurface51"];
/// and its intensity's panel, the name and the number, another node,
/// drawn in a place of its own.
const TOP_PANEL: &str = "polySurface47";

/// the top's scene drawing's own parts, called again for the panel: the
/// viewport and the camera, then the 3D, the drawing in r0 and the scene
/// in r1.
const TOP_SCENE_SETUP: u32 = 0x00727804;
const TOP_SCENE_3D_DRAW: u32 = 0x00727C24;

/// a model's mesh nodes' visibility as drawn, 8 bytes each from the
/// pointer at +0x17C, whether it shows at +4. its CGFX model's dictionary
/// of them, at +0xD0 from there, has them in the same order.
const MODEL_NODES: u32 = 0x17C;

/// a scene's cameras, pointers from +0xC8, by screen. a camera's
/// projection is kept at +0x6C while bit 2 of +8 is set, turned for the
/// screen, see cropped_projection.
const SCENE_CAMERAS: u32 = 0xC8;

/// sets the GPU's viewport, 0x0012BC68(vsync manager, x, y, width, height
/// on the stack), and where the manager is kept.
const SET_VIEWPORT: u32 = 0x0012BC68;
const VSYNC_MANAGER: u32 = 0x00D63824;

/// where 0x006D269C goes on after 0x00B12218 summed the aim delta, which it
/// left at sp+8 as reticle pixels a second, with the aim object in r4 and
/// the time the step covers in s16.
const AFTER_AIM_DELTA: u32 = 0x006D26C4;

/// where 0x006D2D8C calls 0x006D2ABC, with the box the reticle moves in
/// before the camera turns loaded in s0 to s3 (right, left, bottom, top
/// edges), the aim object in r4. on the ground the camera turns by how far
/// the reticle goes past the box.
const BEFORE_CAMERA_TURN: u32 = 0x006D2ED4;

/// the 2D sprite batch's flush, entered with the batch in r0: it draws the
/// quads added since the last one, which end at [batch+0x38], count
/// [batch+0x3C] - [batch+0x40], 0x60 bytes each, four corners of 0x18 bytes
/// with x and y first, in pixels of the screen drawn to.
const SPRITE_FLUSH: u32 = 0x0092F408;

/// a camera's projection, 0x003901C4(camera), built from the lens at
/// camera+4 the first time and kept at camera+0x6C while bit 2 of camera+8
/// stays set. the 3D, the culling and the aim's ray all take it.
const PROJECTION: u32 = 0x003901C4;

/// the lenses that make a perspective one, their vtable, with the aspect,
/// width over height, at lens+0x14 (5:3 in battle), the vertical field
/// of view at +0x10 (45 degrees).
const PERSPECTIVE_LENS: u32 = 0x00D20080;
const LENS_FOVY: u32 = 0x10;
const LENS_ASPECT: u32 = 0x14;

/// a 2D context starting a pass, 0x0092E4F8(context, screen): 0 the top
/// screen, 1 the bottom one. the context is the sprite batch the pass's
/// flushes get.
const PASS_BEGIN: u32 = 0x0092E4F8;

/// In the top screen's battle UI pass, after every HUD branch, including
/// boss reveals without a score. The narration is drawn here too.
const TOP_UI_AFTER_HUD: u32 = 0x0060D66C;

/// a bottom screen call the top screen makes too, in the same frame's top
/// pass of the 2D context it draws through, with what the last bottom pass
/// gave it.
struct ToTop {
    /// the call's bl in the bottom pass.
    site: u32,
    /// where in the top pass it is made again, just after a call there.
    place: u32,
    /// r1 points at a 4x4 projection and r2 at a 3x4 view the call takes,
    /// which are on the bottom pass's stack, so they are kept, the
    /// projection made the top screen's.
    projection: bool,
}

/// the pause menu and the dialogs over it, Retreat's for one, in the
/// middle of the top screen.
const PAUSE_MENU: [ToTop; 15] = [
    // the bottom battle UI pass, 0x0060D93C, calling 0x00A01A1C while
    // paused: the dimming, Resume and Retreat, Options, the loot and Remove
    // Display, with the pause screen in r0, the 2D context in r1 and in r2
    // whether to show it. made again after the top pass, 0x0060D3EC, drew
    // the pause screen's label, which it does only while paused
    ToTop { site: 0x0060DBB4, place: 0x0060D68C, projection: false },
    // the dialogs' bottom pass, 0x00722A50, calling 0x00721C64, the
    // dimming and the box, with the dialogs' 2D context in r0 and how far
    // they are shown in s0, then 0x00721484, the buttons. made again after
    // their top pass, 0x007255AC, drew the corner icon it draws every frame
    ToTop { site: 0x00722AF4, place: 0x00725828, projection: false },
    ToTop { site: 0x00722B0C, place: 0x00725828, projection: false },
    // and once their 2D is done, the text, drawn through the 3D renderer
    // (r0) with a projection for the bottom screen: the lights (0x00909D54),
    // the camera (0x0090A5B4), the start (0x0090FFA0), the message and the
    // buttons' text (0x00909ACC, the text in r0), the end (0x0090A6A4).
    // made again after the top pass's 2D is done, for the right eye's
    // buffer, which 0x00916588 left it drawing into, and once it went back
    // to the left eye's, as the top pass draws its own text in both
    ToTop { site: 0x00722BA4, place: 0x007258A0, projection: false },
    ToTop { site: 0x00722C30, place: 0x007258A0, projection: true },
    ToTop { site: 0x00722C3C, place: 0x007258A0, projection: false },
    ToTop { site: 0x00722D14, place: 0x007258A0, projection: false },
    ToTop { site: 0x00722DAC, place: 0x007258A0, projection: false },
    ToTop { site: 0x00722DB8, place: 0x007258A0, projection: false },
    ToTop { site: 0x00722BA4, place: 0x007258C0, projection: false },
    ToTop { site: 0x00722C30, place: 0x007258C0, projection: true },
    ToTop { site: 0x00722C3C, place: 0x007258C0, projection: false },
    ToTop { site: 0x00722D14, place: 0x007258C0, projection: false },
    ToTop { site: 0x00722DAC, place: 0x007258C0, projection: false },
    ToTop { site: 0x00722DB8, place: 0x007258C0, projection: false },
];

/// how far right the pause menu goes on the top screen, its 320 pixels in
/// the middle of the top's 400, and so how far left a touch there goes.
const PAUSE_MENU_OFFSET: f32 = 40.0;

/// where the pause screen's sprite flushes come back to for Resume and
/// Retreat, their buttons, icons and words, one quad each a flush, which go
/// to the top screen's left and right edges.
const PAUSE_BUTTONS: [u32; 4] = [0x00A0D788, 0x00A0D88C, 0x00A0D9F8, 0x00A0DB00];

/// the rows Resume and Retreat are in, Resume from the left edge to 148,
/// Retreat from 172 to the right edge, bottom screen pixels.
const PAUSE_BUTTON_ROWS: std::ops::Range<f32> = 55.0..111.0;
const RESUME_RIGHT: f32 = 148.0;
const RETREAT_LEFT: f32 = 172.0;

/// and for Remove Display, in the bottom right corner from 193 and 204 on,
/// which goes to the top screen's.
const REMOVE_DISPLAY: [u32; 2] = [0x00A0D524, 0x00A0D5DC];
const REMOVE_DISPLAY_CORNER: [f32; 2] = [193.0, 204.0];

/// draws a message's line, shadow then text, from where the message says,
/// x at +0x14, y at +0x18, its size at +0x1C, +0x20: r0 the message, r1 the
/// 2D context.
const DRAW_MESSAGE_LINE: u32 = 0x009E9A14;

/// the top HUD's score, flushed from the 2D context the narration's message
/// is part of.
const SCORE_FLUSH_CALLER: u32 = 0x009F2F90;

/// the narration's message in the 2D context, shown when its first byte
/// and its line at +4 are set.
const NARRATION: u32 = 0x3124;

/// draws the speakers' portraits, up to four 256-pixel squares placed for
/// the bottom screen: r0 the portraits, r1 the 2D context.
const DRAW_PORTRAITS: u32 = 0x009E2BB4;

/// where in the 2D context the bottom screen's portraits are kept, a pointer.
const PORTRAITS: u32 = 0xC3C;

/// how far the bottom screen's UI has faded out, 0 when fully shown: the
/// bottom pass hands it on (0x0060DA48), and the portraits are drawn at one
/// less it, and not at all once it is 1 (0x00A182C4).
const BOTTOM_FADE: u32 = 0x2454;

/// a bottom screen element drawn on the top screen too: the call the bottom
/// pass makes to draw it, which the top pass makes again with what the
/// bottom pass gave it, and where its sprites go.
struct Mirrored {
    /// the call's bl, in the bottom screen's background drawing (0x00A17F70),
    /// with the bottom UI in r0, the 2D context in r1 and floats in s0..s2.
    site: u32,
    /// the bottom screen's corner the element is drawn by.
    from: [f32; 2],
    /// the top screen's corner it goes by, and how far down from it in HUD
    /// pixels, the height of what is above it there.
    to: [f32; 2],
    below: f32,
    /// its size against the rest of the HUD.
    size: f32,
    /// two rows swapped, bottom screen pixels: quads centred above the first
    /// go down by the second, the rest up by the third.
    swap_rows: Option<[f32; 3]>,
    /// where its sprite flushes come back to that draw something left off
    /// the top screen.
    hidden: &'static [u32],
    /// the sprites of the call's last flush, which comes back where the call
    /// does, placed on their own.
    last: Option<Apart>,
}

/// where a mirrored element's last flush goes instead, as for Mirrored.
struct Apart {
    from: [f32; 2],
    to: [f32; 2],
    below: f32,
    size: f32,
}

/// the chapter title and hearts, under the score, their rows as on the
/// bottom screen: the intensity and the chapter's hearts, then the hearts
/// held;
/// the item box and power icon, under the health bar; what was just picked
/// up ("You got treasure!", 0x009EA06C with the bottom UI's +0xDC) at the
/// top's middle, and the item held, which the same call draws in the bottom
/// right corner, bigger under the power icon, without the tab behind it
/// (0x009EAC80). the chapter title is left off.
const BOTTOM_BATTLE_UI: u32 = 0x00A17F70;
const ILLUSTRATION: usize = 3;
const BOSS_CARD: usize = 4;
const BOSS_EFFECT: usize = 5;
const BOSS_STATUS: usize = 6;
const PICKUP: usize = 7;

const MIRRORED: [Mirrored; 8] = [
    Mirrored { site: 0x00A181CC, from: [320.0, 0.0], to: [400.0, 0.0], below: 40.0, size: 1.0, swap_rows: None, hidden: &[0x00A192E4], last: None },
    Mirrored { site: 0x00A18400, from: [0.0, 194.0], to: [0.0, 0.0], below: 36.0, size: 0.75, swap_rows: None, hidden: &[], last: None },
    Mirrored { site: 0x00A18430, from: [0.0, 194.0], to: [0.0, 0.0], below: 36.0, size: 0.75, swap_rows: None, hidden: &[], last: None },
    // The illustration call also draws the bottom screen's black bands.
    // Only its image flush (0x00A0F9E8) belongs on top.
    Mirrored { site: 0x00A18244, from: [0.0; 2], to: [0.0; 2], below: 0.0, size: 1.0, swap_rows: None, hidden: &[], last: None },
    Mirrored { site: 0x00A18204, from: [0.0; 2], to: [0.0; 2], below: 0.0, size: 1.0, swap_rows: None, hidden: &[], last: None },
    Mirrored { site: 0x00A18224, from: [0.0; 2], to: [0.0; 2], below: 0.0, size: 1.0, swap_rows: None, hidden: &[], last: None },
    Mirrored { site: 0x00A18130, from: [0.0; 2], to: [0.0; 2], below: 0.0, size: 1.0, swap_rows: None, hidden: &[], last: None },
    Mirrored { site: 0x00A18368, from: [162.0, 118.0], to: [200.0, 0.0], below: 8.0, size: 0.75, swap_rows: None, hidden: &[0x009EAC80],
        last: Some(Apart { from: [289.0, 209.0], to: [4.0, 0.0], below: 76.0, size: 1.5 }) },
];

/// A rectangle in native UI pixels, before the widescreen correction.
#[derive(Clone, Copy, Debug)]
struct UiBox {
    min: [f32; 2],
    max: [f32; 2],
}

impl UiBox {
    fn valid(self) -> bool {
        (0..2).all(|axis| self.min[axis].is_finite() && self.max[axis].is_finite() && self.max[axis] > self.min[axis])
    }
}

#[derive(Clone, Copy)]
struct MirroredCall {
    call: Recorded,
    pass: u64,
    bounds: Option<UiBox>,
    material: [u32; 4],
}

impl MirroredCall {
    fn for_pass(self, pass: u64) -> bool {
        // Count UI passes, not vblanks: heavy scenes can take several
        // refreshes to render. A missing bottom pass expires on the next
        // top pass, independently of the game's frame rate.
        self.pass.checked_add(1) == Some(pass)
    }
}

/// what a mirrored element's call was given in the bottom pass, made again
/// in the next frame's top pass.
#[derive(Clone, Copy)]
struct Recorded {
    target: u32,
    regs: [u32; 4],
    floats: [u32; 8],
    /// the projection and the view a call takes, see ToTop::projection.
    matrices: Option<[u32; 28]>,
}

/// what the game's code is drawing on the top screen for us.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Drawing {
    Portraits,
    Mirrored(usize),
    Narration,
    PauseMenu(usize),
    /// the Fiend's Cauldron's top picture's panel, its setup then its 3D,
    /// then the bottom screen, its bar, the bar's dial, Back, then the rest.
    Cauldron(usize),
    /// A scripted menu part, its camera setup and drawing.
    MenuTop(usize),
    MenuClip(u32),
    /// the Fiend's Cauldron's cursor, lit where the mouse went as an arrow
    /// key would: set, then moved.
    CauldronCursor(usize),
    /// the viewport, set for what is drawn from the place on.
    Viewport(u32),
    /// a menu of MENU_SCREENS, the second, outlining what is under the
    /// mouse by its own code before its update: what is there found, its
    /// part's descriptor, then each of the menu's calls.
    Menu(usize, usize),
}

impl Drawing {
    /// where in the top pass it is drawn, and comes back to.
    fn place(self) -> u32 {
        match self {
            Drawing::PauseMenu(index) => PAUSE_MENU[index].place,
            Drawing::Cauldron(_) => TOP_SCENE_DRAWN,
            Drawing::MenuTop(_) => menu_top::FINISHED,
            Drawing::CauldronCursor(_) => CAULDRON_CURSOR_KEYS_DONE,
            Drawing::Viewport(place) | Drawing::MenuClip(place) => place,
            Drawing::Menu(_, screen) => MENU_SCREENS[screen].update,
            _ => TOP_UI_AFTER_HUD,
        }
    }
}

/// what the CPU held where the call was made in the game's place, put back
/// when the call returns there, and the memory changed for the call, where
/// and what it held, put back too.
struct Saved {
    regs: [u32; 16],
    cpsr: zakuro_cpu::psr::Psr,
    vfp: [u32; 32],
    drawing: Drawing,
    moved: Vec<(u32, Vec<u8>)>,
}

/// where placing the reticle, 0x007C4854, hands each of its two parts the
/// size and place to draw it at: r1 points at them, the size first, r4
/// counts the parts. the size it keeps (reticle+0x5C) goes on into what the
/// reticle takes in, so only this copy is made smaller.
const RETICLE_DRAW: u32 = 0x007C4DC0;

/// the battle UI's handling of what its touches asked for, the UI object
/// in r0 and r1 0 while a battle is played. its power panel's command word
/// asks it to use a power: bit 0, the power's place in the panel's list
/// from bit 17, its id from bit 23, the box touched from bit 15. it uses
/// it through 0x003636F8, which checks the player can and has uses left,
/// and clears the word after.
const UI_COMMANDS: u32 = 0x00A30D00;

/// the power panel in the 2D context at the UI object's +0x140, the strip
/// of three power boxes in the bottom screen's lower left corner, the
/// current power in the middle one. what 0x00A129F0, its touch handling,
/// and 0x00A17A10, its update, look at before they ask for a power.
const POWER_PANEL: u32 = 0xB64;
/// bit 0x800000 of its first word: no power can be used now.
const POWERS_LOCKED: u32 = 0x0080_0000;
/// bits of its second word that keep a touch from asking for one.
const POWER_PANEL_BUSY: u32 = 0x2800_0000;
const POWER_COMMAND: u32 = 8;
/// the current power's place in the list, the list's length, the list,
/// each a byte naming one of the panel's entries of 8 bytes: uses left at
/// +0x11C, id at +0x11E, 1 at +0x121 while it can't be used.
const CURRENT_POWER: u32 = 0x118;
const POWER_COUNT: u32 = 0x184;
const POWER_LIST: u32 = 0x188;
/// what of the command word 0x00A17A10 keeps when it writes one.
const POWER_COMMAND_KEPT: u32 = 0x007E_7FEE;

/// the battle UI's input while paused, 0x0060DC7C, with the pause screen
/// in r9: where it hands the key pressed, r5, to 0x00A0C5A8, its keys'
/// handling, a code from 0x00A3115C, -1 for none.
const PAUSE_KEY: u32 = 0x0060DD04;
/// then the touch to 0x00A01DA0, its touches': whether touching in r8,
/// whether that began now in r6, the pixel in r7 and s16. the touch picks
/// the option under it, and only one that began now uses it.
const PAUSE_TOUCH: u32 = 0x0060DD10;
/// and where it goes on, after both.
const PAUSE_INPUT_DONE: u32 = 0x0060DD28;
/// where the main page's touches, 0x00A0867C, and an Options page's,
/// 0x00A06188, entered with the pause screen in r0, found the option under
/// the touch: r1 on the main page, with the pause screen in r0, r4 on a
/// page of Options, with it in r6. off every option they return before.
const PAUSE_TOUCHES: u32 = 0x00A0867C;
const OPTIONS_TOUCHES: u32 = 0x00A06188;
const PAUSE_OPTION_FOUND: u32 = 0x00A08820;
const OPTIONS_OPTION_FOUND: u32 = 0x00A06384;
/// the pause screen's state, 2 its main page, 7 Options, the page of
/// Options shown and the one being gone to, and the option picked, which
/// is drawn lit, -1 for none.
const PAUSE_STATE: u32 = 4;
const OPTIONS_PAGE: u32 = 0x678;
const OPTIONS_PAGE_NEXT: u32 = 0x6EC;
const PAUSE_CURSOR: u32 = 0x67C;
/// the page of Options whose touches drag what they hold, Control
/// Configuration.
const OPTIONS_DRAG_PAGE: u32 = 5;
/// pages of Options whose sliders a held touch sets, where the page looks
/// for an option: Reticle/Camera's, in the rows above 70, and Volume's,
/// from 121 to 288 across each row, its name left of that.
const CAMERA_PAGE: u32 = 6;
const CAMERA_SLIDERS_BELOW: i32 = 70;
const VOLUME_PAGE: u32 = 7;
const VOLUME_SLIDERS: std::ops::RangeInclusive<i32> = 121..=288;
/// the keys that move or use the pause screen's option: the directions,
/// then the codes 0x00A0C5A8 uses it on.
const CURSOR_KEYS: [i32; 7] = [0, 1, 2, 3, 4, 6, 9];

/// a dialog's input, 0x007226C4, the dialog in r4, the option picked before
/// in r6, just after it read 0x00A3115C's input onto the stack: the pixel
/// touched at sp, whether that began now at sp+9, the key at sp+0xA. only a
/// touch that began now picks a button, which uses it.
const DIALOG_INPUT: u32 = 0x00722758;
/// and where it plays the cursor's sound for an option picked by a key, r5
/// -1, after it lit the one picked.
const DIALOG_CURSOR_SOUND: u32 = 0x007229E4;
/// the dialog's buttons: how many, their boxes, 4 shorts each, left, right,
/// top, bottom, inclusive, and the one picked, -1 for none.
const DIALOG_BUTTON_COUNT: u32 = 0x28C;
const DIALOG_BUTTONS: u32 = 0x2A8;
const DIALOG_CURSOR: u32 = 0x294;

/// a pause screen's touch given a hover in place of the real one's.
struct Hovering {
    /// r6, r7, r8 and s16, as they were.
    touch: [u32; 4],
    /// whether the page's touches looked, and found an option.
    looked: bool,
    found: bool,
}

/// where the Fiend's Cauldron's two screens go on the top screen, each
/// screen's backgrounds left off: the top's picture, the bottom's bar, its
/// Back button, and the rest of the bottom over them. the top's hint is
/// drawn as before.
#[derive(Clone, Copy)]
pub struct CauldronLayout {
    pub top: CauldronPlace,
    /// the top picture's intensity panel, drawn on its own over the bar,
    /// sliding along it with the dial as it slides under the picture: from's
    /// x is where it has slid to under the picture and to's where over the
    /// bar, both set each frame.
    pub panel: CauldronPlace,
    pub bar: CauldronPlace,
    /// the bar's dial, under the bar, drawn on its own at this size against
    /// the bar's, its arrow still at the bar's value.
    pub dial: f32,
    pub back: CauldronPlace,
    pub move_out: CauldronPlace,
    pub rest: CauldronPlace,
}

impl CauldronLayout {
    /// the layout for a top screen width pixels wide as shown, 240 high:
    /// the top's picture and under it the panel, the bar, its dial and Back
    /// on the left, the pot over Move Out on the right, made for 16:9 and
    /// smaller on a narrower screen.
    pub fn fitting(width: f32) -> CauldronLayout {
        const WIDTH: f32 = 427.0;
        let scale = (width / WIDTH).min(1.0);
        // right is by the right edge, down by the bottom
        let place = |to: [f32; 4], from: [f32; 4], right: bool, down: bool| {
            let x = to[0] * scale + if right { width - WIDTH * scale } else { 0.0 };
            let y = to[1] * scale + if down { 240.0 * (1.0 - scale) } else { 0.0 };
            CauldronPlace { from, to: [x, y, to[2] * scale, to[3] * scale] }
        };
        CauldronLayout {
            top: place([16.0, 24.0, 258.0, 172.0], [36.0, 22.0, 328.0, 218.0], false, false),
            panel: place([0.0, 148.0, 91.0, 49.0], [0.0, 176.0, 140.0, 76.0], false, true),
            bar: place([40.0, 197.0, 220.0, 23.0], [14.0, 0.0, 312.0, 32.0], false, true),
            dial: 0.55,
            back: place([9.0, 207.0, 33.0, 30.0], [6.0, 202.0, 42.0, 38.0], false, true),
            move_out: place([244.9, 192.9, 171.1, 47.5], [16.0, 160.0, 288.0, 80.0], true, true),
            rest: place([188.0, 47.0, 237.4, 178.1], [0.0, 0.0, 320.0, 240.0], true, true),
        }
    }
}

impl CauldronPlace {
    /// where a box of from's screen, x, y, width and height, shows.
    fn to_box(&self, part: [f32; 4]) -> [f32; 4] {
        let (scale_x, scale_y) = (self.to[2] / self.from[2], self.to[3] / self.from[3]);
        [self.to[0] + (part[0] - self.from[0]) * scale_x, self.to[1] + (part[1] - self.from[1]) * scale_y, part[2] * scale_x, part[3] * scale_y]
    }
}

/// a part of a screen, from, shown in a box of the top screen, to: x, y
/// from the top left, width and height, in the screen's pixels, the top's
/// 400 by 240 and the bottom's 320 by 240, and to in top screen pixels as
/// shown, 400 by 240 times widen.
#[derive(Clone, Copy)]
pub struct CauldronPlace {
    pub from: [f32; 4],
    pub to: [f32; 4],
}

pub struct AimMouse {
    /// reticle pixels moved since the game last took them.
    pub pending: [f32; 2],
    /// whether the mouse is aiming, which on the ground keeps the reticle
    /// in the middle and turns the camera by all of the mouse's movement.
    pub captured: bool,
    /// how big the top screen's HUD and reticle are drawn, 1 as the game
    /// draws them.
    pub hud_scale: f32,
    /// whether the narration is drawn on the top screen too.
    pub narration_on_top: bool,
    /// how big the portraits are drawn on the top screen, 0 for not at all.
    pub portrait_scale: f32,
    /// the 2D context the top HUD was last drawn through.
    context: Option<u32>,
    /// the CPU while the game draws the portraits or the narration on the
    /// top screen.
    saved: Option<Saved>,
    /// whether the bottom screen's hearts, powers, pickups, dialogue images
    /// and boss information are drawn on the top screen too.
    pub mirror_bottom: bool,
    /// the mirrored elements' calls from the last bottom pass, by MIRRORED's
    /// order.
    recorded: [Option<MirroredCall>; MIRRORED.len()],
    extras_pass: u64,
    /// whether the pause menu and the dialogs over it are drawn on the top
    /// screen too.
    pub pause_on_top: bool,
    /// their calls from the last bottom pass, by PAUSE_MENU's order.
    pause: [Option<Recorded>; 15],
    /// the frame the pause menu was last drawn on the top screen in.
    pause_drawn: Option<u64>,
    /// and the last one Resume, Retreat or Remove Display was drawn by the
    /// top screen's edges in.
    edges_drawn: Option<u64>,
    /// the frame the top screen's battle UI was last drawn in, which it is
    /// in every frame of a battle and in no menu's.
    battle_drawn: Option<u64>,
    /// the frame the Fiend's Cauldron was last shown in, and whether its pot
    /// was pouring then.
    cauldron_drawn: Option<u64>,
    cauldron_pouring: bool,
    /// the pot's task, which those updates are for.
    cauldron_task: Option<u32>,
    /// whether the Fiend's Cauldron's bottom screen is drawn on the top
    /// screen too, and where each screen's picture goes there, when not
    /// CauldronLayout::fitting the top screen.
    pub cauldron_on_top: bool,
    pub cauldron_layout: Option<CauldronLayout>,
    /// where the bar's dial was last drawn, which moves with its value.
    cauldron_dial: Option<CauldronPlace>,
    /// the part of it a press of the mouse began in, which it stays with
    /// until let go, a drag going on past it.
    touched_part: Option<usize>,
    /// the part the mouse was last over, and the cursor's name to light
    /// for it, once, when it went there.
    hovered_part: Option<usize>,
    cursor_wanted: Option<usize>,
    /// the bottom screen's scene drawing the last bottom pass made.
    bottom_scene: Option<u32>,
    /// the top screen's viewport while the top's picture is drawn smaller,
    /// put back before its 2D, and the one it is drawn in.
    top_viewport: Option<([u8; 16], [u8; 16])>,
    /// what was changed in memory for the top's picture, put back after
    /// its 3D.
    top_moved: Vec<(u32, Vec<u8>)>,
    /// the top camera's projection while it shows a part of the picture,
    /// where it is kept, and the game's and the one that shows that part:
    /// the text's place on the screen comes from it, so it gets the game's.
    top_projection: Option<(u32, [u8; 64], [u8; 64])>,
    /// the frame the power key was pressed in, for the battle UI's next
    /// update to use the current power, unless that is long after.
    power_requested: Option<u64>,
    /// whether the mouse over a menu's options lights the one under it, as
    /// on a PC, without using it, and over none lights none. keys bring the
    /// game's own cursor back until the mouse moves again.
    pub menu_hover: bool,
    /// Scripted menus also draw their controls on the top screen.
    pub menus_on_top: bool,
    menu_top: MenuTop,
    /// the bottom screen pixel the mouse is over, none off it.
    pointer: Option<(f32, f32)>,
    /// whether the mouse moved since a key was last pressed in a menu.
    mouse_mode: bool,
    /// the frame the pause screen last took input in.
    paused: Option<u64>,
    /// the pause screen's option a hover over nothing took the light off,
    /// for the next key to light it again.
    cleared: Option<u32>,
    /// the pause screen's touch while a hover takes its place.
    hovering: Option<Hovering>,
    /// whether a hover took the dialog's light off its buttons, for its
    /// cursor's sound not to play for that.
    dialog_cleared: bool,
    /// the menus' touch object, from where it last had no touch, and the
    /// frame.
    menu_touch: Option<(u32, u64)>,
    /// the menu of MENU_SCREENS last updated, its object and the frame.
    menu_screen: Option<(usize, u32, u64)>,
    /// the menus of MENU_SCREENS updating, each object with the frame it
    /// was first seen and last: of two at once (a choice over Other), the
    /// mouse is the newer one's.
    menus_open: Vec<(u32, u64, u64)>,
    /// where the mouse was when what is under it was first looked for there,
    /// and the frame, and what was found there that its menu outlines.
    menu_looked: Option<((f32, f32), u64)>,
    menu_part: Option<Found>,
    /// the menu's calls still to make to outline it.
    menu_calls: Vec<MenuCall>,
    /// whether a click is to use the part under the mouse once it is
    /// outlined.
    menu_click: bool,
    menu_button_down: bool,
    menu_press: Option<MenuPress>,
    /// Keep hover out of a native drag's release handling too.
    menu_touched: Option<u64>,
    /// Wheel fractions, kept with the menu that received them.
    menu_wheel: Option<(u32, u64, [f32; 2])>,
    /// how many times wider than the console's 5:3 the top screen shows,
    /// 1 for its own shape: the view is that much wider and the top
    /// screen's 2D that much narrower, so it shows the right shape stretched.
    pub widen: f32,
    /// the 2D contexts seen starting a pass, and the screen each draws.
    screens: Vec<(u32, u32)>,
    /// the lenses widened: where, the aspect the game gave each and the one
    /// written over it.
    lenses: Vec<(u32, f32, f32)>,
    /// tan of half the view's width at the aspect the game gave, 0.69 in
    /// battle.
    half_width: f32,
}

impl Default for AimMouse {
    fn default() -> AimMouse {
        AimMouse {
            pending: [0.0; 2],
            captured: false,
            hud_scale: 1.0,
            narration_on_top: false,
            portrait_scale: 0.0,
            context: None,
            saved: None,
            mirror_bottom: false,
            recorded: [None; MIRRORED.len()],
            extras_pass: 0,
            pause_on_top: false,
            pause: [None; 15],
            pause_drawn: None,
            edges_drawn: None,
            battle_drawn: None,
            cauldron_drawn: None,
            cauldron_pouring: false,
            cauldron_task: None,
            cauldron_on_top: false,
            cauldron_layout: None,
            cauldron_dial: None,
            touched_part: None,
            hovered_part: None,
            cursor_wanted: None,
            bottom_scene: None,
            top_viewport: None,
            top_moved: Vec::new(),
            top_projection: None,
            power_requested: None,
            menu_hover: false,
            menus_on_top: true,
            menu_top: MenuTop::default(),
            pointer: None,
            mouse_mode: false,
            paused: None,
            cleared: None,
            hovering: None,
            dialog_cleared: false,
            menu_touch: None,
            menu_screen: None,
            menus_open: Vec::new(),
            menu_looked: None,
            menu_part: None,
            menu_calls: Vec::new(),
            menu_click: false,
            menu_button_down: false,
            menu_press: None,
            menu_touched: None,
            menu_wheel: None,
            widen: 1.0,
            screens: Vec::new(),
            lenses: Vec::new(),
            half_width: (std::f32::consts::FRAC_PI_8).tan() * 5.0 / 3.0,
        }
    }
}

impl AimMouse {
    /// for the game, drawing the HUD at hud_scale, the narration on the top
    /// screen too when narration_on_top, the portraits there at
    /// portrait_scale, 0 for not, the hearts, item box and power icon there
    /// when mirror_bottom, the pause menu when pause_on_top, and menus lit
    /// by the mouse over them when menu_hover.
    pub fn new(hud_scale: f32, narration_on_top: bool, portrait_scale: f32, mirror_bottom: bool, pause_on_top: bool, menu_hover: bool) -> AimMouse {
        AimMouse { hud_scale, narration_on_top, portrait_scale, mirror_bottom, pause_on_top, menu_hover, ..AimMouse::default() }
    }

    /// the mouse moved to the bottom screen's pixel, or off it: menus light
    /// what is under it from now on, until a key is pressed in one.
    pub fn hover(&mut self, pointer: Option<(f32, f32)>) {
        self.pointer = pointer;
        self.mouse_mode = true;
    }

    /// a click of the mouse where it is, which the menu shown takes when it
    /// is over a part the menu outlines: that part is outlined and used, as
    /// A uses it, and nothing is touched. frame as for pause_menu_shown.
    pub fn menu_click(&mut self, frame: u64) -> bool {
        let taken = self.menu_clickable(frame);
        self.menu_click |= taken;
        taken
    }

    fn menu_clickable(&self, frame: u64) -> bool {
        if self.native_menu_on_top(frame) { return false; }
        let shown = self.menu_screen.is_some_and(|(screen, _, seen)| frame <= seen + self.menu_age() && MENU_SCREENS[screen].use_outlined != CLICK_TOUCHES);
        self.menu_hover && shown && self.menu_part.is_some() && self.pointer.is_some() && self.pointer == self.menu_looked.map(|(pointer, _)| pointer)
    }

    /// The native touch for a physical mouse button, polled each frame
    /// while held. Unmapped controls touch immediately, as before.
    pub fn menu_touch(&mut self, frame: u64, pressed: bool) -> Option<(f32, f32)> {
        if self.menu_top.blocked {
            self.menu_top.blocked = pressed;
            self.menu_button_down = pressed;
            return None;
        }
        let age = self.menu_age();
        let was_down = std::mem::replace(&mut self.menu_button_down, pressed);
        if !pressed {
            if let Some(press) = self.menu_press.take() {
                if press.dragging.is_none() && frame < press.since + 12 && self.menu_screen.is_some_and(|(_, object, seen)| object == press.object && frame <= seen + age) {
                    self.menu_click(frame);
                } else {
                    self.menu_looked = None;
                    self.menu_part = None;
                }
            }
            return None;
        }
        if !was_down && self.menu_clickable(frame) {
            self.menu_press = Some(MenuPress {
                pointer: self.pointer.unwrap(),
                object: self.menu_screen.unwrap().1,
                since: frame,
                dragging: None,
            });
        }
        let Some(press) = &mut self.menu_press else {
            if self.pointer.is_some() {
                self.menu_touched = Some(frame);
            }
            return self.pointer;
        };
        // A transition or leaving the screen cancels this gesture.
        if self.pointer.is_none() || !self.menu_screen.is_some_and(|(_, object, seen)| object == press.object && frame <= seen + age) {
            press.object = 0;
            return None;
        }
        let pointer = self.pointer.unwrap();
        let moved = (pointer.0 - press.pointer.0).hypot(pointer.1 - press.pointer.1) >= 4.0;
        if press.dragging.is_none() && (moved || frame >= press.since + 12) {
            press.dragging = Some(frame);
        }
        // Keep the initial point through a native input poll, including
        // the extra drawing time of a top-screen menu.
        press.dragging.map(|started| {
            self.menu_touched = Some(frame);
            if frame <= started + age.saturating_sub(1).max(2) { press.pointer } else { pointer }
        })
    }

    /// Losing focus or capturing the mouse cancels without clicking.
    pub fn cancel_menu_touch(&mut self) {
        self.menu_top.blocked |= self.menu_button_down;
        self.menu_top.grab = None;
        self.menu_button_down = false;
        self.pointer = None;
        self.menu_looked = None;
        self.menu_part = None;
        self.menu_press = None;
        self.menu_click = false;
        self.menu_wheel = None;
    }

    /// Wheel amounts in rows/columns, positive down/right. Never touch
    /// or activate an item, and never carry a wheel gesture into a battle.
    pub fn scroll_menu(&mut self, frame: u64, delta: [f32; 2]) -> bool {
        let Some((index, object, seen)) = self.menu_screen else { return false };
        if index < 14 || frame > seen + self.menu_age() || !self.menu_hover || self.captured || self.pointer.is_none() || self.menu_button_down
            || delta.iter().any(|value| !value.is_finite()) {
            return false;
        }
        let mut pending = self.menu_wheel.filter(|&(owner, last, _)| owner == object && frame <= last + 30).map_or([0.0; 2], |(_, _, pending)| pending);
        for axis in 0..2 { pending[axis] = (pending[axis] + delta[axis]).clamp(-32.0, 32.0); }
        self.menu_wheel = Some((object, frame, pending));
        true
    }

    pub fn add(&mut self, dx: f32, dy: f32) {
        self.pending[0] += dx;
        self.pending[1] += dy;
    }

    /// whether the pause menu or a dialog over it was drawn on the top
    /// screen lately, by frame, the frames the System counts: at 30 FPS the
    /// game draws in every other one, and in a slow one a little less.
    pub fn pause_menu_shown(&self, frame: u64) -> bool {
        self.pause_drawn.is_some_and(|drawn| frame <= drawn + 3)
    }

    /// whether a battle is being played, not paused, frame as for
    /// pause_menu_shown.
    pub fn in_battle(&self, frame: u64) -> bool {
        self.battle_drawn.is_some_and(|drawn| frame <= drawn + 3) && !self.pause_menu_shown(frame)
    }

    /// whether the Fiend's Cauldron is shown, frame as for pause_menu_shown.
    pub fn cauldron_shown(&self, frame: u64) -> bool {
        self.cauldron_drawn.is_some_and(|drawn| frame <= drawn + 3)
    }

    /// the Fiend's Cauldron's layout on the top screen, none when it is not
    /// drawn there.
    fn cauldron_layout(&self) -> Option<CauldronLayout> {
        self.cauldron_on_top.then(|| self.cauldron_layout.unwrap_or_else(|| CauldronLayout::fitting(400.0 * self.widen)))
    }

    /// whether the Fiend's Cauldron is shown on the top screen, frame as
    /// for pause_menu_shown.
    pub fn cauldron_on_top_shown(&self, frame: u64) -> bool {
        self.cauldron_on_top && self.cauldron_shown(frame)
    }

    /// the bottom screen pixel a touch of the top screen's pixel x, y
    /// touches while the Fiend's Cauldron is shown there, x of the top
    /// screen's 400 as for pause_menu_touch, frame as for
    /// pause_menu_shown: in the box of the part drawn there, or while
    /// pressed, the part the press began in. a press of the dial goes
    /// along the bar, where it is the dial's. none off them.
    pub fn cauldron_touch(&mut self, frame: u64, x: f32, y: f32, pressed: bool) -> Option<(f32, f32)> {
        let layout = self.cauldron_layout().filter(|_| self.cauldron_shown(frame))?;
        let x = x * self.widen;
        // by the order they are drawn over each other, the dial first
        let parts = [self.cauldron_dial, Some(layout.bar), Some(layout.back), Some(layout.move_out), Some(layout.rest)];
        let button = layout.move_out.to_box(MOVE_OUT_BUTTON);
        let inside = |place: &CauldronPlace| {
            let [left, top, width, height] = if place.from == layout.move_out.from { button } else { place.to };
            (left..left + width).contains(&x) && (top..top + height).contains(&y)
        };
        let over = parts.iter().position(|place| place.as_ref().is_some_and(inside));
        if over != self.hovered_part {
            // the dial, the bar, Back and Move Out light the cursor's
            // names in turn
            self.cursor_wanted = over.and_then(|part| [Some(0), Some(0), Some(2), Some(1), None][part]);
            self.hovered_part = over;
        }
        let part = match self.touched_part.filter(|_| pressed) {
            Some(part) => part,
            None => over?,
        };
        self.touched_part = pressed.then_some(part);
        let place = if part == 0 { layout.bar } else { parts[part]? };
        let [left, top, width, height] = place.to;
        let [from_x, from_y, from_width, from_height] = place.from;
        let bottom_x = from_x + (x - left) * from_width / width;
        let bottom_y = if part == 0 { DIAL_GRAB } else { from_y + (y - top) * from_height / height };
        Some((bottom_x.clamp(0.0, 319.0), bottom_y.clamp(0.0, 239.0)))
    }

    /// whether the Fiend's Cauldron's pot pours, after Move Out, the bar
    /// gone and the pan lifted, frame as for pause_menu_shown.
    pub fn cauldron_pouring(&self, frame: u64) -> bool {
        self.cauldron_shown(frame) && self.cauldron_pouring
    }

    /// uses the current power, as touching its box does, in the battle UI's
    /// next update, frame as for pause_menu_shown. the game checks it can be
    /// used, as for a touch.
    pub fn use_power(&mut self, frame: u64) {
        self.power_requested = Some(frame);
    }

    /// the bottom screen pixel a touch of the top screen's pixel x, y
    /// touches while the pause menu is shown there, frame as for
    /// pause_menu_shown. none off the bottom screen, or beside Resume,
    /// Retreat or Remove Display by the top screen's edges, where the
    /// bottom screen has them but the top screen does not.
    pub fn pause_menu_touch(&self, frame: u64, x: f32, y: f32) -> Option<(f32, f32)> {
        let edges = self.edges_drawn.is_some_and(|drawn| frame <= drawn + 3);
        let right = 2.0 * PAUSE_MENU_OFFSET;
        // where it would be with the top screen its own shape, by the left
        // edge, the right one or the middle, see place_pause_menu
        let k = self.widen;
        let (by_left, by_right, middle) = (x * k, 400.0 - (400.0 - x) * k, 200.0 + (x - 200.0) * k);
        let x = if edges && PAUSE_BUTTON_ROWS.contains(&y) {
            if by_left < RESUME_RIGHT {
                by_left
            } else if by_right >= RETREAT_LEFT + right {
                by_right - right
            } else {
                return None;
            }
        } else if edges && y >= REMOVE_DISPLAY_CORNER[1] && by_right >= REMOVE_DISPLAY_CORNER[0] + PAUSE_MENU_OFFSET {
            if by_right < REMOVE_DISPLAY_CORNER[0] + right {
                return None;
            }
            by_right - right
        } else {
            middle - PAUSE_MENU_OFFSET
        };
        ((0.0..320.0).contains(&x) && (0.0..240.0).contains(&y)).then_some((x, y))
    }
}

/// does what the instruction about to run calls for: the mouse's movement
/// where the game has just summed the aim delta, the camera box on the
/// ground, the HUD's and the reticle's size where they are drawn.
pub(crate) fn apply(system: &mut System, pc: u32) {
    menu_top::physics(system, pc);
    match pc {
        AFTER_AIM_DELTA => add_delta(system),
        BEFORE_CAMERA_TURN => close_box(system),
        PROJECTION => widen_view(system),
        PASS_BEGIN => {
            let (context, screen) = (system.cpu.regs[0], system.cpu.regs[1]);
            if let Some(aim_mouse) = &mut system.aim_mouse {
                if screen == 1 && aim_mouse.context == Some(context) {
                    aim_mouse.recorded.fill(None);
                }
                match aim_mouse.screens.iter_mut().find(|(seen, _)| *seen == context) {
                    Some(seen) => seen.1 = screen,
                    None => aim_mouse.screens.push((context, screen)),
                }
            }
        }
        SPRITE_FLUSH => {
            if system.cpu.regs[14] == SCORE_FLUSH_CALLER {
                let context = system.cpu.regs[0];
                if let Some(aim_mouse) = &mut system.aim_mouse {
                    aim_mouse.context = Some(context);
                }
            }
            shrink_hud(system);
        }
        RETICLE_DRAW => shrink_reticle(system),
        TOP_UI_AFTER_HUD => top_screen_extras(system),
        BOTTOM_BATTLE_UI => {
            if drawing(system).is_some() { return; }
            // A skipped call must not survive a fade, pause or scene change.
            if let Some(aim_mouse) = &mut system.aim_mouse {
                aim_mouse.recorded.fill(None);
                aim_mouse.context = Some(system.cpu.regs[1]);
            }
        }
        UI_COMMANDS => power_command(system),
        PAUSE_KEY => pause_key(system),
        PAUSE_TOUCH => hover_pause(system),
        PAUSE_INPUT_DONE => hovered_pause(system),
        PAUSE_TOUCHES | OPTIONS_TOUCHES | PAUSE_OPTION_FOUND | OPTIONS_OPTION_FOUND => {
            if let Some(hovering) = system.aim_mouse.as_mut().and_then(|aim_mouse| aim_mouse.hovering.as_mut()) {
                hovering.looked = true;
                hovering.found |= matches!(pc, PAUSE_OPTION_FOUND | OPTIONS_OPTION_FOUND);
            }
        }
        DIALOG_INPUT => hover_dialog(system),
        TOP_SCENE_3D => top_scene_viewport(system),
        menu_top::START => { menu_top::begin(system); }
        menu_top::FINISHED => { menu_top::draw(system); }
        CAULDRON_CURSOR_KEYS_DONE => light_cauldron_cursor(system),
        TOP_TEXT | TOP_TEXT_DRAWN => {
            if !menu_top::text(system, pc) { top_text_viewport(system, pc); }
        }
        menu_top::SCISSOR => menu_top::scissor(system),
        menu_top::SCISSOR_DONE => menu_top::scissor_done(system),
        menu_top::BOTTOM_TEXT => menu_top::bottom_text(system),
        menu_top::BOTTOM_TRANSLUCENT => menu_top::bottom_translucent(system),
        _ if menu_top::FONT_CALL.contains(&pc) => menu_top::font(system),
        _ if menu_top::FONT_DONE.contains(&pc) => menu_top::font_done(system),
        TOP_SCENE_DRAWN => top_screen_cauldron(system),
        BOTTOM_SCENE_DRAW => {
            if !matches!(drawing(system), Some(Drawing::Cauldron(_) | Drawing::MenuTop(_))) {
                let scene = system.cpu.regs[0];
                if let Some(aim_mouse) = &mut system.aim_mouse {
                    aim_mouse.bottom_scene = Some(scene);
                }
            }
        }
        BOTTOM_SCENE_3D_DRAWN => cauldron_drawn(system),
        MENU_NO_TOUCH => {
            let (frame, touch) = (system.frames, system.cpu.regs[0]);
            if read_word(system, touch) == MENU_TOUCH {
                if let Some(aim_mouse) = &mut system.aim_mouse {
                    aim_mouse.menu_touch = Some((touch, frame));
                }
            }
        }
        _ if MENU_SCREENS.iter().any(|screen| screen.update == pc) => outline_menu(system, pc),
        _ if CAULDRON.contains(&pc) => {
            let (frame, task) = (system.frames, system.cpu.regs[0]);
            if let Some(aim_mouse) = &mut system.aim_mouse {
                aim_mouse.cauldron_drawn = Some(frame);
                aim_mouse.cauldron_task = Some(task);
                aim_mouse.cauldron_pouring = pc == CAULDRON[1];
            }
        }
        DIALOG_CURSOR_SOUND => {
            if system.aim_mouse.as_mut().is_some_and(|aim_mouse| std::mem::take(&mut aim_mouse.dialog_cleared)) {
                system.cpu.regs[5] = 0;
            }
        }
        _ if PAUSE_MENU.iter().any(|to_top| to_top.site == pc || to_top.place == pc) => {
            if let Some(index) = PAUSE_MENU.iter().position(|to_top| to_top.site == pc) {
                let mut recorded = recorded_call(system, pc);
                if let Some(recorded) = recorded.as_mut().filter(|_| PAUSE_MENU[index].projection) {
                    let mut bytes = [0u8; 0x70];
                    system.memory.read_bytes(system.cpu.regs[1], &mut bytes[..0x40]);
                    system.memory.read_bytes(system.cpu.regs[2], &mut bytes[0x40..]);
                    let words = bytes.as_chunks::<4>().0.iter().map(|word| u32::from_le_bytes(*word));
                    let mut matrices = [0u32; 28];
                    matrices.iter_mut().zip(words).for_each(|(matrix, word)| *matrix = word);
                    recorded.matrices = Some(matrices);
                }
                if let Some(aim_mouse) = &mut system.aim_mouse {
                    for (kept, to_top) in aim_mouse.pause.iter_mut().zip(&PAUSE_MENU) {
                        if to_top.site == pc {
                            *kept = recorded;
                        }
                    }
                }
            }
            if PAUSE_MENU.iter().any(|to_top| to_top.place == pc) {
                top_screen_pause_menu(system, pc);
            }
        }
        _ if MIRRORED.iter().any(|mirrored| mirrored.site == pc) => record(system, pc),
        _ => {}
    }
}

fn read_word(system: &mut System, at: u32) -> u32 {
    let mut word = [0u8; 4];
    system.memory.read_bytes(at, &mut word);
    u32::from_le_bytes(word)
}

/// makes the view widen times wider, before a camera's projection is
/// taken: its perspective lens gets the aspect the game gave it times
/// widen, and the projection is built again whenever that changes.
fn widen_view(system: &mut System) {
    let camera = system.cpu.regs[0];
    let lens = read_word(system, camera + 4);
    if lens == 0 || read_word(system, lens) != PERSPECTIVE_LENS {
        return;
    }
    let aspect = f32::from_bits(read_word(system, lens + LENS_ASPECT));
    let fovy = f32::from_bits(read_word(system, lens + LENS_FOVY));
    let Some(aim_mouse) = &mut system.aim_mouse else { return };
    // an aspect other than the one written last is the game's own
    let kept = aim_mouse.lenses.iter().position(|&(at, ..)| at == lens);
    let given = match kept.map(|index| aim_mouse.lenses[index]) {
        Some((_, given, written)) if written == aspect => given,
        _ => aspect,
    };
    if !(given.is_finite() && given > 0.0) {
        return;
    }
    let wanted = given * aim_mouse.widen;
    let half_width = (fovy * 0.5).tan() * given;
    if half_width.is_finite() && half_width > 0.0 {
        aim_mouse.half_width = half_width;
    }
    match kept {
        Some(index) => aim_mouse.lenses[index] = (lens, given, wanted),
        None => aim_mouse.lenses.push((lens, given, wanted)),
    }
    if wanted != aspect {
        system.memory.write_bytes(lens + LENS_ASPECT, &wanted.to_le_bytes());
        let flags = read_word(system, camera + 8) & !4;
        system.memory.write_bytes(camera + 8, &flags.to_le_bytes());
    }
}

/// how many times narrower the sprite batch about to be flushed is drawn,
/// so it shows the right shape on a top screen widen times wider: widen for
/// a context drawing the top screen, 1 for the bottom or one not seen
/// starting a pass.
fn widening(system: &System) -> f32 {
    let Some(aim_mouse) = &system.aim_mouse else { return 1.0 };
    let batch = system.cpu.regs[0];
    match aim_mouse.screens.iter().find(|&&(context, _)| context == batch) {
        Some(&(_, 0)) => aim_mouse.widen,
        _ => 1.0,
    }
}

/// on the ground, shrinks the box to its middle point, so that any movement
/// of the reticle is past it and turns the camera.
fn close_box(system: &mut System) {
    if !system.aim_mouse.as_ref().is_some_and(|aim_mouse| aim_mouse.captured) {
        return;
    }
    let aim = system.cpu.regs[4];
    let mut flags = [0u8; 0xD6];
    system.memory.read_bytes(aim, &mut flags);
    // +0x81 is set on the ground, +0xD5 for anyone but the local player
    if flags[0x81] == 0 || flags[0xD5] != 0 {
        return;
    }
    let regs = &mut system.cpu.vfp.regs;
    let x = (f32::from_bits(regs[0]) + f32::from_bits(regs[1])) * 0.5;
    let y = (f32::from_bits(regs[2]) + f32::from_bits(regs[3])) * 0.5;
    regs[0] = x.to_bits();
    regs[1] = x.to_bits();
    regs[2] = y.to_bits();
    regs[3] = y.to_bits();
}

/// adds what the mouse moved to the local player's aim delta, divided by
/// the time the step covers since the delta is a speed.
fn add_delta(system: &mut System) {
    let Some(aim_mouse) = &mut system.aim_mouse else { return };
    let [dx, dy] = std::mem::take(&mut aim_mouse.pending);
    let (widen, half_width) = (aim_mouse.widen, aim_mouse.half_width);
    let aim = system.cpu.regs[4];
    let mut remote = [0u8];
    system.memory.read_bytes(aim + 0xD5, &mut remote);
    // on a wider top screen the mouse moves as far: in the air the reticle
    // as far across the window, on the ground the camera by as many degrees,
    // where the game turns by the reticle's share of the view's width
    let mut ground = [0u8];
    system.memory.read_bytes(aim + 0x81, &mut ground);
    let dx = match (widen == 1.0, ground[0] != 0) {
        (true, _) => dx,
        (false, true) => dx * half_width.atan() / (half_width * widen).atan(),
        (false, false) => dx / widen,
    };
    let dt = f32::from_bits(system.cpu.vfp.regs[16]);
    if remote[0] != 0 || dt <= 0.0 {
        return;
    }
    let out = system.cpu.regs[13] + 8;
    let mut delta = [0u8; 8];
    system.memory.read_bytes(out, &mut delta);
    let x = f32::from_le_bytes(delta[0..4].try_into().unwrap()) + dx / dt;
    let y = f32::from_le_bytes(delta[4..8].try_into().unwrap()) + dy / dt;
    delta[0..4].copy_from_slice(&x.to_le_bytes());
    delta[4..8].copy_from_slice(&y.to_le_bytes());
    system.memory.write_bytes(out, &delta);
}

/// asks the battle UI to use the current power, when the power key was
/// pressed, with the word its update, 0x00A17A10, writes for a touch of the
/// middle box, and only when a touch would ask for it.
fn power_command(system: &mut System) {
    let Some(aim_mouse) = &mut system.aim_mouse else { return };
    let requested = aim_mouse.power_requested.take().is_some_and(|pressed| system.frames <= pressed + 3);
    if !requested || system.cpu.regs[1] != 0 {
        return;
    }
    let object = system.cpu.regs[0];
    let panel = read_word(system, object + 0x140) + POWER_PANEL;
    let (locked, busy) = (read_word(system, panel), read_word(system, panel + 4));
    let (current, count) = (read_word(system, panel + CURRENT_POWER), read_word(system, panel + POWER_COUNT));
    if locked & POWERS_LOCKED != 0 || busy & POWER_PANEL_BUSY != 0 || current >= count {
        return;
    }
    let mut entry = [0u8];
    system.memory.read_bytes(panel + POWER_LIST + current, &mut entry);
    let mut power = [0u8; 6];
    system.memory.read_bytes(panel + entry[0] as u32 * 8 + 0x11C, &mut power);
    let (uses, id, unusable) = (power[0] as i8, u16::from_le_bytes([power[2], power[3]]), power[5]);
    if uses <= 0 || unusable != 0 {
        return;
    }
    let kept = read_word(system, panel + POWER_COMMAND) & POWER_COMMAND_KEPT;
    let command = kept | (id as u32) << 23 | (current & 0x3F) << 17 | 1 << 15 | 1;
    system.memory.write_bytes(panel + POWER_COMMAND, &command.to_le_bytes());
}

/// a key pressed on the pause screen gives the keys the menu back, and
/// lights the option a hover took the light off, using the key up if it
/// would move or use the option. the screen opening does as well, before
/// the mouse moves over it.
fn pause_key(system: &mut System) {
    let pause = system.cpu.regs[9];
    let key = system.cpu.regs[5] as i32;
    let cursor = read_word(system, pause + PAUSE_CURSOR) as i32;
    let frame = system.frames;
    let Some(aim_mouse) = &mut system.aim_mouse else { return };
    if !aim_mouse.paused.is_some_and(|paused| frame <= paused + 3) {
        aim_mouse.mouse_mode = false;
        aim_mouse.cleared = None;
    }
    aim_mouse.paused = Some(frame);
    if key == -1 {
        return;
    }
    aim_mouse.mouse_mode = false;
    let Some(option) = aim_mouse.cleared.take() else { return };
    if cursor == -1 {
        system.memory.write_bytes(pause + PAUSE_CURSOR, &option.to_le_bytes());
        if CURSOR_KEYS.contains(&key) {
            system.cpu.regs[5] = u32::MAX;
        }
    }
}

/// gives the pause screen's main page or a page of Options the mouse over
/// it as a touch that is held there, not one that began now, which lights
/// the option under it without using it. off the menu, nothing is lit. a
/// real touch, a key last, or the page of Options whose touches drag what
/// they hold, it gets as it is, and a slider is not set: Volume's row is
/// lit from left of it, Reticle/Camera's are as off the menu.
fn hover_pause(system: &mut System) {
    let pause = system.cpu.regs[9];
    let (state, page, next) = (read_word(system, pause + PAUSE_STATE), read_word(system, pause + OPTIONS_PAGE), read_word(system, pause + OPTIONS_PAGE_NEXT));
    let Some(aim_mouse) = &mut system.aim_mouse else { return };
    let options = state == 7 && page == next;
    if !aim_mouse.menu_hover || !aim_mouse.mouse_mode || system.cpu.regs[8] != 0 || !(state == 2 || options && page != OPTIONS_DRAG_PAGE) {
        return;
    }
    let pointer = aim_mouse.pointer.map(|(x, y)| (x as i32, y as i32));
    let pointer = match pointer {
        Some((_, y)) if options && page == CAMERA_PAGE && y < CAMERA_SLIDERS_BELOW => None,
        Some((x, y)) if options && page == VOLUME_PAGE && VOLUME_SLIDERS.contains(&x) => Some((VOLUME_SLIDERS.start() - 1, y)),
        pointer => pointer,
    };
    let Some((x, y)) = pointer else {
        clear_pause_cursor(system, pause);
        return;
    };
    let regs = &mut system.cpu.regs;
    let touch = [regs[6], regs[7], regs[8], system.cpu.vfp.regs[16]];
    aim_mouse.hovering = Some(Hovering { touch, looked: false, found: false });
    regs[6] = 0;
    regs[7] = x as u32;
    regs[8] = 1;
    system.cpu.vfp.regs[16] = y as u32;
}

/// after the pause screen took a hover: the real touch back for what comes
/// after, and the light off if the page looked for an option under the
/// mouse and found none.
fn hovered_pause(system: &mut System) {
    let Some(hovering) = system.aim_mouse.as_mut().and_then(|aim_mouse| aim_mouse.hovering.take()) else { return };
    let [r6, r7, r8, s16] = hovering.touch;
    (system.cpu.regs[6], system.cpu.regs[7], system.cpu.regs[8]) = (r6, r7, r8);
    system.cpu.vfp.regs[16] = s16;
    if hovering.looked && !hovering.found {
        clear_pause_cursor(system, system.cpu.regs[9]);
    }
}

/// lights none of the pause screen's options, keeping the one lit for a key.
fn clear_pause_cursor(system: &mut System, pause: u32) {
    let cursor = read_word(system, pause + PAUSE_CURSOR);
    if cursor as i32 == -1 {
        return;
    }
    system.memory.write_bytes(pause + PAUSE_CURSOR, &u32::MAX.to_le_bytes());
    if let Some(aim_mouse) = &mut system.aim_mouse {
        aim_mouse.cleared = Some(cursor);
    }
}

/// lights the dialog's button under the mouse, none off them, as a key
/// moving to it would, but for the sound when it is none. a touch that began
/// now, or a key, it gets as it is, and a key gives the keys the dialog back.
fn hover_dialog(system: &mut System) {
    let (dialog, sp) = (system.cpu.regs[4], system.cpu.regs[13]);
    let mut input = [0u8; 0xB];
    system.memory.read_bytes(sp, &mut input);
    let (began, key) = (input[9] != 0, input[10] as i8);
    let count = (read_word(system, dialog + DIALOG_BUTTON_COUNT) as i32).clamp(0, 8) as u32;
    let mut boxes = [0u8; 8 * 8];
    system.memory.read_bytes(dialog + DIALOG_BUTTONS, &mut boxes[..count as usize * 8]);
    let cursor = read_word(system, dialog + DIALOG_CURSOR) as i32;
    let Some(aim_mouse) = &mut system.aim_mouse else { return };
    aim_mouse.dialog_cleared = false;
    if key != -1 {
        aim_mouse.mouse_mode = false;
    }
    if !aim_mouse.menu_hover || !aim_mouse.mouse_mode || began || count == 0 {
        return;
    }
    let under = aim_mouse.pointer.and_then(|(x, y)| {
        let (x, y) = (x as i32, y as i32);
        boxes.as_chunks::<8>().0[..count as usize].iter().position(|button| {
            let edge = |at: usize| i16::from_le_bytes([button[at], button[at + 1]]) as i32;
            (edge(0)..=edge(2)).contains(&x) && (edge(4)..=edge(6)).contains(&y)
        })
    });
    let option = under.map_or(-1, |under| under as i32);
    if option != cursor {
        aim_mouse.dialog_cleared = option == -1;
        system.memory.write_bytes(dialog + DIALOG_CURSOR, &option.to_le_bytes());
    }
}

/// draws the portraits and then the narration's line on the top screen as
/// well: the game's own drawing of them, each called here as if the pass
/// called it. back here after each, everything is put back, and once both
/// are drawn the pass goes on.
fn top_screen_extras(system: &mut System) {
    let Some(aim_mouse) = &mut system.aim_mouse else { return };
    aim_mouse.battle_drawn = Some(system.frames);
    if aim_mouse.saved.is_none() {
        aim_mouse.extras_pass = aim_mouse.extras_pass.wrapping_add(1);
        log::trace!("battle extras pass {} frame {} context {:08X}, recordings {:?}", aim_mouse.extras_pass, system.frames, system.cpu.regs[4], aim_mouse.recorded.map(|kept| kept.map(|kept| (kept.pass, kept.call.regs[1]))));
        // This pass keeps its own UI context in r4, even without a score.
        // Do not reuse a context from another scene or a loaded state.
        aim_mouse.context = Some(system.cpu.regs[4]);
    }
    if let Some(back) = aim_mouse.saved.take() {
        system.cpu.regs = back.regs;
        system.cpu.cpsr = back.cpsr;
        system.cpu.vfp.regs = back.vfp;
        put_back(system, &back.moved);
        if matches!(back.drawing, Drawing::Mirrored(_)) {
            if let Some(context) = context(system) { dirty_material(system, context); }
        }
        let next = match back.drawing {
            Drawing::Portraits => 0,
            Drawing::Mirrored(index) => index + 1,
            Drawing::Narration | Drawing::PauseMenu(_) | Drawing::Cauldron(_) | Drawing::MenuTop(_) | Drawing::MenuClip(_) | Drawing::CauldronCursor(_) | Drawing::Viewport(_) | Drawing::Menu(..) => return,
        };
        if !start_mirrored(system, next) {
            start_narration(system);
        }
        return;
    }
    if !start_portraits(system) && !start_mirrored(system, 0) {
        start_narration(system);
    }
}

/// keeps what the bottom pass gives a mirrored element's call, for the top
/// pass to make it again.
fn record(system: &mut System, site: u32) {
    if drawing(system).is_some() { return; }
    let Some(index) = MIRRORED.iter().position(|mirrored| mirrored.site == site) else { return };
    let Some(recorded) = recorded_call(system, site) else { return };
    let bounds = if index == ILLUSTRATION { illustration_bounds(system, recorded.regs[0]) } else { None };
    let material = [0, 1, 2, 3].map(|word| read_word(system, recorded.regs[1] + 0xEC + word * 4));
    if let Some(aim_mouse) = &mut system.aim_mouse {
        aim_mouse.recorded[index] = Some(MirroredCall { call: recorded, pass: aim_mouse.extras_pass, bounds, material });
    }
}

/// Up to four illustration slots: image id/flags, x/y, UV offset, width/
/// height, scale and alpha. Fit their union, rather than the 320-pixel band,
/// so a narrow picture such as the blue Hewdraw stays legible.
fn illustration_bounds(system: &mut System, ui: u32) -> Option<UiBox> {
    let mask = read_word(system, ui + 0x200);
    let mut bounds = UiBox { min: [f32::MAX; 2], max: [f32::MIN; 2] };
    for slot in 0..4 {
        if mask & (1 << slot) == 0 { continue; }
        let at = ui + 0x204 + slot * 0x24;
        let x = f32::from_bits(read_word(system, at + 4));
        let y = f32::from_bits(read_word(system, at + 8));
        let scale = f32::from_bits(read_word(system, at + 0x1C));
        let width = f32::from_bits(read_word(system, at + 0x14)) * scale;
        let height = f32::from_bits(read_word(system, at + 0x18)) * scale;
        let image = UiBox { min: [x, y], max: [x + width, y + height] };
        if !image.valid() { return None; }
        for axis in 0..2 {
            bounds.min[axis] = bounds.min[axis].min(image.min[axis]);
            bounds.max[axis] = bounds.max[axis].max(image.max[axis]);
        }
    }
    bounds.valid().then_some(bounds)
}

/// the call the bl at site is about to make, and what it is given.
fn recorded_call(system: &mut System, site: u32) -> Option<Recorded> {
    let mut word = [0u8; 4];
    system.memory.read_bytes(site, &mut word);
    let word = u32::from_le_bytes(word);
    // a bl: its target is 8 past it and the offset in words. one whose
    // condition fails makes no call, what the registers hold is not for it
    if word >> 24 & 0x0F != 0x0B || !zakuro_cpu::psr::check_condition(word >> 28, &system.cpu.cpsr) {
        return None;
    }
    let offset = ((word << 8) as i32 >> 6) as u32;
    let target = site.wrapping_add(8).wrapping_add(offset);
    let cpu = &system.cpu;
    Some(Recorded {
        target,
        regs: [cpu.regs[0], cpu.regs[1], cpu.regs[2], cpu.regs[3]],
        floats: cpu.vfp.regs[..8].try_into().unwrap(),
        matrices: None,
    })
}

/// draws the pause menu, or a dialog over it, on the top screen as well,
/// each call the last bottom pass made that comes here made again in turn,
/// everything put back after each.
fn top_screen_pause_menu(system: &mut System, place: u32) {
    let Some(aim_mouse) = &mut system.aim_mouse else { return };
    let mut next = 0;
    if let Some(back) = aim_mouse.saved.take() {
        system.cpu.regs = back.regs;
        system.cpu.cpsr = back.cpsr;
        system.cpu.vfp.regs = back.vfp;
        let Drawing::PauseMenu(index) = back.drawing else { return };
        next = index + 1;
    }
    if !aim_mouse.pause_on_top {
        return;
    }
    let Some(index) = (next..PAUSE_MENU.len()).find(|&index| PAUSE_MENU[index].place == place && aim_mouse.pause[index].is_some()) else {
        return;
    };
    let Some(recorded) = aim_mouse.pause[index].take() else { return };
    aim_mouse.pause_drawn = Some(system.frames);
    call_here(system, recorded.target, recorded.regs[0], recorded.regs[1], Drawing::PauseMenu(index), Vec::new());
    system.cpu.regs[2] = recorded.regs[2];
    system.cpu.regs[3] = recorded.regs[3];
    system.cpu.vfp.regs[..8].copy_from_slice(&recorded.floats);
    if let Some(mut matrices) = recorded.matrices {
        // the projection's second row turns the bottom screen's 320 pixels
        // into its width, the top's middle 320 of 400 is that much less
        // and narrower about the middle on a wider one
        let widen = system.aim_mouse.as_ref().map_or(1.0, |aim_mouse| aim_mouse.widen);
        for word in &mut matrices[4..8] {
            *word = (f32::from_bits(*word) * 320.0 / 400.0 / widen).to_bits();
        }
        // below the stack the call starts with, put back after it
        let at = system.cpu.regs[13] - 0x70;
        let bytes: Vec<u8> = matrices.iter().flat_map(|word| word.to_le_bytes()).collect();
        system.memory.write_bytes(at, &bytes);
        system.cpu.regs[1] = at;
        system.cpu.regs[2] = at + 0x40;
        system.cpu.regs[13] = at;
    }
}

/// draws the Fiend's Cauldron's bottom screen on the top screen as well,
/// over the top's 3D and under its 2D: the bottom's scene drawing the last
/// bottom pass made, called here for its bar, the bar's dial, Back and then
/// the rest, each in its place, the background left off, and stopped once its layers are
/// drawn (see cauldron_drawn). back here after each, everything is put
/// back, as it is for the top's picture first.
fn top_screen_cauldron(system: &mut System) {
    let top = system.cpu.regs[4];
    let Some(aim_mouse) = &mut system.aim_mouse else { return };
    let next = match aim_mouse.saved.take_if(|saved| matches!(saved.drawing, Drawing::Cauldron(_))) {
        Some(back) => {
            system.cpu.regs = back.regs;
            system.cpu.cpsr = back.cpsr;
            system.cpu.vfp.regs = back.vfp;
            let Drawing::Cauldron(part) = back.drawing else { return };
            if part == 0 {
                // the panel's viewport and camera are set: its 3D, with
                // the same changes
                let scene = read_word(system, top + 0x40);
                call_here(system, TOP_SCENE_3D_DRAW, top, scene, Drawing::Cauldron(1), back.moved);
                return;
            }
            put_back(system, &back.moved);
            part + 1
        }
        None => {
            let top_moved = std::mem::take(&mut aim_mouse.top_moved);
            let top_projection = aim_mouse.top_projection.take();
            if let Some((viewport, _)) = aim_mouse.top_viewport.take() {
                system.memory.write_bytes(top + 0x44, &viewport);
            }
            put_back(system, &top_moved);
            if let Some((at, projection, _)) = top_projection {
                system.memory.write_bytes(at, &projection);
            }
            0
        }
    };
    let Some((layout, bottom)) = cauldron_scenes(system, top) else { return };
    if next == 0 {
        // the panel, its part of the top screen around where it has slid
        let mut panel = layout.panel;
        let Some(value) = cauldron_value(system) else { return };
        panel.from[0] = PANEL_ZERO + value * PANEL_STEP - panel.from[2] * 0.5;
        let [bar_x, _, bar_width, _] = layout.bar.to;
        panel.to[0] = bar_x + (DIAL_ZERO + value * DIAL_STEP - layout.bar.from[0]) * bar_width / layout.bar.from[2] - panel.to[2] * 0.5;
        top_panel(system, top, panel);
        return;
    }
    // each place, and the group of CAULDRON_PARTS it has
    let dial = dial_place(system, layout);
    if let Some(aim_mouse) = &mut system.aim_mouse {
        aim_mouse.cauldron_dial = Some(dial);
    }
    let Some(&(place, group)) = [(layout.bar, 0), (dial, 0), (layout.back, 1), (layout.move_out, 2), (layout.rest, 3)].get(next - 2) else {
        return;
    };
    let scene = read_word(system, top + 0x40);
    let viewport = viewport_bytes(system, top, place.to);
    let mut moved = Vec::new();
    change(system, &mut moved, bottom + 0x44, &viewport);
    // no dialog over it, the bottom pass draws those. it sets the scene's
    // camera and screen and its renderer's +0x58, which the top's 2D may
    // look at after
    change(system, &mut moved, bottom + 0x60, &[0; 4]);
    let renderer = read_word(system, scene + 4);
    for (at, len) in [(scene + 0xF4, 8), (renderer + 0x58, 4)] {
        let mut bytes = vec![0u8; len];
        system.memory.read_bytes(at, &mut bytes);
        moved.push((at, bytes));
    }
    // the part of the bottom screen shown
    let screen = read_word(system, bottom + 0x58);
    let camera = read_word(system, scene + SCENE_CAMERAS + screen * 4);
    if let Some((matrix, part)) = cropped_projection(system, camera, place.from, [320.0, 240.0], false) {
        moved.push((camera + 0x6C, matrix.to_vec()));
        system.memory.write_bytes(camera + 0x6C, &part);
    }
    let layers = read_word(system, scene + SCENE_LAYERS);
    for layer in BOTTOM_LAYERS {
        let list = read_word(system, layers + layer * LAYER_SIZE + 4);
        if list == 0 {
            continue;
        }
        let (start, end) = (read_word(system, list + 0x18), read_word(system, list + 0x1C));
        if end < start || end - start > 0x400 {
            continue;
        }
        let models: Vec<u32> = (start..end).step_by(4).map(|at| read_word(system, at)).collect();
        let kept: Vec<u32> = models
            .iter()
            .copied()
            .filter(|&model| {
                let name = model_name(system, model);
                let part = CAULDRON_PARTS.iter().position(|part| part.contains(&name.as_str())).unwrap_or(CAULDRON_PARTS.len());
                part == group && !CAULDRON_LEFT_OFF.contains(&name.as_str())
            })
            .collect();
        if kept.len() < models.len() {
            let bytes: Vec<u8> = kept.iter().flat_map(|model| model.to_le_bytes()).collect();
            change(system, &mut moved, start, &bytes);
            change(system, &mut moved, list + 0x1C, &(start + bytes.len() as u32).to_le_bytes());
        }
    }
    call_here(system, BOTTOM_SCENE_DRAW, bottom, 0, Drawing::Cauldron(next), moved);
}

/// once the Fiend's Cauldron's cursor's update, r4, has handled the keys,
/// lights what the mouse went over, as an arrow key does there: the game's
/// own setting of it, with the name below the stack, then its moving of
/// the light, back here after each.
fn light_cauldron_cursor(system: &mut System) {
    let frame = system.frames;
    let Some(aim_mouse) = &mut system.aim_mouse else { return };
    if let Some(back) = aim_mouse.saved.take_if(|saved| matches!(saved.drawing, Drawing::CauldronCursor(_))) {
        system.cpu.regs = back.regs;
        system.cpu.cpsr = back.cpsr;
        system.cpu.vfp.regs = back.vfp;
        if back.drawing == Drawing::CauldronCursor(0) {
            let cursor = system.cpu.regs[4];
            let flags = read_word(system, cursor + CAULDRON_CURSOR_FLAGS) as u8 | 0x10;
            system.memory.write_bytes(cursor + CAULDRON_CURSOR_FLAGS, &[flags]);
            call_here(system, CAULDRON_CURSOR_MOVED, cursor, 1, Drawing::CauldronCursor(1), Vec::new());
        }
        return;
    }
    if !aim_mouse.cauldron_on_top_shown(frame) || aim_mouse.saved.is_some() {
        return;
    }
    let Some(wanted) = aim_mouse.cursor_wanted.take() else { return };
    let cursor = system.cpu.regs[4];
    if read_word(system, cursor) != CAULDRON_CURSOR {
        return;
    }
    let (name, hash) = CAULDRON_CURSOR_NAMES[wanted];
    if read_word(system, cursor + CAULDRON_CURSOR_LIT) == hash {
        return;
    }
    call_here(system, SET_CAULDRON_CURSOR, cursor, 0, Drawing::CauldronCursor(0), Vec::new());
    let at = (system.cpu.regs[13] - 0x40) & !7;
    let mut record = [0u8; 0x2C];
    record[..4].copy_from_slice(&hash.to_le_bytes());
    record[4..8].copy_from_slice(&(at + 0xC).to_le_bytes());
    record[8..12].copy_from_slice(&0x20u32.to_le_bytes());
    record[12..12 + name.len()].copy_from_slice(name.as_bytes());
    system.memory.write_bytes(at, &record);
    system.cpu.regs[1] = at;
    system.cpu.regs[2] = 0;
    system.cpu.regs[3] = 0;
    system.cpu.regs[13] = at;
}

/// before a menu of MENU_SCREENS updates, the object in r0, outlines what
/// is under the mouse when it moved there, by the menu's own code as its
/// keys do, and after a click uses it as A does. the game finds what is
/// under the mouse (MENU_HIT) and its part (PART_LOOKUP), then the menu's
/// calls that outline it are made, and after a click its A. each call
/// comes back here, the CPU put back between them. off its parts what is
/// outlined stays, and keys move it from there.
fn outline_menu(system: &mut System, pc: u32) {
    let Some(index) = MENU_SCREENS.iter().position(|screen| screen.update == pc) else { return };
    let screen = &MENU_SCREENS[index];
    let frame = system.frames;
    let back = system.aim_mouse.as_mut().and_then(|aim_mouse| aim_mouse.saved.take_if(|saved| matches!(saved.drawing, Drawing::Menu(..))));
    if let Some(back) = back {
        let returned = system.cpu.regs[0];
        system.cpu.regs = back.regs;
        system.cpu.cpsr = back.cpsr;
        system.cpu.vfp.regs = back.vfp;
        let Drawing::Menu(step, _) = back.drawing else { return };
        let object = system.cpu.regs[0];
        let at = (system.cpu.regs[13] - 0x40) & !7;
        match step {
            // what is under the mouse: a layer, the element's index, and
            // the element
            0 => {
                let (layer, element) = (read_word(system, at), read_word(system, at + 4));
                let part = if layer != 0 && element != u32::MAX { read_word(system, layer + LAYER_PART) } else { 0 };
                if part != 0 {
                    system.memory.write_bytes(at + 0x18, &PART_KEY.to_le_bytes());
                    call_here(system, PART_LOOKUP, part, at + 0x18, Drawing::Menu(1, index), Vec::new());
                    system.cpu.regs[13] = at;
                    return;
                }
                if !menu_top::accept_hit(system, None) { outline_menu(system, pc); return; }
            }
            // its part's descriptor, and the menu's calls to outline it
            1 => {
                let descriptor = if returned != 0 { read_word(system, returned + PART_DESCRIPTOR) } else { 0 };
                if descriptor != 0 {
                    let layer = read_word(system, at);
                    let found = Found { hash: read_word(system, descriptor + DESCRIPTOR_NAME), name: descriptor + DESCRIPTOR_NAME, element: read_word(system, at + 0xC), part: read_word(system, layer + LAYER_PART) };
                    if !menu_top::accept_hit(system, Some(found.hash)) { outline_menu(system, pc); return; }
                    if let Some(calls) = (screen.outline)(system, object, found) {
                        if let Some(aim_mouse) = &mut system.aim_mouse {
                            aim_mouse.menu_part = Some(found);
                            aim_mouse.menu_calls = calls;
                        }
                    }
                }
            }
            _ => {}
        }
        // Queued menus apply the new outline in their update. Let that
        // run before A, especially when keys changed the hovered choice.
        if step == 2 && system.aim_mouse.as_ref().is_some_and(|mouse| mouse.menu_calls.is_empty() && mouse.menu_click) {
            return;
        }
        next_menu_call(system, index, object, at);
        return;
    }
    let object = system.cpu.regs[0];
    if read_word(system, object) != screen.vtable {
        return;
    }
    let Some(aim_mouse) = &mut system.aim_mouse else { return };
    let age = aim_mouse.menu_age();
    if aim_mouse.pointer.is_some() {
        log::debug!("menu update {index} frame {frame}, previous {:?}, touch {:?}, pointer {:?}, age {age}, looked {:?}, saved {}", aim_mouse.menu_screen, aim_mouse.menu_touch, aim_mouse.pointer, aim_mouse.menu_looked, aim_mouse.saved.is_some());
    }
    aim_mouse.menus_open.retain(|&(_, _, last)| frame <= last + age);
    match aim_mouse.menus_open.iter_mut().find(|(open, _, _)| *open == object) {
        Some(open) => open.2 = frame,
        None => aim_mouse.menus_open.push((object, frame, frame)),
    }
    let newest = aim_mouse.menus_open.iter().max_by_key(|&&(_, first, _)| first).map(|&(open, _, _)| open);
    if newest != Some(object) {
        return;
    }
    // what was found on another menu, or before this one went, is not here,
    // and a menu opens with its own outline, until the mouse moves
    if !aim_mouse.menu_screen.is_some_and(|(_, seen, last)| seen == object && frame <= last + age) {
        aim_mouse.menu_looked = aim_mouse.pointer.map(|pointer| (pointer, 0));
        aim_mouse.menu_part = None;
        aim_mouse.menu_click = false;
    }
    aim_mouse.menu_screen = Some((index, object, frame));
    menu_top::refresh_hover(aim_mouse, frame);
    if !aim_mouse.menu_hover || aim_mouse.saved.is_some() || system.services.hid.input.touch.is_some()
        || aim_mouse.menu_touched.is_some_and(|last| frame <= last + age) {
        return;
    }
    let at = (system.cpu.regs[13] - 0x40) & !7;
    if let Some((owner, last, mut wheel)) = aim_mouse.menu_wheel.take() {
        if owner == object && frame <= last + 30 {
            let steps = wheel.map(|value| value.trunc() as i32);
            for axis in 0..2 { wheel[axis] -= steps[axis] as f32; }
            aim_mouse.menu_wheel = Some((owner, last, wheel));
            if steps != [0, 0] {
                if !matches!(screen.vtable, 0x00CEE1C0 | 0x00CEFCA8) {
                    aim_mouse.menu_looked = None;
                    aim_mouse.menu_part = None;
                }
                let calls = scroll_armory(system, object, steps, at);
                system.aim_mouse.as_mut().unwrap().menu_calls = calls;
                next_menu_call(system, index, object, at);
                return;
            }
        }
    }
    if aim_mouse.menu_click {
        // what was found last, outlined first when it is not
        let Some(found) = aim_mouse.menu_part else {
            aim_mouse.menu_click = false;
            return;
        };
        let calls = (screen.outline)(system, object, found).unwrap_or_default();
        if let Some(aim_mouse) = &mut system.aim_mouse {
            aim_mouse.menu_calls = calls;
        }
        next_menu_call(system, index, object, at);
        return;
    }
    // looked again while nothing was found, for a second: a part can be
    // gone from under the mouse while it moves, as a book turning
    let touch = aim_mouse.menu_touch.filter(|&(_, seen)| frame <= seen + age).map(|(touch, _)| touch);
    let (Some(touch), Some((x, y))) = (touch, aim_mouse.pointer) else { return };
    let looked = aim_mouse.menu_looked.filter(|&(pointer, _)| Some(pointer) == aim_mouse.pointer);
    if looked.is_some_and(|(_, since)| aim_mouse.menu_part.is_some() || frame > since + 60) {
        return;
    }
    if looked.is_none() {
        aim_mouse.menu_looked = Some(((x, y), frame));
    }
    aim_mouse.menu_part = None;
    let mut record = [0u8; 0x18];
    record[0x10..0x14].copy_from_slice(&x.to_le_bytes());
    record[0x14..].copy_from_slice(&y.to_le_bytes());
    system.memory.write_bytes(at, &record);
    call_here(system, MENU_HIT, at, touch, Drawing::Menu(0, index), Vec::new());
    system.cpu.regs[2] = at + 0x10;
    system.cpu.regs[13] = at;
}

/// makes the menu's next call of those outlining what is under the mouse,
/// then, after a click, its A, coming back to its update.
fn next_menu_call(system: &mut System, index: usize, object: u32, at: u32) {
    let Some(aim_mouse) = &mut system.aim_mouse else { return };
    if aim_mouse.menu_calls.is_empty() && aim_mouse.menu_click && MENU_SCREENS[index].use_outlined == CLICK_TOUCHES {
        aim_mouse.menu_click = false;
        return;
    }
    if aim_mouse.menu_calls.is_empty() && aim_mouse.menu_click && MENU_SCREENS[index].use_outlined == 0 {
        aim_mouse.menu_click = false;
        activate_armory(system, object);
        return;
    }
    let call = match aim_mouse.menu_calls.is_empty() {
        true => std::mem::take(&mut aim_mouse.menu_click).then_some((MENU_SCREENS[index].use_outlined, [object, 0, 0])),
        false => Some(aim_mouse.menu_calls.remove(0)),
    };
    let Some((function, [r0, r1, r2])) = call else { return };
    call_here(system, function, r0, r1, Drawing::Menu(2, index), Vec::new());
    system.cpu.regs[2] = r2;
    if index >= 14 { system.cpu.regs[3] = 0; }
    system.cpu.regs[13] = at;
}

/// the value on the Fiend's Cauldron's bar, the intensity, from the pot's
/// task.
fn cauldron_value(system: &mut System) -> Option<f32> {
    let task = system.aim_mouse.as_ref()?.cauldron_task?;
    let value = f32::from_bits(read_word(system, task + CAULDRON_VALUE));
    (0.0..=10.0).contains(&value).then_some(value)
}

/// where the bar's dial goes: the part of the bottom screen around it, at
/// the value the pot's task has, its arrow's tip at the top, in a box
/// dial times the bar's size with that tip on the bar's lower edge. none
/// with no value.
fn dial_place(system: &mut System, layout: CauldronLayout) -> CauldronPlace {
    let none = CauldronPlace { from: [0.0, 0.0, 1.0, 1.0], to: [0.0, 0.0, 0.0, 0.0] };
    let Some(value) = cauldron_value(system) else { return none };
    let x = DIAL_ZERO + value * DIAL_STEP;
    let [bar_x, bar_y, bar_width, bar_height] = layout.bar.to;
    let [from_x, from_y, from_width, from_height] = layout.bar.from;
    let (scale_x, scale_y) = (bar_width / from_width, bar_height / from_height);
    let (width, height) = (DIAL_SIZE[0] * scale_x * layout.dial, DIAL_SIZE[1] * scale_y * layout.dial);
    let middle = bar_x + (x - from_x) * scale_x;
    CauldronPlace {
        from: [x - DIAL_SIZE[0] * 0.5, DIAL_TOP, DIAL_SIZE[0], DIAL_SIZE[1]],
        to: [middle - width * 0.5, bar_y + (DIAL_TOP - from_y) * scale_y, width, height],
    }
}

/// a camera's projection, as it is and turned to show only the part from of
/// a screen of size pixels in the whole view, when it has one kept. the
/// view of a wider screen is that much wider.
fn cropped_projection(system: &mut System, camera: u32, from: [f32; 4], size: [f32; 2], stereo: bool) -> Option<([u8; 64], [u8; 64])> {
    if camera == 0 || read_word(system, camera + 8) & 4 == 0 {
        return None;
    }
    let widen = system.aim_mouse.as_ref().map_or(1.0, |aim_mouse| aim_mouse.widen);
    let mut matrix = [0u8; 64];
    system.memory.read_bytes(camera + 0x6C, &mut matrix);
    let mut m: Vec<f32> = matrix.as_chunks::<4>().0.iter().map(|word| f32::from_le_bytes(*word)).collect();
    let [x, y, width, height] = from;
    let [screen_width, screen_height] = size;
    // the first row gives the view's -1..1 from the bottom of the screen
    // up, the second from its right edge to its left: each gets the
    // middle of the part and its share of the view. the top's stereo
    // camera (0x00916234) builds each eye's from it turned back, and its
    // middles end up the other way round, the second row's going across
    // the screen from the left and the first's up it
    let across = 1.0 - (x + width * 0.5) * 2.0 / screen_width;
    let up = 1.0 - (y + height * 0.5) * 2.0 / screen_height;
    let rows = match stereo {
        false => [(up, height / screen_height), (across / widen, width / screen_width / widen)],
        true => [(-across / widen, height / screen_height), (up, width / screen_width / widen)],
    };
    for (row, (middle, half)) in rows.into_iter().enumerate() {
        for column in 0..4 {
            m[row * 4 + column] = (m[row * 4 + column] - middle * m[12 + column]) / half;
        }
    }
    let mut part = [0u8; 64];
    part.iter_mut().zip(m.iter().flat_map(|value| value.to_le_bytes())).for_each(|(byte, value)| *byte = value);
    Some((matrix, part))
}

/// starts drawing the top picture's panel in its place: the top's scene
/// drawing's own setup called again, with its viewport the place's, its
/// camera showing the panel's part, and only the panel's node of the
/// picture's models shown, then (see top_screen_cauldron) its 3D, without
/// the hint (see top_text_viewport).
fn top_panel(system: &mut System, top: u32, place: CauldronPlace) {
    let scene = read_word(system, top + 0x40);
    let layers = read_word(system, scene + SCENE_LAYERS);
    let list = read_word(system, layers + TOP_LAYER * LAYER_SIZE + 4);
    if list == 0 {
        return;
    }
    let mut moved = Vec::new();
    let viewport = viewport_bytes(system, top, place.to);
    change(system, &mut moved, top + 0x44, &viewport);
    let renderer = read_word(system, scene + 4);
    for (at, len) in [(scene + 0xF4, 8), (renderer + 0x58, 4)] {
        let mut bytes = vec![0u8; len];
        system.memory.read_bytes(at, &mut bytes);
        moved.push((at, bytes));
    }
    let screen = read_word(system, top + 0x58);
    let camera = read_word(system, scene + SCENE_CAMERAS + screen * 4);
    if let Some((matrix, part)) = cropped_projection(system, camera, place.from, [400.0, 240.0], true) {
        moved.push((camera + 0x6C, matrix.to_vec()));
        system.memory.write_bytes(camera + 0x6C, &part);
    }
    let pictures = top_pictures(system, list);
    for &model in &pictures {
        show_nodes(system, &mut moved, model, |name| name == TOP_PANEL);
    }
    // the layer's other models, the hearts' number among them, left off
    let start = read_word(system, list + 0x18);
    let bytes: Vec<u8> = pictures.iter().flat_map(|model| model.to_le_bytes()).collect();
    change(system, &mut moved, start, &bytes);
    change(system, &mut moved, list + 0x1C, &(start + bytes.len() as u32).to_le_bytes());
    call_here(system, TOP_SCENE_SETUP, top, scene, Drawing::Cauldron(0), moved);
}

/// the Fiend's Cauldron's layout, and the bottom screen's scene drawing,
/// when it is shown on the top screen and the top's scene drawing, top, is
/// of the same scene.
fn cauldron_scenes(system: &mut System, top: u32) -> Option<(CauldronLayout, u32)> {
    let aim_mouse = system.aim_mouse.as_ref()?;
    let (layout, bottom) = (aim_mouse.cauldron_layout()?, aim_mouse.bottom_scene?);
    if !aim_mouse.cauldron_shown(system.frames) {
        return None;
    }
    let scene = read_word(system, top + 0x40);
    (read_word(system, bottom) == BOTTOM_SCENE && read_word(system, bottom + 0x40) == scene).then_some((layout, bottom))
}

/// the top screen's viewport for a box of it, x, y, width and height as
/// CauldronLayout has them, as the top's scene drawing, top, takes it, from
/// its own. the buffer is the screen turned, so a viewport's x and y count
/// from the screen's right and bottom edges, and a wider top screen is
/// squeezed into it.
fn viewport_bytes(system: &mut System, top: u32, view: [f32; 4]) -> Vec<u8> {
    let widen = system.aim_mouse.as_ref().map_or(1.0, |aim_mouse| aim_mouse.widen);
    let [x, y, width, height] = view;
    let (x, width) = (x / widen, width / widen);
    let screen = [0x44, 0x48, 0x4C, 0x50].map(|at| read_word(system, top + at) as i32 as f32);
    let viewport = [screen[0] + screen[2] - x - width, screen[1] + screen[3] - y - height, width, height];
    viewport.iter().flat_map(|value| (value.round() as i32).to_le_bytes()).collect()
}

/// before the top's scene drawing, r4, sets its viewport and draws its 3D:
/// while the Fiend's Cauldron is shown on the top screen, the top's
/// picture goes in its box. top_screen_cauldron puts the viewport back.
fn top_scene_viewport(system: &mut System) {
    let top = system.cpu.regs[4];
    let Some((layout, _)) = cauldron_scenes(system, top) else { return };
    // with no models in the layer it does not reach its 3D, nor
    // top_screen_cauldron
    let scene = read_word(system, top + 0x40);
    let layers = read_word(system, scene + SCENE_LAYERS);
    let list = read_word(system, layers + TOP_LAYER * LAYER_SIZE + 4);
    if list == 0 || read_word(system, list + 0x18) == read_word(system, list + 0x1C) {
        return;
    }
    let mut viewport = [0u8; 16];
    system.memory.read_bytes(top + 0x44, &mut viewport);
    let smaller: [u8; 16] = viewport_bytes(system, top, layout.top.to).try_into().unwrap();
    system.memory.write_bytes(top + 0x44, &smaller);
    let mut moved = Vec::new();
    // the picture's background and its panel
    for model in top_pictures(system, list) {
        show_nodes(system, &mut moved, model, |name| !TOP_BACKGROUND.contains(&name) && name != TOP_PANEL);
    }
    // and the part of it shown, while the viewport is the box's
    let screen = read_word(system, top + 0x58);
    let camera = read_word(system, scene + SCENE_CAMERAS + screen * 4);
    let projection = cropped_projection(system, camera, layout.top.from, [400.0, 240.0], true).map(|(matrix, part)| {
        system.memory.write_bytes(camera + 0x6C, &part);
        (camera + 0x6C, matrix, part)
    });
    if let Some(aim_mouse) = &mut system.aim_mouse {
        aim_mouse.top_viewport = Some((viewport, smaller));
        aim_mouse.top_moved = moved;
        aim_mouse.top_projection = projection;
    }
}

/// the top picture's models in a layer's list of them.
fn top_pictures(system: &mut System, list: u32) -> Vec<u32> {
    let (start, end) = (read_word(system, list + 0x18), read_word(system, list + 0x1C));
    let models: Vec<u32> = (start..end.min(start + 0x400)).step_by(4).map(|at| read_word(system, at)).collect();
    models.into_iter().filter(|&model| model_name(system, model) == TOP_PICTURE).collect()
}

/// hides a model's mesh nodes but those shown says to show, what was there
/// kept in moved.
fn show_nodes(system: &mut System, moved: &mut Vec<(u32, Vec<u8>)>, model: u32, shown: impl Fn(&str) -> bool) {
    let cmdl = read_word(system, model + 8);
    let dictionary = read_word(system, cmdl + 0xD0);
    if dictionary == 0 {
        return;
    }
    let dictionary = cmdl + 0xD0 + dictionary;
    let nodes = read_word(system, model + MODEL_NODES);
    let count = read_word(system, dictionary + 8).min(read_word(system, model + MODEL_NODES + 4).wrapping_sub(nodes) / 8);
    for node in 0..count {
        let name = dictionary + 0x24 + node * 0x10;
        let name = name.wrapping_add(read_word(system, name));
        if !shown(&c_string(system, name)) {
            change(system, moved, nodes + node * 8 + 4, &[0]);
        }
    }
}

/// the string at at, up to 32 bytes.
fn c_string(system: &mut System, at: u32) -> String {
    let mut bytes = [0u8; 32];
    system.memory.read_bytes(at, &mut bytes);
    let len = bytes.iter().position(|&byte| byte == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..len]).into_owned()
}

/// while the top's picture is drawn smaller, its text is drawn with the
/// whole screen's viewport and the game's projection, and what is drawn
/// after with the smaller one and the part's again.
fn top_text_viewport(system: &mut System, place: u32) {
    if place == TOP_TEXT && drawing(system) == Some(Drawing::Cauldron(1)) {
        // the panel's, past the text
        system.cpu.regs[15] = TOP_TEXT_DRAWN;
        return;
    }
    let Some(aim_mouse) = &mut system.aim_mouse else { return };
    if let Some(back) = aim_mouse.saved.take_if(|saved| saved.drawing == Drawing::Viewport(place)) {
        system.cpu.regs = back.regs;
        system.cpu.cpsr = back.cpsr;
        system.cpu.vfp.regs = back.vfp;
        return;
    }
    let Some((whole, smaller)) = aim_mouse.top_viewport else { return };
    if let Some((at, game, part)) = aim_mouse.top_projection {
        system.memory.write_bytes(at, if place == TOP_TEXT { &game } else { &part });
    }
    let viewport = if place == TOP_TEXT { whole } else { smaller };
    let manager = read_word(system, VSYNC_MANAGER);
    if manager == 0 {
        return;
    }
    let word = |i: usize| u32::from_le_bytes(viewport[i * 4..i * 4 + 4].try_into().unwrap());
    call_here(system, SET_VIEWPORT, manager, word(0), Drawing::Viewport(place), Vec::new());
    // the height goes on the stack, below what the place's function keeps
    // there; the stack is put back with the rest
    let sp = system.cpu.regs[13] - 8;
    system.cpu.regs[2] = word(1);
    system.cpu.regs[3] = word(2);
    system.cpu.regs[13] = sp;
    system.memory.write_bytes(sp, &word(3).to_le_bytes());
}

/// once the bottom's scene drawing has drawn its layers on the top screen
/// for top_screen_cauldron, sets the top's viewport back, and goes on past
/// the bottom's 2D to return.
fn cauldron_drawn(system: &mut System) {
    let Some(top) = system.aim_mouse.as_ref().and_then(|aim_mouse| aim_mouse.saved.as_ref()).filter(|saved| matches!(saved.drawing, Drawing::Cauldron(_) | Drawing::MenuTop(_))).map(|saved| saved.regs[4]) else {
        return;
    };
    let manager = read_word(system, VSYNC_MANAGER);
    if manager == 0 {
        return;
    }
    let viewport: Vec<u32> = (0..4).map(|i| read_word(system, top + 0x44 + i * 4)).collect();
    let cpu = &mut system.cpu;
    let sp = cpu.regs[13];
    cpu.regs[0] = manager;
    cpu.regs[1..4].copy_from_slice(&viewport[..3]);
    cpu.regs[14] = BOTTOM_SCENE_RETURN;
    cpu.regs[15] = SET_VIEWPORT;
    cpu.cpsr.thumb = false;
    system.memory.write_bytes(sp, &viewport[3].to_le_bytes());
}

/// writes bytes at at, keeping what was there in moved.
fn change(system: &mut System, moved: &mut Vec<(u32, Vec<u8>)>, at: u32, bytes: &[u8]) {
    let mut was = vec![0u8; bytes.len()];
    system.memory.read_bytes(at, &mut was);
    moved.push((at, was));
    system.memory.write_bytes(at, bytes);
}

/// the name of a scene model's CGFX model, empty for none.
fn model_name(system: &mut System, model: u32) -> String {
    let cmdl = read_word(system, model + 8);
    if cmdl == 0 || !system.memory.is_readable(cmdl, 0xD4) || read_word(system, cmdl + 4) != u32::from_le_bytes(*b"CMDL") {
        return String::new();
    }
    let at = cmdl.wrapping_add(0xC).wrapping_add(read_word(system, cmdl + 0xC));
    c_string(system, at)
}

/// starts drawing the first mirrored element from index on that the last
/// bottom pass drew, whether there was one. Keep the calls for both eyes;
/// the next bottom pass clears them before recording its visible elements.
fn start_mirrored(system: &mut System, from: usize) -> bool {
    let Some(aim_mouse) = &mut system.aim_mouse else { return false };
    if !aim_mouse.mirror_bottom {
        return false;
    }
    for index in from..MIRRORED.len() {
        let Some(kept) = aim_mouse.recorded[index].filter(|kept| kept.for_pass(aim_mouse.extras_pass) && Some(kept.call.regs[1]) == aim_mouse.context) else { continue };
        log::trace!("battle extras replay {} pass {}", index, aim_mouse.extras_pass);
        let recorded = kept.call;
        // These draw routines inherit the bottom pass's material selectors.
        // Restore them around each call and explicitly bind them on top.
        let mut moved = Vec::new();
        for (word, value) in kept.material.into_iter().enumerate() {
            change(system, &mut moved, recorded.regs[1] + 0xEC + word as u32 * 4, &value.to_le_bytes());
        }
        dirty_material(system, recorded.regs[1]);
        call_here(system, recorded.target, recorded.regs[0], recorded.regs[1], Drawing::Mirrored(index), moved);
        system.cpu.regs[2] = recorded.regs[2];
        system.cpu.regs[3] = recorded.regs[3];
        system.cpu.vfp.regs[..8].copy_from_slice(&recorded.floats);
        return true;
    }
    false
}

fn dirty_material(system: &mut System, context: u32) {
    let flags = read_word(system, context + 0x50) | 0x1F00;
    system.memory.write_bytes(context + 0x50, &flags.to_le_bytes());
}

/// has the game's code run function as if called from here with r0 and r1,
/// coming back here, the CPU kept to be put back then.
fn call_here(system: &mut System, function: u32, r0: u32, r1: u32, drawing: Drawing, moved: Vec<(u32, Vec<u8>)>) {
    let cpu = &mut system.cpu;
    let saved = Saved { regs: cpu.regs, cpsr: cpu.cpsr, vfp: cpu.vfp.regs, drawing, moved };
    cpu.regs[0] = r0;
    cpu.regs[1] = r1;
    cpu.regs[14] = drawing.place();
    cpu.regs[15] = function;
    cpu.cpsr.thumb = false;
    if let Some(aim_mouse) = &mut system.aim_mouse {
        aim_mouse.saved = Some(saved);
    }
}

/// the 2D context the top HUD was drawn through, when there is one.
fn context(system: &System) -> Option<u32> {
    system.aim_mouse.as_ref().and_then(|aim_mouse| aim_mouse.context)
}

/// starts drawing the portraits on the top screen, whether it did.
fn start_portraits(system: &mut System) -> bool {
    let scale = system.aim_mouse.as_ref().map_or(0.0, |aim_mouse| aim_mouse.portrait_scale);
    let Some(context) = context(system).filter(|_| scale > 0.0) else { return false };
    let mut pointer = [0u8; 4];
    system.memory.read_bytes(context + PORTRAITS, &mut pointer);
    let portraits = u32::from_le_bytes(pointer);
    if portraits == 0 {
        return false;
    }
    let mut fade = [0u8; 4];
    system.memory.read_bytes(context + BOTTOM_FADE, &mut fade);
    let fade = f32::from_le_bytes(fade);
    if !(fade < 1.0) {
        return false;
    }
    call_here(system, DRAW_PORTRAITS, portraits, context, Drawing::Portraits, Vec::new());
    system.cpu.vfp.regs[0] = (1.0 - fade.max(0.0)).to_bits();
    true
}

/// starts drawing the narration's line on the top screen, centred at its
/// bottom, when a line is shown.
fn start_narration(system: &mut System) {
    let on = system.aim_mouse.as_ref().is_some_and(|aim_mouse| aim_mouse.narration_on_top);
    let Some(context) = context(system).filter(|_| on) else { return };
    let message = context + NARRATION;
    let mut head = [0u8; 0x24];
    system.memory.read_bytes(message, &mut head);
    let word = |at: usize| u32::from_le_bytes(head[at..at + 4].try_into().unwrap());
    if head[0] == 0 || word(4) == 0 {
        return;
    }
    let (width, height) = (f32::from_bits(word(0x1C)), f32::from_bits(word(0x20)));
    if !(0.0..=400.0).contains(&width) || !(0.0..=240.0).contains(&height) {
        return;
    }
    let position = head[0x14..0x1C].to_vec();
    let mut moved = [0u8; 8];
    moved[..4].copy_from_slice(&((400.0 - width) * 0.5).to_le_bytes());
    moved[4..].copy_from_slice(&(240.0 - height - 8.0).to_le_bytes());
    system.memory.write_bytes(message + 0x14, &moved);
    call_here(system, DRAW_MESSAGE_LINE, message, context, Drawing::Narration, vec![(message + 0x14, position)]);
}

/// writes back what memory held before a call changed it, the last change
/// first.
fn put_back(system: &mut System, moved: &[(u32, Vec<u8>)]) {
    for (at, bytes) in moved.iter().rev() {
        system.memory.write_bytes(*at, bytes);
    }
}

/// what the game's code is drawing on the top screen for us now, if anything.
fn drawing(system: &System) -> Option<Drawing> {
    system.aim_mouse.as_ref().and_then(|aim_mouse| aim_mouse.saved.as_ref()).map(|saved| saved.drawing)
}

/// the quads a flush is about to draw, where they are and what they hold.
fn flushed_quads(system: &mut System) -> Option<(u32, Vec<u8>)> {
    let batch = system.cpu.regs[0];
    let mut header = [0u8; 12];
    system.memory.read_bytes(batch + 0x38, &mut header);
    let word = |i: usize| u32::from_le_bytes(header[i * 4..i * 4 + 4].try_into().unwrap());
    let (end, count) = (word(0), word(1).wrapping_sub(word(2)));
    if count == 0 || count > 0x400 || end < count * 0x60 {
        return None;
    }
    let mut quads = vec![0u8; (count * 0x60) as usize];
    system.memory.read_bytes(end - count * 0x60, &mut quads);
    Some((end - count * 0x60, quads))
}

fn get(quads: &[u8], at: usize) -> f32 {
    f32::from_le_bytes(quads[at..at + 4].try_into().unwrap())
}

fn set(quads: &mut [u8], at: usize, value: f32) {
    quads[at..at + 4].copy_from_slice(&value.to_le_bytes());
}

/// Fit an element as a group, keeping its shape and the gaps between its
/// sprites. Widescreen correction is applied after placement, about anchor.
fn fit_ui(quads: &mut [u8], source: UiBox, area: UiBox, anchor: f32, widen: f32, right: bool) {
    if !source.valid() || !area.valid() { hide_ui(quads); return; }
    let scale = ((area.max[0] - area.min[0]) / (source.max[0] - source.min[0]))
        .min((area.max[1] - area.min[1]) / (source.max[1] - source.min[1]));
    let width = (source.max[0] - source.min[0]) * scale;
    let left = if right { area.max[0] - width } else { (area.min[0] + area.max[0] - width) * 0.5 };
    for at in (0..quads.len()).step_by(0x18) {
        let x = left + (get(quads, at) - source.min[0]) * scale;
        let y = area.min[1] + (get(quads, at + 4) - source.min[1]) * scale;
        set(quads, at, anchor + (x - anchor) / widen);
        set(quads, at + 4, y);
    }
}

fn hide_ui(quads: &mut [u8]) {
    for at in (0..quads.len()).step_by(0x18) {
        set(quads, at, -10000.0);
        set(quads, at + 4, -10000.0);
    }
}

/// Clip the native boss card to its title during combat. Its two texture
/// tiles must share the same transform and UV clipping, including the seam.
fn clip_ui(quads: &mut [u8], source: UiBox) {
    for quad in quads.chunks_exact_mut(0x60) {
        let old = quad.to_vec();
        let min = [0, 1].map(|axis| (0..4).map(|i| get(&old, i * 0x18 + axis * 4)).fold(f32::MAX, f32::min));
        let max = [0, 1].map(|axis| (0..4).map(|i| get(&old, i * 0x18 + axis * 4)).fold(f32::MIN, f32::max));
        if (0..2).any(|axis| max[axis] <= source.min[axis] || min[axis] >= source.max[axis]) {
            hide_ui(quad);
            continue;
        }
        for axis in 0..2 {
            let coord = axis * 4;
            let uv = 0x10 + axis * 4;
            let start = get(&old, coord);
            let slope = (1..4).find_map(|i| {
                let distance = get(&old, i * 0x18 + coord) - start;
                (distance.abs() > 0.001).then(|| (get(&old, i * 0x18 + uv) - get(&old, uv)) / distance)
            }).unwrap_or(0.0);
            for at in (0..0x60).step_by(0x18) {
                let was = get(&old, at + coord);
                let clipped = was.clamp(source.min[axis], source.max[axis]);
                set(quad, at + coord, clipped);
                set(quad, at + uv, get(&old, at + uv) + (clipped - was) * slope);
            }
        }
    }
}

fn illustration_area(hud: f32, portrait: f32) -> UiBox {
    // Hearts end at (40 + 50) * hud. Portrait artwork starts about 64
    // pixels into its 256-pixel tile, above Pit's visible head.
    let bottom = if portrait > 0.0 { 240.0 - 192.0 * portrait } else { 240.0 - 48.0 * hud };
    UiBox { min: [400.0 - 136.0 * hud, 98.0 * hud], max: [400.0 - 8.0 * hud, bottom - 8.0 * hud] }
}

fn boss_area(hud: f32, combat: bool) -> UiBox {
    let (width, top, height) = if combat { (160.0, 8.0, 32.0) } else { (128.0, 40.0, 96.0) };
    UiBox { min: [200.0 - width * hud * 0.5, top * hud], max: [200.0 + width * hud * 0.5, (top + height) * hud] }
}

fn pickup_below_boss(hud: f32, portrait: f32, combat: bool) -> UiBox {
    let width = (216.0 * hud).min((400.0 - 512.0 * portrait - 16.0 * hud).max(16.0 * hud));
    UiBox { min: [200.0 - width * 0.5, boss_area(hud, combat).max[1] + 8.0 * hud],
        max: [200.0 + width * 0.5, 240.0 - 48.0 * hud] }
}

/// Special layouts use the original native draw calls, without the bottom
/// screen's full-screen bands. No gameplay state is changed to place them.
fn place_battle_panel(system: &mut System, index: usize, widen: f32) -> bool {
    let Some(mouse) = &system.aim_mouse else { return false };
    let (hud, portrait) = (mouse.hud_scale, mouse.portrait_scale);
    let kept = mouse.recorded[index];
    let boss = mouse.recorded[BOSS_CARD].or(mouse.recorded[BOSS_STATUS])
        .filter(|kept| kept.for_pass(mouse.extras_pass) && Some(kept.call.regs[1]) == mouse.context);
    let boss_context = boss.map(|kept| kept.call.regs[1]);
    // The normal player's health HUD is enabled once the reveal ends.
    let combat = boss_context.is_some_and(|context| read_word(system, context + 0x31BC) & 1 != 0);
    let caller = system.cpu.regs[14];
    let (source, area, anchor, right, hidden, clip) = match index {
        ILLUSTRATION => (kept.and_then(|kept| kept.bounds).unwrap_or(UiBox { min: [0.0; 2], max: [0.0; 2] }),
            illustration_area(hud, portrait), 400.0, true, caller != 0x00A0F9E8, false),
        BOSS_CARD | BOSS_EFFECT | BOSS_STATUS => (
            UiBox { min: [0.0; 2], max: [320.0, if combat && index == BOSS_CARD { 64.0 } else { 240.0 }] },
            boss_area(hud, combat), 200.0, false,
            index == BOSS_STATUS && caller == 0x00A15D3C, combat && index == BOSS_CARD),
        PICKUP if boss.is_some() && caller != TOP_UI_AFTER_HUD => (
            UiBox { min: [23.0, 118.0], max: [301.0, 191.0] },
            pickup_below_boss(hud, portrait, combat), 200.0, false, caller == 0x009EAC80, false),
        _ => return false,
    };
    let Some((at, mut quads)) = flushed_quads(system) else { return true };
    if hidden {
        // Omit decorative bands rather than submitting degenerate quads.
        // Leave material changes dirty for the next visible sprite.
        let batch = system.cpu.regs[0];
        let first = read_word(system, batch + 0x40);
        system.memory.write_bytes(batch + 0x38, &at.to_le_bytes());
        system.memory.write_bytes(batch + 0x3C, &first.to_le_bytes());
        return true;
    } else {
        if clip { clip_ui(&mut quads, source); }
        fit_ui(&mut quads, source, area, anchor, widen, right);
    }
    system.memory.write_bytes(at, &quads);
    true
}

/// puts a mirrored element's sprites, drawn for the bottom screen, by its
/// corner of the top screen, at the HUD's scale, below what is there; those
/// it leaves off go far off the screen.
fn place_mirrored(system: &mut System, mirrored: &Mirrored, widen: f32) {
    let scale = system.aim_mouse.as_ref().map_or(1.0, |aim_mouse| aim_mouse.hud_scale);
    let caller = system.cpu.regs[14];
    let hide = mirrored.hidden.contains(&caller);
    let Some((at, mut quads)) = flushed_quads(system) else { return };
    let apart = mirrored.last.as_ref().filter(|_| caller == TOP_UI_AFTER_HUD);
    let (from, to) = apart.map_or((mirrored.from, mirrored.to), |apart| (apart.from, apart.to));
    let (below, size) = apart.map_or((mirrored.below, mirrored.size), |apart| (apart.below, apart.size));
    let down = if to[1] == 0.0 { below * scale } else { -below * scale };
    let scale = scale * size;
    for quad in (0..quads.len()).step_by(0x60) {
        let middle = (0..4).map(|corner| get(&quads, quad + corner * 0x18 + 4)).sum::<f32>() / 4.0;
        let moved = match mirrored.swap_rows {
            Some([split, down, up]) => if middle < split { down } else { -up },
            None => 0.0,
        };
        for corner in (quad..quad + 0x60).step_by(0x18) {
            let (x, y) = if hide {
                (-10000.0, -10000.0)
            } else {
                (
                    to[0] + (get(&quads, corner) - from[0]) * scale / widen,
                    to[1] + down + (get(&quads, corner + 4) + moved - from[1]) * scale,
                )
            };
            set(&mut quads, corner, x);
            set(&mut quads, corner + 4, y);
        }
    }
    system.memory.write_bytes(at, &quads);
}

/// puts the pause menu or a dialog, drawn for the bottom screen, in the
/// middle of the top screen as big as it is drawn, so a touch there is the
/// same one moved left, see pause_menu_touch. what spans the bottom screen,
/// the dimming and the band behind Options, spans the top screen, and
/// Resume, Retreat and Remove Display go by its edges as they are by the
/// bottom screen's.
fn place_pause_menu(system: &mut System, widen: f32) {
    let caller = system.cpu.regs[14];
    let (buttons, corner) = (PAUSE_BUTTONS.contains(&caller), REMOVE_DISPLAY.contains(&caller));
    let Some((at, mut quads)) = flushed_quads(system) else { return };
    for quad in (0..quads.len()).step_by(0x60) {
        let xs = (0..4).map(|corner| get(&quads, quad + corner * 0x18));
        let (left, right) = xs.fold((f32::MAX, f32::MIN), |(left, right), x| (left.min(x), right.max(x)));
        let across = left <= 0.5 && right >= 319.5;
        // Resume on the left, Retreat on the right
        let by_right = corner || (buttons && left + right >= 320.0);
        let offset = match (buttons || corner, by_right) {
            (false, _) => PAUSE_MENU_OFFSET,
            (true, false) => 0.0,
            (true, true) => 2.0 * PAUSE_MENU_OFFSET,
        };
        // and narrower on a wider top screen, toward the edge it is by or
        // the middle
        let anchor = match (buttons || corner, by_right) {
            (false, _) => 200.0,
            (true, false) => 0.0,
            (true, true) => 400.0,
        };
        for corner in (quad..quad + 0x60).step_by(0x18) {
            let x = get(&quads, corner);
            let x = if across { x * 400.0 / 320.0 } else { anchor + (x + offset - anchor) / widen };
            set(&mut quads, corner, x);
        }
    }
    system.memory.write_bytes(at, &quads);
    if buttons || corner {
        let frame = system.frames;
        if let Some(aim_mouse) = &mut system.aim_mouse {
            aim_mouse.edges_drawn = Some(frame);
        }
    }
}

/// puts the portraits, drawn for the bottom screen, in the top screen's
/// bottom corners, smaller: a speaker on the left half of the bottom screen
/// in the bottom left corner, one on the right in the bottom right.
fn place_portraits(system: &mut System, widen: f32) {
    let scale = system.aim_mouse.as_ref().map_or(1.0, |aim_mouse| aim_mouse.portrait_scale);
    let Some((at, mut quads)) = flushed_quads(system) else { return };
    for quad in (0..quads.len()).step_by(0x60) {
        let middle = (0..4).map(|corner| get(&quads, quad + corner * 0x18)).sum::<f32>() / 4.0;
        if !middle.is_finite() {
            return;
        }
        // the bottom screen's edge the speaker is by, and the top screen's
        let (from, to) = if middle < 160.0 { (0.0, 0.0) } else { (320.0, 400.0) };
        for corner in (quad..quad + 0x60).step_by(0x18) {
            let x = to + (get(&quads, corner) - from) * scale / widen;
            let y = 240.0 + (get(&quads, corner + 4) - 240.0) * scale;
            set(&mut quads, corner, x);
            set(&mut quads, corner + 4, y);
        }
    }
    system.memory.write_bytes(at, &quads);
}

/// the top screen's battle HUD, by where its sprite flushes return to:
/// score, health bar, wings, status icon. found in the USA build's air and
/// ground battles, other HUD stays the size the game draws it.
const TOP_HUD: [u32; 7] = [0x009F2F90, 0x009F45FC, 0x009F4800, 0x009F4BD4, 0x009F52C8, 0x009F557C, 0x00725828];

/// Training's defeat and distance counters share one flush across both
/// bottom corners. Keep each panel's slices and digits on the same anchor.
const TRAINING_HUD: u32 = 0x0060D66C;

fn place_training_hud(quads: &mut [u8], scale: f32, widen: f32) {
    for quad in (0..quads.len()).step_by(0x60) {
        let middle = (0..4).map(|corner| get(quads, quad + corner * 0x18)).sum::<f32>() * 0.25;
        let anchor = if middle < 200.0 { 0.0 } else { 400.0 };
        for corner in (quad..quad + 0x60).step_by(0x18) {
            let x = anchor + (get(quads, corner) - anchor) * scale / widen;
            let y = 240.0 + (get(quads, corner + 4) - 240.0) * scale;
            set(quads, corner, x);
            set(quads, corner + 4, y);
        }
    }
}

/// whether a sprite batch flushed from caller draws the top screen's HUD,
/// which the narration's line is part of while it is drawn there.
fn top_hud(system: &System, caller: u32) -> bool {
    TOP_HUD.contains(&caller) || drawing(system) == Some(Drawing::Narration)
}

/// draws a top screen HUD element smaller: the quads the flush is about to
/// draw, scaled together toward the edge or corner they sit by, or their
/// own middle when they are by none, like a marker on an enemy.
fn shrink_hud(system: &mut System) {
    // The console status pass flushes the battery sprite separately from
    // the scene's menu models. Remove it before any widescreen transform.
    if system.cpu.regs[14] == 0x00725828 {
        if let Some((at, quads)) = flushed_quads(system) {
            let kept: Vec<u8> = quads.chunks_exact(0x60).filter(|quad| {
                !(0..4).all(|corner| {
                    let at = corner * 0x18;
                    (382.0..=400.0).contains(&get(quad, at))
                        && (0.0..=13.0).contains(&get(quad, at + 4))
                })
            }).flatten().copied().collect();
            if kept.len() != quads.len() {
                let batch = system.cpu.regs[0];
                let first = read_word(system, batch + 0x40);
                system.memory.write_bytes(at, &kept);
                system.memory.write_bytes(batch + 0x38, &(at + kept.len() as u32).to_le_bytes());
                system.memory.write_bytes(batch + 0x3C, &(first + (kept.len() / 0x60) as u32).to_le_bytes());
            }
        }
    }
    let widen = widening(system);
    match drawing(system) {
        Some(Drawing::Portraits) => return place_portraits(system, widen),
        Some(Drawing::Mirrored(index)) => {
            if !place_battle_panel(system, index, widen) {
                place_mirrored(system, &MIRRORED[index], widen);
            }
            return;
        }
        Some(Drawing::PauseMenu(_)) => return place_pause_menu(system, widen),
        _ => {}
    }
    if drawing(system).is_none() && system.cpu.regs[14] == TRAINING_HUD {
        let scale = system.aim_mouse.as_ref().map_or(1.0, |aim_mouse| aim_mouse.hud_scale);
        if let Some((at, mut quads)) = flushed_quads(system) {
            place_training_hud(&mut quads, scale, widen);
            system.memory.write_bytes(at, &quads);
        }
        return;
    }
    // the rest of the top screen's 2D only narrower, on a wider one
    let hud = top_hud(system, system.cpu.regs[14]);
    let scale = if hud { system.aim_mouse.as_ref().map_or(1.0, |aim_mouse| aim_mouse.hud_scale) } else { 1.0 };
    if scale == 1.0 && widen == 1.0 {
        return;
    }
    let batch = system.cpu.regs[0];
    let mut header = [0u8; 12];
    system.memory.read_bytes(batch + 0x38, &mut header);
    let word = |i: usize| u32::from_le_bytes(header[i * 4..i * 4 + 4].try_into().unwrap());
    let (end, count) = (word(0), word(1).wrapping_sub(word(2)));
    if count == 0 || count > 0x400 || end < count * 0x60 {
        return;
    }
    let mut quads = vec![0u8; (count * 0x60) as usize];
    system.memory.read_bytes(end - count * 0x60, &mut quads);
    let get = |quads: &[u8], at: usize| f32::from_le_bytes(quads[at..at + 4].try_into().unwrap());
    let corners = (0..quads.len()).step_by(0x18);
    let (mut min, mut max) = ([f32::MAX; 2], [f32::MIN; 2]);
    for at in corners.clone() {
        for axis in 0..2 {
            let value = get(&quads, at + axis * 4);
            if !value.is_finite() {
                return;
            }
            min[axis] = min[axis].min(value);
            max[axis] = max[axis].max(value);
        }
    }
    // the whole screen, a fade or a flash, stays as it is, the narration's
    // line, as wide as it may be, doesn't. sprites spread over it, markers
    // on enemies say, each get narrower about their own middle
    let narration = drawing(system) == Some(Drawing::Narration);
    if !narration && (max[0] - min[0] > 300.0 || max[1] - min[1] > 200.0) {
        if widen == 1.0 {
            return;
        }
        for quad in (0..quads.len()).step_by(0x60) {
            let xs = (0..4).map(|corner| get(&quads, quad + corner * 0x18));
            let (left, right) = xs.fold((f32::MAX, f32::MIN), |(left, right), x| (left.min(x), right.max(x)));
            if right - left >= 390.0 {
                continue;
            }
            let middle = (left + right) * 0.5;
            for corner in (quad..quad + 0x60).step_by(0x18) {
                let x = get(&quads, corner);
                set(&mut quads, corner, middle + (x - middle) / widen);
            }
        }
        system.memory.write_bytes(end - count * 0x60, &quads);
        return;
    }
    let anchor = |min: f32, max: f32, size: f32| {
        if min < size * 0.12 {
            0.0
        } else if max > size * 0.88 {
            size
        } else {
            (min + max) * 0.5
        }
    };
    // the narration's line shrinks toward the middle of the bottom edge,
    // where it is put
    let anchor = if narration { [200.0, 240.0] } else { [anchor(min[0], max[0], 400.0), anchor(min[1], max[1], 240.0)] };
    let scales = [scale / widen, scale];
    for at in corners {
        for axis in 0..2 {
            let value = get(&quads, at + axis * 4);
            let scaled = anchor[axis] + (value - anchor[axis]) * scales[axis];
            quads[at + axis * 4..at + axis * 4 + 4].copy_from_slice(&scaled.to_le_bytes());
        }
    }
    system.memory.write_bytes(end - count * 0x60, &quads);
}

/// draws the reticle at the HUD's scale: its parts' size, made smaller once
/// before the first is drawn.
fn shrink_reticle(system: &mut System) {
    let scale = system.aim_mouse.as_ref().map_or(1.0, |aim_mouse| aim_mouse.hud_scale);
    if scale == 1.0 || system.cpu.regs[4] != 0 {
        return;
    }
    let at = system.cpu.regs[1];
    let mut size = [0u8; 4];
    system.memory.read_bytes(at, &mut size);
    let size = f32::from_le_bytes(size) * scale;
    system.memory.write_bytes(at, &size.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn panel_quad(bounds: UiBox) -> Vec<u8> {
        let mut quad = vec![0; 0x60];
        for (i, (x, y, u, v)) in [
            (bounds.min[0], bounds.min[1], 0.0, 1.0),
            (bounds.max[0], bounds.min[1], 1.0, 1.0),
            (bounds.min[0], bounds.max[1], 0.0, 0.0),
            (bounds.max[0], bounds.max[1], 1.0, 0.0),
        ].into_iter().enumerate() {
            for (offset, value) in [(0, x), (4, y), (0x10, u), (0x14, v)] {
                set(&mut quad, i * 0x18 + offset, value);
            }
            quad[i * 0x18 + 0xC..i * 0x18 + 0x10].copy_from_slice(&0x80FFFFFFu32.to_le_bytes());
        }
        quad
    }

    #[test]
    fn dialogue_images_fit_above_pits_head_without_stretching() {
        for hud in [0.5, 0.7, 1.0] {
            for widen in [1.0, 16.0 / 9.0 / (5.0 / 3.0), 21.0 / 9.0 / (5.0 / 3.0)] {
                for source in [UiBox { min: [112.0, 60.0], max: [208.0, 184.0] },
                    UiBox { min: [53.0, 56.0], max: [273.0, 184.0] }] {
                    let mut quad = panel_quad(source);
                    let original = quad.clone();
                    let area = illustration_area(hud, 0.5);
                    fit_ui(&mut quad, source, area, 400.0, widen, true);
                    let physical_x = |at| 400.0 + (get(&quad, at) - 400.0) * widen;
                    assert!(physical_x(0) >= area.min[0] - 0.001);
                    assert!((physical_x(0x18) - area.max[0]).abs() < 0.001);
                    assert!((get(&quad, 4) - 98.0 * hud).abs() < 0.001);
                    assert!(get(&quad, 0x34) <= 240.0 - 192.0 * 0.5 - 8.0 * hud + 0.001);
                    let ratio = (physical_x(0x18) - physical_x(0)) / (get(&quad, 0x34) - get(&quad, 4));
                    assert!((ratio - (source.max[0] - source.min[0]) / (source.max[1] - source.min[1])).abs() < 0.001);
                    for at in (0..0x60).step_by(0x18) {
                        assert_eq!(&quad[at + 8..at + 0x18], &original[at + 8..at + 0x18]);
                    }
                }
            }
        }
    }

    #[test]
    fn boss_title_tiles_keep_their_seam_and_uvs_when_clipped() {
        let source = UiBox { min: [0.0; 2], max: [320.0, 64.0] };
        let mut left = panel_quad(UiBox { min: [0.0; 2], max: [256.0, 240.0] });
        let mut right = panel_quad(UiBox { min: [256.0, 0.0], max: [320.0, 240.0] });
        for tile in [&mut left, &mut right] {
            clip_ui(tile, source);
            assert_eq!(get(tile, 0x34), 64.0);
            assert!((get(tile, 0x44) - (1.0 - 64.0 / 240.0)).abs() < 0.001);
            fit_ui(tile, source, boss_area(0.7, true), 200.0, 1.4, false);
        }
        assert_eq!(get(&left, 0x18), get(&right, 0));
        assert_eq!(get(&left, 0x4C), get(&right, 0x34));
    }

    #[test]
    fn boss_pickups_fit_between_the_card_portraits_and_subtitles() {
        for hud in [0.5, 0.7, 1.0] {
            for combat in [false, true] {
                let boss = boss_area(hud, combat);
                let pickup = pickup_below_boss(hud, 0.5, combat);
                assert!(pickup.valid());
                assert!(pickup.min[1] >= boss.max[1] + 8.0 * hud);
                assert!(pickup.min[0] >= 128.0 && pickup.max[0] <= 272.0);
                assert!(pickup.max[1] <= 240.0 - 48.0 * hud);
                let image = illustration_area(hud, 0.5);
                assert!(boss.max[0] <= image.min[0] || boss.max[1] <= image.min[1]);
            }
        }
    }

    #[test]
    fn mirrored_calls_expire_by_ui_pass_instead_of_vblank_time() {
        let kept = MirroredCall { call: Recorded { target: 0, regs: [0; 4], floats: [0; 8], matrices: None }, pass: 10, bounds: None, material: [0; 4] };
        assert!(kept.for_pass(11));
        assert!(kept.for_pass(11)); // the shared list serves both eyes
        assert!(!kept.for_pass(12)); // no intervening bottom recording
        assert!(!kept.for_pass(10));
        assert!(!kept.for_pass(9));
    }

    #[test]
    fn narration_stays_centered_at_the_shared_training_return_address() {
        let mut system = armory_test_system();
        let batch = 0x1400_0000;
        let vertices = batch + 0x1000;
        system.cpu.regs[0] = batch;
        system.cpu.regs[14] = TOP_UI_AFTER_HUD;
        for (offset, value) in [(0x38, vertices + 0x60), (0x3C, 1), (0x40, 0)] {
            system.memory.write_bytes(batch + offset, &value.to_le_bytes());
        }
        for hud in [0.5, 0.7, 1.0] {
            for widen in [1.0, 16.0 / 9.0 / (5.0 / 3.0), 21.0 / 9.0 / (5.0 / 3.0)] {
                let mut quad = panel_quad(UiBox { min: [100.0, 202.0], max: [300.0, 232.0] });
                system.memory.write_bytes(vertices, &quad);
                system.aim_mouse = Some(AimMouse { hud_scale: hud, widen, screens: vec![(batch, 0)],
                    saved: Some(Saved { regs: system.cpu.regs, cpsr: system.cpu.cpsr, vfp: system.cpu.vfp.regs,
                        drawing: Drawing::Narration, moved: Vec::new() }), ..AimMouse::default() });
                shrink_hud(&mut system);
                system.memory.read_bytes(vertices, &mut quad);
                assert!(((get(&quad, 0) + get(&quad, 0x18)) * 0.5 - 200.0).abs() < 0.001);
                assert!((get(&quad, 0x18) - get(&quad, 0) - 200.0 * hud / widen).abs() < 0.001);
                assert!((get(&quad, 0x34) - (240.0 - 8.0 * hud)).abs() < 0.001);
            }
        }
    }

    #[test]
    fn training_panels_keep_their_slices_joined() {
        // Adjacent slices in each corner, as submitted in the shared batch.
        let boxes = [(2.0, 39.0), (39.0, 87.0), (233.0, 269.0), (269.0, 383.0)];
        for widen in [1.0, 16.0 / 10.0, 21.0 / 10.0] {
            let mut quads = vec![0; boxes.len() * 0x60];
            for (index, (left, right)) in boxes.into_iter().enumerate() {
                for (corner, (x, y)) in [(left, 200.0), (right, 200.0), (right, 242.0), (left, 242.0)].into_iter().enumerate() {
                    set(&mut quads, index * 0x60 + corner * 0x18, x);
                    set(&mut quads, index * 0x60 + corner * 0x18 + 4, y);
                }
            }
            place_training_hud(&mut quads, 0.7, widen);
            assert_eq!(get(&quads, 0x18), get(&quads, 0x60));
            assert_eq!(get(&quads, 2 * 0x60 + 0x18), get(&quads, 3 * 0x60));
            assert!((get(&quads, 4) - 212.0).abs() < 0.001);
            assert!((get(&quads, 0x18) - 39.0 * 0.7 / widen).abs() < 0.001);
            assert!((get(&quads, 3 * 0x60 + 0x18) - (400.0 - 17.0 * 0.7 / widen)).abs() < 0.001);
        }
    }

    /// a touch of the top screen's pause menu touches the bottom screen
    /// where the same thing is, by the edges too, and nothing beside what
    /// went to them.
    #[test]
    fn pause_menu_touches_follow_the_edges() {
        let mut aim_mouse = AimMouse { pause_drawn: Some(10), ..AimMouse::default() };
        // the main page drawn by the edges, Resume, Retreat, Options, Remove Display
        aim_mouse.edges_drawn = Some(10);
        assert_eq!(aim_mouse.pause_menu_touch(11, 60.0, 83.0), Some((60.0, 83.0)));
        assert_eq!(aim_mouse.pause_menu_touch(11, 330.0, 83.0), Some((250.0, 83.0)));
        assert_eq!(aim_mouse.pause_menu_touch(11, 200.0, 134.0), Some((160.0, 134.0)));
        assert_eq!(aim_mouse.pause_menu_touch(11, 340.0, 222.0), Some((260.0, 222.0)));
        // between Resume and Retreat, and left of Remove Display
        assert_eq!(aim_mouse.pause_menu_touch(11, 200.0, 83.0), None);
        assert_eq!(aim_mouse.pause_menu_touch(11, 250.0, 222.0), None);
        // a page of Options, all of it in the middle
        assert_eq!(aim_mouse.pause_menu_touch(20, 60.0, 83.0), Some((20.0, 83.0)));
        assert_eq!(aim_mouse.pause_menu_touch(20, 20.0, 83.0), None);
    }

    /// a menu takes a click while it is shown, over a part it outlines that
    /// was found where the mouse still is, and not otherwise.
    #[test]
    fn menus_take_clicks_over_their_parts() {
        let mut aim_mouse = AimMouse { menu_hover: true, ..AimMouse::default() };
        aim_mouse.hover(Some((160.0, 80.0)));
        aim_mouse.menu_screen = Some((0, 0x1404_E918, 10));
        aim_mouse.menu_looked = Some(((160.0, 80.0), 10));
        // nothing found there yet
        assert!(!aim_mouse.menu_click(11));
        aim_mouse.menu_part = Some(Found { hash: 0xA70D_5C64, name: 0x14B9_51A0, element: 0x1406_D9D0, part: 0 });
        assert!(aim_mouse.menu_click(11));
        // the menu gone, or the mouse moved since
        assert!(!aim_mouse.menu_click(20));
        aim_mouse.hover(Some((161.0, 80.0)));
        assert!(!aim_mouse.menu_click(11));
        // or the mouse's menus off
        aim_mouse.hover(Some((160.0, 80.0)));
        aim_mouse.menu_hover = false;
        assert!(!aim_mouse.menu_click(11));
    }

    fn clickable_menu() -> AimMouse {
        let mut mouse = AimMouse { menu_hover: true, ..AimMouse::default() };
        mouse.hover(Some((160.0, 80.0)));
        mouse.menu_screen = Some((0, 0x1404_E918, 10));
        mouse.menu_looked = Some(((160.0, 80.0), 10));
        mouse.menu_part = Some(Found { hash: FILE_PARTS[0], name: 0x14B9_51A0, element: 0x1406_D9D0, part: 0 });
        mouse
    }

    #[test]
    fn short_menu_press_clicks_only_on_release() {
        let mut mouse = clickable_menu();
        assert_eq!(mouse.menu_touch(10, true), None);
        assert!(!mouse.menu_click);
        assert_eq!(mouse.menu_touch(11, true), None);
        assert_eq!(mouse.menu_touch(12, false), None);
        assert!(std::mem::take(&mut mouse.menu_click));
        mouse.menu_touch(13, false);
        assert!(!mouse.menu_click);
    }

    #[test]
    fn wheel_fractions_stay_with_the_menu_and_never_click() {
        let mut mouse = clickable_menu();
        mouse.menu_screen.as_mut().unwrap().0 = 18;
        assert!(mouse.scroll_menu(10, [0.0, 0.25]));
        assert!(mouse.scroll_menu(11, [0.0, 0.75]));
        assert_eq!(mouse.menu_wheel.unwrap().2, [0.0, 1.0]);
        assert!(!mouse.menu_click);
        mouse.menu_screen = Some((18, 0x1405_0000, 12));
        assert!(mouse.scroll_menu(12, [1.0, 0.0]));
        assert_eq!(mouse.menu_wheel.unwrap().2, [1.0, 0.0]);
        assert!(!mouse.scroll_menu(16, [0.0, 1.0]));
        mouse.captured = true;
        assert!(!mouse.scroll_menu(12, [0.0, 1.0]));
        mouse.captured = false;
        mouse.menu_button_down = true;
        assert!(!mouse.scroll_menu(12, [0.0, 1.0]));
        mouse.menu_button_down = false;
        assert!(!mouse.scroll_menu(12, [f32::NAN, 0.0]));
    }

    fn armory_test_system() -> System {
        let mut system = System::new(crate::Config::default());
        system.memory.map(0x1400_0000, 0x2000_0000, 0x10000, crate::memory::Permission::RW, crate::memory::MemoryState::Private);
        system
    }

    #[test]
    fn armory_rows_follow_the_viewport_and_reject_empty_space() {
        let mut system = armory_test_system();
        let list = 0x1400_0000;
        for (offset, value) in [(4, 30.0f32.to_bits()), (0xFC, 2.3f32.to_bits()), (0xD0, 4)] {
            system.memory.write_bytes(list + offset, &value.to_le_bytes());
        }
        assert_eq!(list_row(&mut system, list, 29.0), None);
        assert_eq!(list_row(&mut system, list, 31.0), Some(0));
        assert_eq!(list_row(&mut system, list, 54.0), Some(1));
        assert_eq!(list_row(&mut system, list, 123.0), None);
        system.memory.write_bytes(list + 0xEC, &2u32.to_le_bytes());
        assert_eq!(list_row(&mut system, list, 31.0), Some(2));
        assert_eq!(list_row(&mut system, list, 77.0), None);
    }

    #[test]
    fn fusion_wheel_axes_are_independent_and_clamped_without_activating() {
        let mut system = armory_test_system();
        let object = 0x1400_0000;
        system.memory.write_bytes(object, &0x00CEE148u32.to_le_bytes());
        system.memory.write_bytes(object + 0x2B40, &20u32.to_le_bytes());
        scroll_armory(&mut system, object, [3, 0], object + 0x5000);
        assert_eq!(read_word(&mut system, object + 0x2B28), 3);
        assert_eq!(read_word(&mut system, object + 0x2B30), 0);
        scroll_armory(&mut system, object, [0, 2], object + 0x5000);
        assert_eq!(read_word(&mut system, object + 0x2B28), 3);
        assert_eq!(read_word(&mut system, object + 0x2B30), 2);
        scroll_armory(&mut system, object, [-99, 99], object + 0x5000);
        assert_eq!(read_word(&mut system, object + 0x2B28), 0);
        assert_eq!(read_word(&mut system, object + 0x2B30), 13);
        assert_eq!(read_word(&mut system, object + 0x2DEC), 0);
    }

    #[test]
    fn part_lookup_ends_at_the_head_instead_of_interpreting_node_colours() {
        let mut system = armory_test_system();
        let part = 0x1400_0000;
        let head = part + 0x1A8;
        let node = part + 0x300;
        let component = part + 0x500;
        for (at, value) in [(head, head), (head + 4, node), (node, component), (node + 8, head),
            (node + 12, head), (node + 16, 2), (node + 20, 0x00D7681C), (component + 0x1C, 8)] {
            system.memory.write_bytes(at, &value.to_le_bytes());
        }
        assert_eq!(kept_of_kind(&mut system, part, 0x00D7681C), 8);
        assert_eq!(kept_of_kind(&mut system, part, 0x00D7681D), 0);
        assert_eq!(kept_of_kind(&mut system, part, 0x00D7681B), 0);
    }

    #[test]
    fn held_menu_press_becomes_a_continuous_touch_without_a_click() {
        let mut mouse = clickable_menu();
        assert_eq!(mouse.menu_touch(10, true), None);
        mouse.menu_screen.as_mut().unwrap().2 = 21;
        assert_eq!(mouse.menu_touch(21, true), None);
        assert_eq!(mouse.menu_touch(22, true), Some((160.0, 80.0)));
        mouse.hover(Some((180.0, 100.0)));
        assert_eq!(mouse.menu_touch(23, true), Some((160.0, 80.0)));
        mouse.menu_screen.as_mut().unwrap().2 = 25;
        assert_eq!(mouse.menu_touch(25, true), Some((180.0, 100.0)));
        assert_eq!(mouse.menu_touch(26, false), None);
        assert!(!mouse.menu_click);
        assert!(mouse.menu_looked.is_none());
    }

    #[test]
    fn moving_a_menu_press_starts_at_the_grabbed_item() {
        let mut mouse = clickable_menu();
        mouse.menu_touch(10, true);
        mouse.hover(Some((200.0, 120.0)));
        assert_eq!(mouse.menu_touch(11, true), Some((160.0, 80.0)));
        mouse.menu_screen.as_mut().unwrap().2 = 14;
        assert_eq!(mouse.menu_touch(14, true), Some((200.0, 120.0)));
        mouse.menu_touch(15, false);
        assert!(!mouse.menu_click);
    }

    #[test]
    fn native_controls_touch_immediately_and_cancelled_presses_do_not_click() {
        let mut mouse = clickable_menu();
        mouse.menu_part = None;
        assert_eq!(mouse.menu_touch(10, true), Some((160.0, 80.0)));
        mouse.menu_part = Some(Found { hash: FILE_PARTS[0], name: 0, element: 0, part: 0 });
        assert_eq!(mouse.menu_touch(11, true), Some((160.0, 80.0)));
        mouse.menu_touch(12, false);
        assert!(!mouse.menu_click);
        mouse.menu_touch(12, true);
        mouse.cancel_menu_touch();
        mouse.menu_touch(13, false);
        assert!(!mouse.menu_click);
        mouse.menu_touch(13, true);
        mouse.menu_screen = Some((1, 0x1405_0000, 14));
        assert_eq!(mouse.menu_touch(14, true), None);
        mouse.menu_touch(15, false);
        assert!(!mouse.menu_click);
    }

    #[test]
    fn menu_click_waits_for_the_queued_outline_to_update() {
        let mut system = System::new(crate::Config::default());
        let screen = 12;
        system.cpu.regs[0] = 0x1234;
        system.cpu.regs[13] = 0x1000;
        system.cpu.regs[15] = MENU_SCREENS[screen].update;
        system.aim_mouse = Some(AimMouse {
            menu_click: true,
            menu_calls: vec![(QUEUE_PART, [0x1330, 0x2000, 0])],
            ..AimMouse::default()
        });
        next_menu_call(&mut system, screen, 0x1234, 0xFC0);
        // Return from the last queueing call, before the game's update.
        outline_menu(&mut system, MENU_SCREENS[screen].update);
        assert_eq!(system.cpu.regs[15], MENU_SCREENS[screen].update);
        assert!(system.aim_mouse.as_ref().unwrap().saved.is_none());
        assert!(system.aim_mouse.as_ref().unwrap().menu_click);
        // Once the update has applied it, A can use the new choice.
        next_menu_call(&mut system, screen, 0x1234, 0xFC0);
        assert_eq!(system.cpu.regs[15], MENU_SCREENS[screen].use_outlined);
        assert!(!system.aim_mouse.as_ref().unwrap().menu_click);
    }

    /// a screen whose parts act on touches and not on A takes no click of
    /// its own: the press over a part touches it there instead.
    #[test]
    fn a_touching_screen_is_not_clickable_and_still_outlines() {
        let idols = MENU_SCREENS.iter().position(|screen| screen.use_outlined == CLICK_TOUCHES).expect("a screen that touches");
        let mut mouse = clickable_menu();
        mouse.menu_screen = Some((idols, 0x1405_0000, 10));
        assert!(!mouse.menu_clickable(11));
        assert!(mouse.menu_touch(11, true).is_some(), "the press touches where it is");
        assert!(!mouse.menu_click);
        // a screen with A stays clickable
        mouse.menu_screen = Some((12, 0x1405_0000, 10));
        assert!(mouse.menu_clickable(11));
    }

    /// a practice screen's part queues the new one and applies it as a key
    /// does: its name at +0x33C and +0x374's bit 0, then the update's own
    /// step; one already outlined is left alone.
    #[test]
    fn practice_outlines_by_queueing_and_applying_it() {
        let mut system = System::new(crate::Config::default());
        let object = 0x0800_1000;
        let found = Found { hash: 0xADD1_FD65, name: 0x14B8_E528, element: 0, part: 0 };
        let calls = outline_practice(&mut system, object, found).expect("the queue and its step");
        assert_eq!(calls, vec![
            (OUTLINE_PRACTICE_PART, [object + PRACTICE_QUEUE, found.name, 0]),
            (APPLY_PRACTICE_QUEUE, [object, 0, 0]),
        ]);
        // the queue's slot holds what the update's step copies from
        assert_eq!(calls[0].1[0], object + PRACTICE_QUEUE);
        assert_eq!(calls[1].1[0], object);
    }

    /// the vault outlines the part a pointer finds with the same call the
    /// screen's own pointer handler makes, and A presses the hash it set.
    #[test]
    fn the_vault_outlines_with_its_pointer_call() {
        let mut system = System::new(crate::Config::default());
        let object = 0x0800_1000;
        let found = Found { hash: 0x0A9F_A648, name: 0x14B8_E528, element: 0, part: 0 };
        let calls = outline_vault(&mut system, object, found).expect("its outline call");
        assert_eq!(calls, vec![(OUTLINE_VAULT, [object, found.name, 1])]);
    }

    /// on a top screen 1.4 times wider, what is by an edge is that much
    /// narrower toward it, the rest toward the middle, and a touch finds it
    /// there.
    #[test]
    fn pause_menu_touches_follow_a_wider_top_screen() {
        let mut aim_mouse = AimMouse { pause_drawn: Some(10), edges_drawn: Some(10), widen: 1.4, ..AimMouse::default() };
        let near = |touch: Option<(f32, f32)>, x: f32| touch.is_some_and(|(at, _)| (at - x).abs() < 0.01);
        // Resume's 70 by the left edge drawn at 50, Retreat's 330 at 350
        assert!(near(aim_mouse.pause_menu_touch(11, 50.0, 83.0), 70.0));
        assert!(near(aim_mouse.pause_menu_touch(11, 350.0, 83.0), 250.0));
        // Options' middle stays in the middle
        assert!(near(aim_mouse.pause_menu_touch(11, 200.0, 134.0), 160.0));
        // and the gap between Resume and Retreat grew
        assert_eq!(aim_mouse.pause_menu_touch(11, 120.0, 83.0), None);
        aim_mouse.widen = 1.0;
        assert!(near(aim_mouse.pause_menu_touch(11, 120.0, 83.0), 120.0));
    }

    /// the mouse moving, onto a menu or off it, gives menus the mouse back
    /// from keys.
    #[test]
    fn hovering_gives_menus_the_mouse() {
        let mut aim_mouse = AimMouse::new(1.0, false, 0.0, false, false, true);
        assert!(!aim_mouse.mouse_mode);
        aim_mouse.hover(Some((90.0, 170.0)));
        assert!(aim_mouse.mouse_mode);
        assert_eq!(aim_mouse.pointer, Some((90.0, 170.0)));
        aim_mouse.mouse_mode = false;
        aim_mouse.hover(None);
        assert!(aim_mouse.mouse_mode);
        assert_eq!(aim_mouse.pointer, None);
    }

    /// in a battle until its UI stops being drawn, and not while paused.
    #[test]
    fn in_battle_while_its_ui_is_drawn() {
        let mut aim_mouse = AimMouse::default();
        assert!(!aim_mouse.in_battle(10));
        aim_mouse.battle_drawn = Some(10);
        assert!(aim_mouse.in_battle(13));
        assert!(!aim_mouse.in_battle(14));
        aim_mouse.pause_drawn = Some(10);
        assert!(!aim_mouse.in_battle(11));
    }
}
