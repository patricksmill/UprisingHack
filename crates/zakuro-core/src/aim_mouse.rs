//! mouse aiming and a smaller HUD for Kid Icarus: Uprising (USA), a test of
//! what a port's overrides will do: the mouse's movement goes straight into
//! the aim delta the game's touch handling produces, with none of its
//! smoothing or flick, and the top screen's HUD and reticle are drawn
//! smaller. only the interpreter stops where this looks, so the functions
//! these places are in run in it, see FUNCTIONS.

use crate::System;

/// Kid Icarus: Uprising, the USA cartridge.
pub const PROGRAM_ID: u64 = 0x0004_0000_0003_0100;

/// the places below, which the interpreter has to run for it to stop at
/// them: every function whose code takes one in runs in it, the rest of the
/// game stays recompiled. one place a function does, the pause menu's top
/// places are in the top pass TOP_UI_AFTER_HUD is in, and the dialogs'
/// in one function, their bottom sites in another, and the paused input's
/// in one.
pub const PLACES: [u32; 21] = [
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
    PAUSE_KEY,
    PAUSE_OPTION_FOUND,
    OPTIONS_OPTION_FOUND,
    DIALOG_INPUT,
    CAULDRON[0],
    CAULDRON[1],
];

/// the Fiend's Cauldron's pot (a task, vtable 0x00CF9AEC), the update it
/// runs each frame while the bar is shown, then while it pours after Move
/// Out. it runs on that screen alone, not on the chapter select before it,
/// the next screen or in a battle.
const CAULDRON: [u32; 2] = [0x00251624, 0x00251C40];

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

/// in the top screen's battle UI pass, 0x0060D39C, just after it drew the
/// health bar: the narration is drawn there too.
const TOP_UI_AFTER_HUD: u32 = 0x0060D60C;

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
const MIRRORED: [Mirrored; 4] = [
    Mirrored { site: 0x00A181CC, from: [320.0, 0.0], to: [400.0, 0.0], below: 40.0, size: 1.0, swap_rows: None, hidden: &[0x00A192E4], last: None },
    Mirrored { site: 0x00A18400, from: [0.0, 194.0], to: [0.0, 0.0], below: 36.0, size: 0.75, swap_rows: None, hidden: &[], last: None },
    Mirrored { site: 0x00A18430, from: [0.0, 194.0], to: [0.0, 0.0], below: 36.0, size: 0.75, swap_rows: None, hidden: &[], last: None },
    Mirrored { site: 0x00A18368, from: [162.0, 118.0], to: [200.0, 0.0], below: 8.0, size: 0.75, swap_rows: None, hidden: &[0x009EAC80],
        last: Some(Apart { from: [289.0, 209.0], to: [4.0, 0.0], below: 76.0, size: 1.5 }) },
];

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
#[derive(Clone, Copy, PartialEq)]
enum Drawing {
    Portraits,
    Mirrored(usize),
    Narration,
    PauseMenu(usize),
}

impl Drawing {
    /// where in the top pass it is drawn, and comes back to.
    fn place(self) -> u32 {
        match self {
            Drawing::PauseMenu(index) => PAUSE_MENU[index].place,
            _ => TOP_UI_AFTER_HUD,
        }
    }
}

/// what the CPU held where the call was made in the game's place, put back
/// when the call returns there, and the narration's position while it is
/// moved.
struct Saved {
    regs: [u32; 16],
    cpsr: zakuro_cpu::psr::Psr,
    vfp: [u32; 32],
    drawing: Drawing,
    moved: Option<(u32, [u8; 8])>,
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
    /// whether the bottom screen's hearts, item box, power icon and pickups
    /// are drawn on the top screen too.
    pub mirror_bottom: bool,
    /// the mirrored elements' calls from the last bottom pass, by MIRRORED's
    /// order.
    recorded: [Option<Recorded>; 4],
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
    /// the frame the power key was pressed in, for the battle UI's next
    /// update to use the current power, unless that is long after.
    power_requested: Option<u64>,
    /// whether the mouse over a menu's options lights the one under it, as
    /// on a PC, without using it, and over none lights none. keys bring the
    /// game's own cursor back until the mouse moves again.
    pub menu_hover: bool,
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
            recorded: [None; 4],
            pause_on_top: false,
            pause: [None; 15],
            pause_drawn: None,
            edges_drawn: None,
            battle_drawn: None,
            cauldron_drawn: None,
            cauldron_pouring: false,
            power_requested: None,
            menu_hover: false,
            pointer: None,
            mouse_mode: false,
            paused: None,
            cleared: None,
            hovering: None,
            dialog_cleared: false,
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
    match pc {
        AFTER_AIM_DELTA => add_delta(system),
        BEFORE_CAMERA_TURN => close_box(system),
        PROJECTION => widen_view(system),
        PASS_BEGIN => {
            let (context, screen) = (system.cpu.regs[0], system.cpu.regs[1]);
            if let Some(aim_mouse) = &mut system.aim_mouse {
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
        _ if CAULDRON.contains(&pc) => {
            let frame = system.frames;
            if let Some(aim_mouse) = &mut system.aim_mouse {
                aim_mouse.cauldron_drawn = Some(frame);
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
    if let Some(back) = aim_mouse.saved.take() {
        system.cpu.regs = back.regs;
        system.cpu.cpsr = back.cpsr;
        system.cpu.vfp.regs = back.vfp;
        if let Some((message, position)) = back.moved {
            system.memory.write_bytes(message + 0x14, &position);
        }
        let next = match back.drawing {
            Drawing::Portraits => 0,
            Drawing::Mirrored(index) => index + 1,
            Drawing::Narration | Drawing::PauseMenu(_) => return,
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
    let Some(index) = MIRRORED.iter().position(|mirrored| mirrored.site == site) else { return };
    let Some(recorded) = recorded_call(system, site) else { return };
    if let Some(aim_mouse) = &mut system.aim_mouse {
        aim_mouse.recorded[index] = Some(recorded);
    }
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
    call_here(system, recorded.target, recorded.regs[0], recorded.regs[1], Drawing::PauseMenu(index), None);
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

/// starts drawing the first mirrored element from index on that the last
/// bottom pass drew, whether there was one. each is drawn once: the bottom
/// pass records it again when it draws it again.
fn start_mirrored(system: &mut System, from: usize) -> bool {
    let Some(aim_mouse) = &mut system.aim_mouse else { return false };
    if !aim_mouse.mirror_bottom {
        return false;
    }
    for index in from..MIRRORED.len() {
        let Some(recorded) = aim_mouse.recorded[index].take() else { continue };
        call_here(system, recorded.target, recorded.regs[0], recorded.regs[1], Drawing::Mirrored(index), None);
        system.cpu.regs[2] = recorded.regs[2];
        system.cpu.regs[3] = recorded.regs[3];
        system.cpu.vfp.regs[..8].copy_from_slice(&recorded.floats);
        return true;
    }
    false
}

/// has the game's code run function as if called from here with r0 and r1,
/// coming back here, the CPU kept to be put back then.
fn call_here(system: &mut System, function: u32, r0: u32, r1: u32, drawing: Drawing, moved: Option<(u32, [u8; 8])>) {
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
    call_here(system, DRAW_PORTRAITS, portraits, context, Drawing::Portraits, None);
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
    let position: [u8; 8] = head[0x14..0x1C].try_into().unwrap();
    let mut moved = [0u8; 8];
    moved[..4].copy_from_slice(&((400.0 - width) * 0.5).to_le_bytes());
    moved[4..].copy_from_slice(&(240.0 - height - 8.0).to_le_bytes());
    system.memory.write_bytes(message + 0x14, &moved);
    call_here(system, DRAW_MESSAGE_LINE, message, context, Drawing::Narration, Some((message, position)));
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

/// whether a sprite batch flushed from caller draws the top screen's HUD,
/// which the narration's line is part of while it is drawn there.
fn top_hud(system: &System, caller: u32) -> bool {
    TOP_HUD.contains(&caller) || drawing(system) == Some(Drawing::Narration)
}

/// draws a top screen HUD element smaller: the quads the flush is about to
/// draw, scaled together toward the edge or corner they sit by, or their
/// own middle when they are by none, like a marker on an enemy.
fn shrink_hud(system: &mut System) {
    let widen = widening(system);
    match drawing(system) {
        Some(Drawing::Portraits) => return place_portraits(system, widen),
        Some(Drawing::Mirrored(index)) => return place_mirrored(system, &MIRRORED[index], widen),
        Some(Drawing::PauseMenu(_)) => return place_pause_menu(system, widen),
        _ => {}
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
