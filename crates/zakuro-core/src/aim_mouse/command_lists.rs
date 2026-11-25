//! Room in the game's GPU command lists for the menus' top-screen replays.
//!
//! The game draws a frame into one of two command lists, each with a
//! 0x4B000-byte buffer it sizes at startup (`0x001018E8`) from its own heap.
//! Every layer the port replays on the top screen writes its drawing into
//! the same list, and a Screen split into many layers (Fuse Weapons' 14 top
//! layers, its question and flash) runs past the buffer's end: the game
//! does not check, the list never finishes, and the main thread waits on
//! its running flag (`GX_STATE + 0x11`, in `0x00168EFC`) for good. The heap
//! has about 300 KB to spare at boot, which chapters need later, so the
//! lists get their buffers from FCRAM outside the game's 64 MiB instead,
//! through the linear heap window the GPU reads, before the game first
//! clears each one. Save states keep the mapping and the allocation.

use crate::memory::{MemoryRegion, MemoryState, Permission};
use crate::System;

/// the game's clear of the bound command list (`nngxClearCmdlist`): it
/// takes the list's buffer and size afresh into its write pointers.
pub(super) const CLEAR_COMMAND_LIST: u32 = 0x0016_88E0;
/// the GPU library's state: the bound command list at +0x9C.
const GX_STATE: u32 = 0x00DB_C4F8;
const BOUND_LIST: u32 = 0x9C;
/// a command list's buffer and its size in bytes.
const LIST_BUFFER: u32 = 4;
const LIST_SIZE: u32 = 8;
/// the size the game gives each list.
const NATIVE_SIZE: u32 = 0x4_B000;
/// the size the lists get: room for every layer of the busiest Screen and
/// more, two of them in the 32 MiB SYSTEM region the game never uses.
const ROOMY_SIZE: u32 = 0x10_0000;

fn read_word(system: &mut System, at: u32) -> u32 {
    let mut word = [0; 4];
    system.memory.read_bytes(at, &mut word);
    u32::from_le_bytes(word)
}

/// at CLEAR_COMMAND_LIST: a list still on its native buffer moves to a
/// roomier one before the game takes its buffer and size.
pub(super) fn make_room(system: &mut System) {
    let list = read_word(system, GX_STATE + BOUND_LIST);
    if list == 0 || !system.memory.is_readable(list, LIST_SIZE + 4) || read_word(system, list + LIST_SIZE) != NATIVE_SIZE {
        return;
    }
    let Some(block) = system.memory.phys.allocate(MemoryRegion::System, ROOMY_SIZE) else {
        log::warn!("no room outside the game's memory for a roomier command list");
        return;
    };
    let vaddr = system.kernel.linear_base + (block.addr - zakuro_common::memory_map::FCRAM_PADDR);
    system.memory.map(vaddr, block.addr, ROOMY_SIZE, Permission::RW, MemoryState::Continuous);
    system.memory.write_bytes(list + LIST_BUFFER, &vaddr.to_le_bytes());
    system.memory.write_bytes(list + LIST_SIZE, &ROOMY_SIZE.to_le_bytes());
    log::info!("command list {list:08X} has {ROOMY_SIZE:#X} bytes at {vaddr:08X}");
}
