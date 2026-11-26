//! Fades a script was a frame short of finishing, under the 60 FPS patch.
//!
//! Gimmicks run scripts of anime commands: waits, screen fades, effects.
//! The Seafloor Palace elevator's ride script waits, fades the screen to
//! black, waits, then fades it back in, while the stage unloads the area
//! below once Pit reaches the top, the elevator floor with it. The game
//! steps two frames an update, so the fade-in lands in the same update as
//! the arrival; the 60 FPS patch steps one, and the floor is deleted with
//! two frames of its last wait left. Its script goes with it and the screen
//! stays black for good (analysis/FINDINGS.md, "Script fades and the 60 FPS
//! patch").
//!
//! As a script is torn down, a fade-in that one unpatched update would
//! still have reached is applied the way the game's own fade command does.

use super::{read_word, System};

/// the clearing of a script runner's works (`0x0048C578`), as its owner
/// tears it down; r0 is the runner's works map, 0xC into the runner.
pub(super) const CLEAR_WORKS: u32 = 0x0048_C578;
/// the runner's current command, from its works map.
const CURRENT: u32 = 0x28;
/// a command's link to the next command's node; a node's first word is
/// its command.
const NEXT: u32 = 8;
/// the vtables of `anime_command::WaitFrame` and of the screen fade
/// command (its run is `0x00AEAD80`).
const WAIT_FRAME: u32 = 0x00D0_C9D0;
const FADE: u32 = 0x00D0_C8E0;
/// the works map's key for `anime_command::WaitFrame_Work`, and the frames
/// a work has left. A finished wait leaves NaN there.
const WAIT_WORK: u32 = 0x00D6_6E20;
const WAIT_LEFT: u32 = 0x24;
/// the fade command: its kind (0 white, 1 black, other kinds are colour
/// fades this leaves alone), slot and screen bytes, length in frames, and
/// whether it fades in.
const FADE_KIND: u32 = 0x10;
const FADE_SLOT: u32 = 0x11;
const FADE_SCREEN: u32 = 0x12;
const FADE_FRAMES: u32 = 0x14;
const FADE_IN: u32 = 0x18;
/// the screen fades singleton (`0x00538B34`): a fader per screen; in a
/// fader, 0x48 bytes a slot, the command's at 4 into it.
const FADES: u32 = 0x00D6_3620;
const FADERS: u32 = 0xB8;
const SLOT_SIZE: u32 = 0x48;
const COMMAND_SLOT: u32 = 4;
/// a fade's colour, change a frame and target in its slot
/// (`0x004641E0`), and the game's fade-in target.
const COLOR: u32 = 0x1C;
const SPEED: u32 = 0x4C;
const TARGET: u32 = 0x3C;
const FADED_IN: u32 = 0x0046_4210;
const WHITE: u32 = 0xFFFF_FF00;
/// the frames each update covers, 2.0 in the game and 1.0 with the 60 FPS
/// patch.
const STEP_FRAMES: u32 = 0x0082_8E30;
const NATIVE_STEP: f32 = 2.0;

/// at CLEAR_WORKS: a runner torn down on a wait one unpatched update would
/// have finished, a fade-in next, has the fade-in applied.
pub(super) fn finish_fade(system: &mut System) {
    let step = f32::from_bits(read_word(system, STEP_FRAMES));
    if !(step < NATIVE_STEP) {
        return;
    }
    let works = system.cpu.regs[0];
    let wait = read_word(system, works + CURRENT);
    if wait == 0 || read_word(system, wait) != WAIT_FRAME {
        return;
    }
    let next = read_word(system, wait + NEXT);
    let fade = if next == 0 { 0 } else { read_word(system, next) };
    if fade == 0 || read_word(system, fade) != FADE || read_byte(system, fade + FADE_IN) == 0 {
        return;
    }
    let Some(work) = find_work(system, works, WAIT_WORK) else { return };
    let left = f32::from_bits(read_word(system, work + WAIT_LEFT));
    if !(left <= NATIVE_STEP) {
        return;
    }
    let color = match read_byte(system, fade + FADE_KIND) {
        0 => WHITE,
        1 => 0,
        _ => return,
    };
    let frames = f32::from_bits(read_word(system, fade + FADE_FRAMES));
    let fades = read_word(system, FADES);
    if fades == 0 || !(frames > 0.0) {
        return;
    }
    let screen = u32::from(read_byte(system, fade + FADE_SCREEN));
    let fader = read_word(system, fades + FADERS + screen * 4);
    if fader == 0 {
        return;
    }
    let slot = fader + u32::from(read_byte(system, fade + FADE_SLOT)) * SLOT_SIZE + COMMAND_SLOT;
    let target = read_word(system, FADED_IN);
    system.memory.write_bytes(slot + COLOR, &color.to_le_bytes());
    system.memory.write_bytes(slot + SPEED, &(-1.0 / frames).to_le_bytes());
    system.memory.write_bytes(slot + TARGET, &target.to_le_bytes());
    log::info!("script {wait:08X} torn down {left} frames before its fade-in, faded in");
}

/// the work under key in a runner's works map, as `0x004C5E44` finds it:
/// a binary tree whose nodes hold the work 4 bytes before their first word,
/// a node's flag byte clear on the tree's ends.
fn find_work(system: &mut System, works: u32, key: u32) -> Option<u32> {
    let end = works + 4;
    let mut node = read_word(system, works + 8);
    let mut found = end;
    for _ in 0..64 {
        if node == 0 || read_byte(system, node + 0x10) == 0 {
            break;
        }
        if read_word(system, node + 0x14) < key {
            node = read_word(system, node + 0xC);
        } else {
            found = node;
            node = read_word(system, node + 8);
        }
    }
    (found != end && read_word(system, found + 0x14) == key).then(|| read_word(system, found).wrapping_sub(4))
}

fn read_byte(system: &mut System, at: u32) -> u8 {
    let mut byte = [0];
    system.memory.read_bytes(at, &mut byte);
    byte[0]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::{MemoryState, Permission};

    const HEAP: u32 = 0x1400_0000;
    const RUNNER: u32 = HEAP + 0x100;
    const WAIT: u32 = HEAP + 0x200;
    const FADE_COMMAND: u32 = HEAP + 0x300;
    const TREE: u32 = HEAP + 0x400;
    const WORK: u32 = HEAP + 0x500;
    const MANAGER: u32 = HEAP + 0x600;
    const FADER: u32 = HEAP + 0x700;

    fn write(system: &mut System, at: u32, word: u32) {
        system.memory.write_bytes(at, &word.to_le_bytes());
    }

    fn float(system: &mut System, at: u32) -> f32 {
        f32::from_bits(read_word(system, at))
    }

    /// the elevator's script torn down as the 60 FPS patch leaves it: on its
    /// last wait with `left` frames to go, a black fade-in of 30 frames next.
    fn torn_down(left: f32, step: f32) -> System {
        let mut system = System::new(crate::Config::default());
        system.memory.map(HEAP, 0x2000_0000, 0x1000, Permission::RW, MemoryState::Private);
        system.memory.map(0x0046_4000, 0x2001_0000, 0x1000, Permission::RW, MemoryState::Private);
        system.memory.map(0x0082_8000, 0x2002_0000, 0x1000, Permission::RW, MemoryState::Private);
        system.memory.map(0x00D6_3000, 0x2003_0000, 0x1000, Permission::RW, MemoryState::Private);
        write(&mut system, STEP_FRAMES, step.to_bits());
        write(&mut system, FADED_IN, 0);
        write(&mut system, FADES, MANAGER);
        write(&mut system, MANAGER + FADERS, FADER);
        let works = RUNNER + 0xC;
        write(&mut system, works + CURRENT, WAIT);
        write(&mut system, WAIT, WAIT_FRAME);
        write(&mut system, WAIT + NEXT, FADE_COMMAND + 4);
        write(&mut system, FADE_COMMAND + 4, FADE_COMMAND);
        write(&mut system, FADE_COMMAND, FADE);
        write(&mut system, FADE_COMMAND + FADE_KIND, 1);
        write(&mut system, FADE_COMMAND + FADE_FRAMES, 30.0f32.to_bits());
        write(&mut system, FADE_COMMAND + FADE_IN, 1);
        // one node, the wait's work, under the map's root; its children are
        // the map's end, whose flag byte is clear
        write(&mut system, works + 8, TREE);
        write(&mut system, TREE, WORK + 4);
        write(&mut system, TREE + 8, works + 4);
        write(&mut system, TREE + 0xC, works + 4);
        write(&mut system, TREE + 0x10, 1);
        write(&mut system, TREE + 0x14, WAIT_WORK);
        write(&mut system, WORK + WAIT_LEFT, left.to_bits());
        let slot = FADER + COMMAND_SLOT;
        write(&mut system, slot + TARGET, 1.0f32.to_bits());
        write(&mut system, slot + SPEED, (1.0f32 / 30.0).to_bits());
        system.cpu.regs[0] = works;
        system
    }

    #[test]
    fn a_fade_in_one_unpatched_update_away_is_applied() {
        let mut system = torn_down(2.0, 1.0);
        finish_fade(&mut system);
        let slot = FADER + COMMAND_SLOT;
        assert_eq!(float(&mut system, slot + TARGET), 0.0, "the screen fades back in");
        assert_eq!(float(&mut system, slot + SPEED), -1.0 / 30.0, "over the command's 30 frames");
        assert_eq!(read_word(&mut system, slot + COLOR), 0, "from black");
    }

    #[test]
    fn a_fade_in_further_away_is_left() {
        let mut system = torn_down(3.0, 1.0);
        finish_fade(&mut system);
        assert_eq!(float(&mut system, FADER + COMMAND_SLOT + TARGET), 1.0);
    }

    #[test]
    fn the_unpatched_game_is_left() {
        let mut system = torn_down(2.0, NATIVE_STEP);
        finish_fade(&mut system);
        assert_eq!(float(&mut system, FADER + COMMAND_SLOT + TARGET), 1.0);
    }

    #[test]
    fn a_fade_out_next_is_left() {
        let mut system = torn_down(2.0, 1.0);
        write(&mut system, FADE_COMMAND + FADE_IN, 0);
        finish_fade(&mut system);
        assert_eq!(float(&mut system, FADER + COMMAND_SLOT + TARGET), 1.0);
    }
}
