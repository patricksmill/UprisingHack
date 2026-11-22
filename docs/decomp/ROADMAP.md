# Full source recovery and standalone port

Two deliverables, not interchangeable:

- Recovered game source: maintainable implementations, documented types and
  ownership, checked against the original. Lifted or recompiled C is evidence,
  not automatically recovered source.
- Standalone PC runtime: game code executes without guest CPU interpretation
  or a general-purpose emulator shell. Reusing a renderer or mixer does not
  itself prevent a standalone port; remaining platform emulation must be
  listed rather than hidden.

## Bulk source recovery

Maintained recovery now covers twelve functions. The latest two are the
hand-written packed-scalar and linked-factor routines in `kiu/overrides/scalar.c`,
checked with both core and float results; they are separate from the generated
bulk candidates below. Windows Recompile includes them, pinned to the
code hash they were checked against; a Windows library built with them
matches a same-tree reference in eight whole-game scripts and is installed.
Interactive Windows play with it is the remaining acceptance check.

`kiu/ghidra/LiftOverrides.java` translates Ghidra high p-code into C Overrides.
It emits variables and basic-block jumps, guest-memory accesses, and guest
calls. This is an experimental semantic lifting pipeline, not original
source reconstruction and not yet production code. Float operations are
supported; indirect calls and several other operations are rejected explicitly.

The maintained lifter suppresses synthetic memory self-copies, reloads
memory-backed phi inputs, and disables Ghidra's NaN-unsafe `trivialarith`
rule alongside `NanIgnoreEnum.None`. P-code diagnostics use the same
configuration. Full-batch comparisons, deliberately constructed hierarchy
fixtures, remaining return-register differences and the unpromoted
float-return contract experiment are recorded in `LEDGER.md`.

Verified mixed core/float results can now be supplied explicitly as the
fourth positional argument to `kiu/scripts/lift.py`, using
`kiu/ghidra/kiu-call-results.txt`. The file is bound to the executable hash;
it is not automatically applied; Windows Recompile includes only the
maintained `kiu/overrides/` files.
Use `REPLAY_VFP_RESULTS` and `DIFFTEST_VFP_RESULTS` for known float-result
contracts in addition to the default replay/random comparisons. General signature-order independence
remains unresolved; the rejected isolation experiment is recorded in the
ledger rather than enabled by default.

`kiu/scripts/lift.py` runs Ghidra, partitions the output into 48 translation
units, builds them in parallel using the existing system C compiler, and
executes `kiu/scripts/difftest.c` against the reference library. No generated
candidate is installed in `kiu/overrides/` or the Windows game library.

Run from the repository root:

```
python3 scripts/lift.py <hex-address-list> build/lift 200
```

The list contains one function address per line. Outputs are ignored under
`build/`: `lifted.c`, `report.txt`, `lifted.so`, `lifted.txt`, `check.txt`.

Verification states are intentionally separate:

- `same`: no observed difference among inputs on which the reference finished.
  Inspect the compared count: this can mean only one useful input.
- `r0-only`: unresolved return-value discrepancy. Not an automatic pass even
  if Ghidra declares the function void.
- `alias-only`: discrepancy with overlapping pointers. Not a pass; original
  callsites must prove aliasing impossible before discarding it.
- `DIFFERS`: observed discrepancy or candidate fault.
- `untested`: no usable inputs. No correctness claim.

Random pointer inputs do not represent valid game objects. The next required
step is capture/replay of actual function-entry state: arguments, reachable
memory, globals, TLS and service boundaries. Before promoting any batch,
compare actual calls, deliberately exercise boundaries and failure paths,
and execute the affected game scenarios. Float-return registers and inferred
calling conventions still require explicit verification. Random checks do
not establish semantic equivalence or completeness.

The comparison harness bounds each execution with a one-second alarm, catches
faults, compares copy-on-write game-image pages and a random arena, and reports
escaped reference runs separately. No candidate is executed if its reference
run did not finish. Batch exit status is nonzero for differences, unresolved
returns, alias differences or untested functions. The pipeline prints those
categories for diagnosis rather than treating its compilation as acceptance.

### Immediate sequence

1. Finish the bulk feasibility measurement and retain rejected cases.
2. Real-entry capture/replay exists (`kiutrace capture`, `kiu/scripts/replay.c`;
   results in `LEDGER.md`). Widen it to more states and frames.
3. Group failures by cause (input lifetime, SSA merge, calling convention,
   memory ordering, unsupported operations) and fix classes of failures.
4. Promote verified subsystem batches, not thousands of unnamed functions.
5. Recover types and ownership around complete gameplay paths. Generated
   candidates retain their experimental status until then.

## Standalone runtime

### Established boundaries

- `kiu/tools/3dsrecomp/src/port.rs:54-85` already generates a linked executable,
  but launches `zakuro::run(Some(code))`. Its own comment states that it still
  reads the game dump. This is packaging, not the finished native runtime.
- `crates/zakuro-core/src/recompiled.rs:254-317` reconstructs an
  ARM `Context`, dispatches code by guest PC, returns on SVC or instruction
  budget, and updates guest cycles. Static linking alone removes none of it.
- `crates/zakuro-core/src/kernel/svc.rs:33-109` owns SVC dispatch,
  including memory, thread creation, synchronization, ticks and IPC. The
  existing trace log includes thread, SVC number, arguments, PC and LR; use
  that before adding another tracing mechanism.
- `crates/zakuro-core/src/kernel/sync.rs:7-81` distinguishes
  one-shot, sticky and pulse events, recursive mutex ownership and counts,
  semaphores and timers. A host-thread migration must preserve these
  semantics, not substitute ordinary host mutexes indiscriminately.

These are source facts, not proof of which services the game invokes at
runtime. Exheader permissions only authorize access; they do not prove use.

### Port milestones and gates

1. **Executable baseline:** use the existing `3dsrecomp port` path with the
   current local Zakuro checkout; boot and run an air/ground scenario. Label
   the result emulator-backed. Keep the original asset dump user-supplied.
2. **Dependency audit:** collect actual SVC/service usage in boot, save/load,
   chapters and alternate modes; identify interpreted addresses and causes.
   Gate: a measured dependency report, including untested modes.
3. **Closed execution:** remove interpreter dependencies by recovering or
   correctly translating every observed gap. Add an acceptance mode that
   reports rather than silently accepting fallback. Gate: original-code
   fallback absent in exercised scenarios, with whole-program gaps listed.
4. **Thread/service boundary:** design resumable calls and synchronization
   before replacing the scheduler. Host threads are an option, not a safe
   mechanical substitution: memory aliasing, exclusive accesses, GPU shared
   memory, priorities, TLS and timing must be addressed. Gate: save/load,
   lock contention, event wakeups and chapter transitions remain correct.
5. **Host backends:** migrate asset/save I/O, input, timing and audio using
   existing project backends; then replace the GPU-facing game boundary.
   Reuse existing PICA translation during migration instead of writing a
   second renderer immediately. Gate: real display/audio/input verification,
   gameplay state comparisons and no unexplained unsupported path.
6. **Cutover:** remove obsolete production paths after all callers migrate.
   Rebuild and identify the Windows release and verify actual interaction.
   Until then the existing Windows executable is not a native-port release.

Do not stub unsupported network, AR or alternate-mode behavior and call the
result full. Record external service availability as a verification limit.
Do not reserve a flat 4 GiB address space or replace page tables merely on
an assumption that all guest mappings are linear.

## Coordination

Bulk lifting and port dependency research can proceed independently. Shared
ABI, memory and scheduler changes have one owner. No new dependencies or
frameworks are needed for the experiment. Parallel agents work on disjoint
subsystems only after interfaces and file ownership are settled.

For bounded autonomous recovery, use [OVERNIGHT.md](OVERNIGHT.md): subscription
routing, file ownership, verification gates, stop controls and morning handoff.

## Initial linked-executable baseline

This historical baseline used the six maintained overrides, not the
experimental lifted candidates. The boot-gap cutover below supersedes it:

- Baseline executable copy: `build/kiu-linked/baseline-executable` (807,461,608 bytes).
- Executable SHA-256:
  `ae8839c562b0e153bc7985f062e0a455aef2eb43b961ef6ce3321ea133d3afb4`.
- Static game archive: `build/kiu-linked/code/librecomp.a`.
- Archive SHA-256:
  `b4d115f0b20ae2136f36797772ad8c0f629833b46595b01c8155ad45d34b3693`.
- The executable is Linux x86-64 ELF, dynamically linked to host system
  libraries. Game code is statically linked; the entire executable is not
  statically linked.

Build commands, run from the repository root:

```
CC=$PWD/scripts/cc-fast tools/3dsrecomp/target/release/3dsrecomp port dumps/kiu.cci build/kiu-linked --name kiu-linked --zakuro .. --overrides overrides/
cp ../Cargo.lock build/kiu-linked/Cargo.lock
CARGO_TARGET_DIR=$PWD/../target cargo build --release --offline --manifest-path build/kiu-linked/Cargo.toml --config "patch.\"https://github.com/fearkov/3dsrecomp\".recomp-abi.path=\"$PWD/tools/3dsrecomp/abi\""
```

The copied lockfile is necessary: resolving the fresh workspace without it
failed offline on a yanked `clipboard-win` version through `arboard`.
The local ABI patch is necessary because a dependency's workspace patches
are not inherited by the new root workspace. No dependency versions were
upgraded to get this build working.

Archive generation completed with 527 compiled source files in 503.3
seconds; Cargo completed its release build in 302.51 seconds. The existing
`kiu/scripts/cc-fast` deliberately compiles the generated C without optimization.
This is a functional smoke artifact, not a performance-tuned release.

### Executed smoke

```
RUST_LOG=warn,zakuro_core::loader=info,cranelift_jit=warn ../target/release/kiu-linked dumps/kiu.cci --headless 1200 --profile --rasterizer software --mute --data "$PWD/trace/linked-audit/user"
```

Observed loader output:

```
running the recompiled code linked in, 448238 entry points, 0 modules
```

Observed final output:

```
Completed after 1200 frames in 33.21s
161.9 MIPS, frame 1200 | 19 threads | 0 modules | 15197 draws | 1768 transfers | 3538 fills | gpu 18.6s | interpreted 0.93%
```

A preceding 300-frame run also completed, reporting 0.40% interpreted.
These percentages are runtime accounting, not source-recovery percentages.
The frontend deliberately interprets hooked functions (`lib.rs:945-952`);
do not attribute all fallback to missing translations. The sampled hot PCs
included `0x005F55A0` and `0x005F6294`. Profiling samples host-step PCs,
including recompiled dispatch boundaries, so those samples alone do not
identify why a particular instruction was interpreted.

The run also reported unimplemented `cecd:u` commands `0x0012` and `0x0009`,
and `frd:u` command `0x001D` returning NOT_CONNECTED. These are unresolved
service dependencies, not reasons to add success stubs.

### Measured gameplay dependencies

Separately, the existing dynamic-library harness ran saved scenarios `s5`
and `s8` for 60 frames with hooks disabled and the existing SVC trace logger:

| SVC | Operation | s5 calls | s8 calls |
| --- | --- | --- | --- |
| 0x14 | ReleaseMutex | 30 | 60 |
| 0x18 | SignalEvent | 215 | 205 |
| 0x19 | ClearEvent | 326 | 385 |
| 0x1A | CreateTimer | 9 | 0 |
| 0x1B | SetTimer | 18 | 0 |
| 0x22 | ArbitrateAddress | 1,039 | 1,064 |
| 0x23 | CloseHandle | 9 | 0 |
| 0x25 | WaitSynchronizationN | 456 | 527 |
| 0x28 | GetSystemTick | 137 | 1,048 |
| 0x32 | SendSyncRequest | 932 | 1,285 |

The logs and scripts are in ignored `trace/linked-audit/`. These are actual
calls, not an exheader permission list. They cover only these scenarios;
boot and other modes can invoke additional services. They were not executed
inside the linked frontend and do not prove linked air/ground acceptance.

### Remaining acceptance limits

- A visible launch initialized OpenGL on llvmpipe, then lost its WSL display
  connection. An X11-only launch failed because `libxkbcommon-x11.so` could
  not be loaded. No package was installed to work around this.
- No linked-frontend air/ground interaction or screenshot has been verified.
  The headless CLI starts from boot and does not load a saved-state slot.
- Audio output was muted and has not been verified.
- Original executable assets are still read from the user's dump. Guest
  scheduling, IPC, memory, hardware translation and interpretation remain.
- At this initial baseline no Windows executable was produced or installed
  in `dist/`. The boot-gap cutover below records the later Windows update.

Fallback causes have since been classified and the dominant measured boot
gap translated. The source-recovery gate remains real-call capture/replay.

## Boot-gap cutover

The version-bound discovery hint and its code anchors are recorded in
`kiu/analysis/FINDINGS.md`, under “Recompiled boot continuation discovery.”
It translates existing ARM instructions; it does not modify the game dump
or increase the count of hand-recovered functions.

Acceptance used a fixed clock and fresh save-data directories. Hook-owning
functions remained disabled for compilation in both comparison runs, while
hook bodies were disabled in both to isolate translation behavior:

| Boot, 300 frames | Before | After |
| --- | --- | --- |
| Interpreter steps in the targeted gap | 11,490 | 0 |
| Interpreter steps outside all translated code pieces | 11,864 | 374 |
| Total outer interpreter steps | 175,761 | 164,053 |

Boot and two 60-frame saved-state scenarios produced identical executable
text, 39 MiB guest-memory snapshots, and both screen images before and after.
The two gameplay snapshots had unchanged interpreter counters. Evidence and
equality hashes are in `trace/boot-gap-cutover/receipt.json`.

### Current linked Linux artifact

- Executable: `../target/release/kiu-linked`, 807,499,008 bytes.
- SHA-256:
  `a8dfd03ecd5e6de08817d0d05850aa76006862de8e6085f36cd079a30b68f97b`.
- Verified archive: `build/boot-gap/librecomp.a`, SHA-256
  `262ee26235006783b61a18a4c03995ea568d9127f9a296f74d67c30f00da3f6b`.
- Project: `build/kiu-boot-gap`; its `code` symlink points to the verified
  generated code in `build/boot-gap`.

The candidate's existing objects were archived and linked, avoiding a
second full C compilation. The new root workspace was built with both local
dependency patches, which Cargo does not inherit from the Zakuro dependency:

```
CARGO_TARGET_DIR=$PWD/../target cargo build --release --offline --manifest-path build/kiu-boot-gap/Cargo.toml --config "patch.\"https://github.com/fearkov/3dsrecomp\".recomp-abi.path=\"$PWD/tools/3dsrecomp/abi\"" --config "patch.\"https://github.com/fearkov/3dsrecomp\".recomp3ds.path=\"$PWD/tools/3dsrecomp\""
```

Executed headless smoke:

```
running the recompiled code linked in, 448263 entry points, 0 modules
Completed after 300 frames in 4.79s
280.8 MIPS, frame 300 | 19 threads | 0 modules | 2147 draws | 418 transfers | 838 fills | gpu 2.1s | interpreted 0.40%
```

This enhanced-frontend smoke is separate from the controlled equivalence
runs. The rounded interpretation percentage includes hooks and scheduler
fallback; it is not a source-recovery percentage or a performance benchmark.

### Windows delivery

Zakuro's workspace now patches its existing `recomp3ds` dependency to the
local fork, alongside `recomp-abi`. This ensures the Recompile button uses
the same title-specific discovery hint as the local CLI.

The Windows release was rebuilt, copied to `dist/zakuro-kiu-mouse.exe`, and
compared byte-for-byte with the new build. Both have SHA-256:

```
4e27e3fdb29dbe3d2b1deb66c93eb49e7b71b0fec031e808a0cb01623a38d152
```

The distributed Windows executable was actually run through WSL interop:

```
dist/zakuro-kiu-mouse.exe dumps/kiu.cci --headless 60 --interpreter --rasterizer software --mute --data trace/boot-gap-windows-final/user
Completed after 60 frames in 3.45s
77.9 MIPS, frame 60 | 16 threads | 0 modules | 21 draws | 79 transfers | 160 fills | gpu 5.3ms
```

This is an interpreter-mode Windows runtime smoke, not proof of Windows
native execution through the new gap. The separately built Windows
recompiler also ran the hint-resolution command and emitted the expected
version-bound section. The native gap's behavior was verified on Linux.
Interactive Windows controls, graphics and audio remain unverified.

Use Recompile again to update an existing cached game library. Replacing
the frontend executable does not rewrite that cache. This remains an
emulator-backed Windows frontend, not a standalone native release.

The release build also required a narrow borrow-scope correction in
`App::emulate`: consume the settings-opening request while accessing the
game, then release pointer capture after that borrow ends.

### Checks and limits

- Recompiler: 56 unit tests and one documentation test passed.
- Frontend: 93 tests passed.
- Clippy completed with warnings in untouched `audio.rs` and `menus.rs`;
  these were not cleaned up as part of the boot-gap change.
- The recompiler-wide format check reports existing differences in Rust
  files untouched by this hint-only change; no unrelated reformat was applied.
- The original game dump hash is unchanged. The saved state's separate
  60 FPS patch is not altered by this discovery hint.
