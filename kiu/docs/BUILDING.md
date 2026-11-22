# Building from source

This repository is a fork of [Zakuro](https://github.com/fearkov/zakuro),
with the port's own files in `kiu/`. The recompiler is the
[3dsrecomp fork](https://github.com/patricksmill/3dsrecomp) (branch `kiu`).
Cargo fetches it as a git dependency, at the commit `Cargo.lock` pins.

Commands here run from the repository root unless they say otherwise.

## 1. Install the prerequisites

All platforms need [git](https://git-scm.com/), stable Rust (1.95 or newer)
from [rustup](https://rustup.rs/), and a C compiler.

**Windows:** MinGW-w64 gcc (through MSYS2 or WinLibs) and CMake.

**Linux (Debian, Ubuntu, Mint):**

```sh
sudo apt install build-essential cmake pkg-config libasound2-dev libudev-dev
```

On Arch: `base-devel cmake pkgconf alsa-lib systemd-libs`.

**macOS:** `xcode-select --install`. For Vulkan, also
`brew install molten-vk vulkan-loader`. Without them it presents with
OpenGL and draws on the CPU. macOS is untested for this port.

## 2. Clone

```sh
git clone https://github.com/patricksmill/UprisingHack
cd UprisingHack
```

## 3. Build and run

You need your own **decrypted** dump of Kid Icarus: Uprising (USA,
`0004000000030100`), as `.cci` or `.3ds`. Place it at `kiu/dumps/kiu.cci`
(git ignores that folder), then build and open the library:

```sh
cargo run --release -p zakuro
```

The first build takes several minutes. The executable ends up in
`target/release/` (`zakuro.exe` on Windows). To launch the game directly,
pass the dump: `cargo run --release -p zakuro -- kiu/dumps/kiu.cci`.

## 4. Recompile the game

In the library, press **Choose folder…**, pick `kiu/dumps/`, and press
**Recompile** next to the game. It takes around ten minutes and needs a C
compiler. Zakuro uses gcc or clang when it finds one, and otherwise offers
to download Zig into its own folder. Recompile again after each update to
the port. Until then the game runs in the interpreter, well below full
speed.

## 5. Optional: the 60 FPS patch

The patch is generated from your own `code.bin`, after the script checks
that the words it changes are the original ones. From `kiu/`:

```sh
python3 scripts/extract_cci.py dumps/kiu.cci build/kiu
python3 scripts/make_60fps_ips.py build/kiu/code.bin build/code.ips
```

Copy `kiu/build/code.ips` into the game's mod folder,
`mods/0004000000030100/`, inside Zakuro's data folder
(`%APPDATA%\zakuro\` on Windows, `~/.local/share/zakuro/` on Linux). The
**Mods** button next to the game in the library opens it. Then recompile
the game, so the patched code runs at full speed.

## Updating

```sh
git pull
```

Then rebuild, and press **Recompile** again.

## Working on the recompiler

Clone the fork into `kiu/tools/3dsrecomp`, which git ignores, and build
against it by overriding the patch:

```sh
git clone -b kiu https://github.com/patricksmill/3dsrecomp kiu/tools/3dsrecomp
cargo build --release -p zakuro \
  --config 'patch."https://github.com/fearkov/3dsrecomp".recomp3ds.path="kiu/tools/3dsrecomp"' \
  --config 'patch."https://github.com/fearkov/3dsrecomp".recomp-abi.path="kiu/tools/3dsrecomp/abi"'
```

While the override is on, Cargo rewrites `Cargo.lock` to the local path:
don't commit it in that state (`git checkout Cargo.lock` restores it).

Scripts in `kiu/scripts/` that compile against `recomp.h` use that clone
when it exists (see `kiu/scripts/recomp_abi.py`).

To ship a recompiler change, push it to the fork's `kiu` branch, then move
the pin here:

```sh
cargo update -p recomp3ds -p recomp-abi
```

and commit `Cargo.lock`.
