# Recovery ledger

What of the game's code has been recovered as maintained source, against
what, and how each piece was checked. Code anchors for behaviour stay in
`kiu/analysis/FINDINGS.md`; this file counts and proves.

## Reference

| Item | Value |
| --- | --- |
| Title | Kid Icarus: Uprising, USA, 0004000000030100 |
| `dumps/exefs/code.bin` SHA-256 | `2718aa2077a29560208c4e499288e5fecc4446d7adec95efb91ebf1b19eda679` |
| `dumps/exheader.bin` SHA-256 | `fca5249c445b87169f2738ca00e9cd0fc810abcb9ea7f609dd9d0055209e61e2` |
| text / rodata / data | `0x00100000` + `0xB950F0`, `0x00C96000` + `0x9B990`, `0x00D32000` + `0x31C78`, bss `0x1D20B0` |
| Zakuro | `487cfd352ada09daa8b64a4a89131e33968c0379` (`kiu-mouse`) |
| 3dsrecomp | `4545dc15dc1147183077cf87305513582de614cb` (`kiu`) |
| Reference library | `build/lib/0004000000030100.so`, no overrides |

The reference is `dumps/kiu.cci` as dumped, run by `kiutrace` with
`KIUTRACE_NO_HOOKS=1` and no hook commands in the script.

## Denominator

| Count | Value | Source |
| --- | --- | --- |
| Functions, 3dsrecomp | 49,236 (1,062 Thumb) | `kiu/analysis/recomp_analyze.txt`, `kiu/analysis/functions.txt` |
| Functions, Ghidra | 51,409 | `kiu/analysis/all.c` |
| Text classified | 93.6% code, 4.2% literals, 2.2% unreached | `kiu/analysis/recomp_analyze.txt` |
| Indirect calls / switches / svcs / dead ends | 16,396 / 541 / 646 / 944 | same |
| Modules (CRO) | 0 | same |
| Functions run by the 55 saved states, 60 frames each | 6,018 (12.2%) | `ran 60` on every `trace/*.state` |

The two function counts differ because 3dsrecomp folds a function reached
only by a tail branch into its caller as a label (`0x00919EF4` inside
`0x00A4245C` is one). Until the lists are reconciled, progress is counted
against 3dsrecomp's list, and a recovered function that was only a label
there is noted as such.

Open: the 2.2% unreached text and the 944 dead ends are unclassified.

### What can be recovered now

A recovered function runs to its end once entered (see Method). It cannot
do that if anything it reaches can block the thread: an svc, an indirect
call whose target is unknown, or code left to the interpreter. Over the
48,173 functions in the reference library's generated code, following
every call 3dsrecomp resolved:

| Class | Functions | Run by the saved states |
| --- | --- | --- |
| Closed: reach no svc, indirect call or interpreted code | 26,860 (14,612 call nothing) | 3,239 |
| Open: reach at least one | 21,313 | 2,702 |
| Containing an svc / an indirect call / interpreted code | 531 / 7,836 / 44 | |

Indirect calls are counted as blocking without looking at their targets,
so the open class is an upper bound; most are virtual calls. Open functions
wait until guest threads run on host threads (plan phase 5) or until their
indirect targets are resolved and closed.

## Method

A function counts as recovered when all of these hold:

1. It is written by hand as a C Override in `kiu/overrides/`, with the fields it
   touches named from evidence.
2. `kiu/scripts/difftest.c` finds no difference from the generated code over
   random registers and memory, aliased and unaligned pointers included. It
   compares memory, the live stack, `r0`, `r4`-`r11`, `sp`, the return, the
   exit reason and the instructions charged. `r1`-`r3`, `r12`, the flags
   and stack below the returned `sp` are the caller's to lose under the ARM
   calling convention and are not compared. Random words rarely agree for
   long or point anywhere, so `DIFFTEST_SHAPE=1` gives `r1`'s memory the
   first bytes of `r0`'s for functions that compare, and `DIFFTEST_SHAPE=2`
   makes an eighth of the words pointers for functions that follow one.
3. A library built with it runs named saved states with input to the same
   game state and screenshots as the reference library. Differences are
   allowed only in dead stack and in timing words, and each is explained.

The standard is functional, not instruction-exact (decided 13 November).
Zakuro runs a thread until its instruction budget runs out, possibly
partway through a function, and resumes it at the label it stopped at,
in generated code that expects the original's registers and stack frame.
Hand-written code does not keep those, so a recovered function charges the
instructions its generated code would have (`BUDGET_ATOMIC`, in
`kiu/tools/3dsrecomp/abi/recomp.h`) and only yields on the way in. When the
budget runs out partway, the thread overruns its slice instead of stopping,
and Zakuro does not count the overrun as cycles (`recompiled.rs`, `ran`
uses the budget clamped at zero). Thread timing can therefore shift, and
words derived from it (ticks) can differ in whole runs.

Known nondeterminism: the interpreter and the reference library end
`trace/s1.state` + 120 frames with 9 different words in bss (`0x00E0B9C8`,
`0x00E0BA80`, `0x00E0BA88`, `0x00EF9F30`, `0x00F2F358`, `0x00F35D00`-`18`).
`0x00F35D00`-`18` hold four 64-bit values of about the same size as each
other, and the runs that differ there differ by 3 to 40: they are tick
values. The others are not confirmed. Two interpreter runs are
byte-identical, and so are two runs of the reference library: eight states,
120 frames with input each, on 13 and 14 November.

Commands:

```
CC=$PWD/scripts/cc-fast tools/3dsrecomp/target/release/3dsrecomp build dumps/kiu.cci build/decomp --overrides overrides/
cc -O1 -I "$(python3 scripts/recomp_abi.py)" -o build/difftest scripts/difftest.c -Wl,--no-as-needed -lm -ldl
build/difftest build/lib/0004000000030100.so build/decomp/0004000000030100.so <address> <pointers> [runs] [via] [size limit]
KIUTRACE_LIB=build/decomp/0004000000030100.so KIUTRACE_NO_HOOKS=1 tools/kiutrace/target/release/kiutrace dumps/kiu.cci <script> <out>
```

## Bulk-lifting experiment

The dual-track plan, acceptance gates, and exercised Linux linked-executable
milestone are in [ROADMAP.md](ROADMAP.md). The linked executable includes
only the six overrides maintained when it was built. No lifted candidate has been
promoted to recovered source.

Command executed:

```
python3 scripts/lift.py build/lift/list.txt build/lift 200
```

The address list contains the 3,239 exercised functions previously classified
as closed. Output after input-lifetime and dispatch fixes:

| Result | Functions |
| --- | --- |
| No observed discrepancy on comparable random inputs | 1,579 |
| Observed discrepancy | 191 |
| No comparable input | 564 |
| Unresolved r0 discrepancy | 214 |
| Discrepancy with aliased pointers | 15 |
| Unsupported by lifter / skipped | 676 |

Elapsed pipeline run: 215.85 seconds. Each function had 200 attempted inputs;
reference faults and non-completing executions are not comparisons. The
first row is a feasibility result, not proof of recovery or a new progress
count. Actual-call capture/replay and gameplay integration remain required.
Compiled candidates and comparison artifacts remain under ignored `build/lift/`.

### Real-entry replay

`kiutrace`'s `capture` command, run in the interpreter, stops as each listed
function is entered and writes its registers and every non-I/O mapped page;
pages shared between captures are written once. `kiu/scripts/replay.c` maps a
capture copy-on-write, runs the reference and the candidate from it, and
compares everything written but the dead stack below the entry's `sp`,
`r4`-`r11`, `sp`, the return, `s16`-`s31` and `fpscr`, with `r0` apart.

```
# script: load trace/s5.state / capture trace/capture/targets.txt 2 1 trace/capture/s5
tools/kiutrace/target/release/kiutrace dumps/kiu.cci <script> <out>
cc -O1 -I "$(python3 scripts/recomp_abi.py)" -o build/replay scripts/replay.c -Wl,--no-as-needed -lm -ldl
build/replay build/lib/0004000000030100.so build/lift/lifted.so trace/capture/s5 trace/capture/s8
```

One frame each of `s5` (air) and `s8` (land), up to two entries per lifted
function: 1,763 captures, 1.6 GB under ignored `trace/capture/`, about 100
seconds of capture per state and 22 seconds of replay. The reference library
against itself gives no difference (`s5`: 533 same, 3 not comparable).

| Real entries | Functions |
| --- | --- |
| No difference | 553 |
| Differ only in `r0` | 108 |
| Differ | 51 |
| Not comparable | 3 |

Against the random-input verdicts: 438 functions are clean in both; 80 the
random check could not test are clean on real entries; 4 clean on random
inputs differ on real ones, and 8 clean on random inputs differ in `r0`.
One of the four, `0x00389808`, was put down to the calling convention here;
that was wrong. Its lifted call passes what the original passes, and it is
the same on real entries once the harness runs the reference to its end
(see the correction below).

No lifted candidate is promoted on these results: two frames do not cover
the paths these functions take elsewhere, and `r0` differences stay open
until callers are shown not to read them.

The maintained overrides replayed the same way (`build/decomp` library as
the candidate) show no difference on any capture: `0x00919E90` (3 entries),
`0x00919ED0` and `0x00919F7C` (2 each). The capture list held no entries for
`0x00425F00`, `0x00919EF4` or `0x00919F38`.

### Float operations and prototype experiments

`kiu/ghidra/LiftOverrides.java` now lifts p-code float operations the way the
generated VFP code computes them (plain C on the values, results flushed to
zero when `fpscr` asks; `kiu/ghidra/lifted.h`). Both harnesses now leave
`fpscr`'s condition flags, QC and cumulative exception bits out of the
comparison: the AAPCS does not preserve them across calls, and lifted
comparisons do not set them.

Same 3,239 functions, same captures:

| Configuration | Lifted | Random: same | Real: same | Real: differ | Real: `r0` only |
| --- | --- | --- | --- | --- | --- |
| Before float support | 2,563 | 1,579 | 553 | 51 | 108 |
| Float support (kept) | 2,876 | 1,695 | 561 | 37 | 114 |
| Float support, re-imported as ARMv7 (hard-float prototypes) | 2,841 | 1,608 | 524 | 46 | 135 |
| Float support, Decompiler Parameter ID run over the project | 2,890 | 1,266 | 550 | 29 | 131 |

The two prototype experiments did not pay: the ARMv7 import loses functions
on both checks, and Parameter ID fixes some real-entry differences while
tripling random-input ones (246 to 657). Their projects were deleted; the
lifter stays on `kiu/ghidra/project`. Recreating them: re-import with
`-processor ARM:LE:32:v7 -cspec default`, or re-analyze a copy with the
"Decompiler Parameter ID" analysis option on and `GHIDRA_HEADLESS_MAXMEM=7G`
(the default 2 GB heap runs out).

`kiu/scripts/r0_live.py` reads a replay's `r0`-only functions and checks, in
the generated C after every direct call, whether the caller reads `r0`
before writing it. Of the 114: 16 have a caller that reads it (a return
value Ghidra missed: real bugs), 31 have every direct caller overwrite it
first, and 67 are undetermined (flow leaves the block, or only indirect
callers). The 31 are not promoted on this alone: callers through pointers
are not seen.

### Harness correction, return values and wider captures (14 November)

Generated code hands back to the host not only when a function returns but
whenever it jumps somewhere its C function does not hold: an indirect
call's target, and the return from one. `difftest` and `replay` took the
first hand-back for the end, so a reference that made an indirect call was
compared half-run against a candidate that had finished (`0x00389808`
"returned" to `0x00421FD8`, inside the function making the indirect call,
with `sp` below where it entered). Both now go on from the hand-back with
the reference's code until the function is back at its caller, as Zakuro
does; a hand-back to an address the reference has no code for is not
compared. `REPLAY_ONLY=<address>` replays one function and prints the bytes
and registers that differ.

The lifter now also lifts a function Ghidra calls void a second time with a
return value, to see what the machine leaves in `r0`, and keeps that lift
when the stack frame and every call's arguments stay as they were
(`kiu/ghidra/LiftOverrides.java`, "Observe machine return").

Captures were widened from 2 sets to 10: `s1` over two frames; `exo-tank`,
`s5` and `s8` after 20 frames and with input, three entries a function
over three frames; and the 313 functions float support added, in all four
states. 8,887 entries of 1,353
functions, 7.9 GB under ignored `trace/`; the reference against itself
differs in none (61 entries not comparable).

| Same 2,876 lifted functions | Random: same | Random: differ | Random: `r0` only | Random: untested | Real: same | Real: differ | Real: `r0` only |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Float support, old harness | 1,695 | 246 | 295 | 614 | | | |
| Float support, corrected harness | 1,722 | 151 | 305 | 676 | 1,013 | 107 | 227 |
| With observed returns, old harness | 1,921 | 246 | 70 | 614 | 1,188 | 112 | 50 |
| With observed returns, corrected harness | 1,956 | 151 | 72 | 676 | 1,195 | 102 | 50 |

95 of the 246 random-input differences and 10 of the 112 real-entry ones
were the harness, not the lift; 62 functions the old harness compared
half-run are now untested on random inputs. The real-entry columns of the
first table above predate both the correction and the wider captures and
are not comparable with these.

Of the 102 that still differ on real entries, the first difference is in
memory for 68, 25 leave the captured memory, 8 trap and 1 returns with
other saved registers. Their causes are in the next section.
`kiu/scripts/r0_live.py` on the 50 `r0`-only: 14 have a direct caller that
reads `r0`, 15 have every direct caller overwrite it, 21 are undetermined.
Outputs are under ignored `build/continue/`; the overnight round's own
logs and receipts are under `build/overnight/round-001/`.

### Why the 102 differ

Found by comparing calls. `build/continue/traced/` is the reference library
built again with a `recomp.h` whose `CALL` reports each call
(`recomp_trace_call`); with `REPLAY_CALLS=<dir>`, `replay` (linked with
`-rdynamic`) writes the calls the reference and the candidate each made,
with `r0`-`r3`, `s0`-`s3` and `sp`. `build/continue/calls_diverge.py`
finds the first call that goes elsewhere or takes another value in a
register the callee reads before writing, from the generated C.
`REPLAY_SHOW=1` prints the bytes and registers that differ, and
`kiu/ghidra/DumpPcode.java` the high p-code a function is lifted from.

Three causes were faults in the lifter, shown by fixing each and lifting
again; 30 of the 102 no longer differ and none that was the same does:

| Fault, fixed in `kiu/ghidra/LiftOverrides.java` | Now same | Now `r0` only |
| --- | --- | --- |
| A value printed where it is used was not cut to its size: a byte flag Ghidra reads as a word was tested whole (`0x00473DA4` and `0x0091E1F0` never left their loops) | 4 | |
| A value Ghidra keeps in a register until a call and then lands in a stack slot (an `INDIRECT` into the stack) was never written, so structures built on the stack for a callee were empty (`0x002F3610`, `0x00352A9C`) | 19 | 4 |
| The frame was Ghidra's, which can be smaller than the locals the function names, so callees wrote over them (`0x0052036C`: frame 0, locals down to `sp-0x48`) | 3 | |

The 72 left, by the first thing that goes wrong:

| Cause | Functions |
| --- | --- |
| A float argument is not passed: Ghidra's convention for this program passes nothing in `s0`-`s15`, and no lifted call of these passes one | 28 |
| The same, with a core argument missing as well | 3 |
| `s0`-`s3` differ at the first call, but whether the callee reads them was not settled | 4 |
| A float result is not taken: Ghidra has `s0` survive a call, so the lifted code goes on with the value from before it (`0x00BAFB70`) | 5 |
| A core argument is not passed: Ghidra infers arguments at each call, and passes fewer than the callee reads (`0x001B0B50` passes five to `0x001E7760` at one call and three at the next; `0x00112550` passes none) | 17 |
| The same, callee's use not settled | 2 |
| Arguments on the stack, copied through from the caller's, not passed (`0x00B8C5A8`) | 1 |
| The callee is marked as not returning, so the lift ends at the call (`0x00259908`, callee `0x001B2D80`, which ends in `bx lr`) | 1 |
| Not a function: `0x00BB0900` is the two instructions that end its neighbours (`vpop {d8}; pop {r4, pc}`) | 1 |
| Not determined | 10 |

All but the last three rows are one thing: the lifted call takes its
arguments and result from Ghidra's guess at the callee's prototype, not
from what the callee reads and writes. 60 of the 72. The two prototype
experiments above tried to mend that in Ghidra and lost more than they
won; the lifter can instead take each callee's inputs from the callee
(its own high p-code, or the generated C as `calls_diverge.py` reads it)
and after a call read `s0`-`s15` back from the machine.

Not determined: `0x00637F5C`, `0x00A3F3F0`, `0x00B421A4` and `0x00B66020`
make the same calls with the same registers and differ after;
`0x0049B79C` makes none of the reference's; `0x00558D18` and `0x007EBBB8`
call something else at their fourth and second call; `0x00C8E0E4` first
differs inside a callee; `0x003B5B94` and `0x00B45F04` pass a core
argument with another value.

Same 3,239 functions with the three fixes (`build/continue/lift-frame/`;
`build/lift/` still holds the lift from before them):

| | Lifted | Random: same | Random: differ | Random: `r0` only | Random: untested | Real: same | Real: differ | Real: `r0` only |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Before | 2,876 | 1,956 | 151 | 72 | 676 | 1,195 | 102 | 50 |
| After | 2,885 | 2,001 | 128 | 68 | 673 | 1,225 | 72 | 50 |

Real entries: 26 that differed and 4 that differed only in `r0` are now
the same, and 4 that differed now differ only in `r0`. Per-function
verdicts and first differences are in `build/continue/frame-all.txt`,
`frame-show.txt` and `diverge3.txt`.

### Four unresolved functions traced to causes (14 November, continuation)

The four previously undetermined cases with agreeing call traces now have
passing isolated probes. Anchors and the original-instruction/high-p-code
evidence are in `kiu/analysis/FINDINGS.md`, under “Lifted-code discrepancy
probes.” These are experimental candidates under `build/re-next/`, with
no production override or recovered-source promotion.

| Function | Cause isolated | Baseline real entries | Probe real entries |
| --- | --- | --- | --- |
| `0x00B66020` | Synthetic memory self-copies emitted as real accesses, including an unconditional access to unmapped `0x88` at return | 0 same / 8 | 8 same / 8 |
| `0x00B421A4` | The float result of a call replaced by the incoming `s0` | 1 same / 2 | 2 same / 2 |
| `0x00A3F3F0` | Stack locals initialized before a callee writes them, then not refreshed across a storage-changing phi copy | 0 same / 2 | 2 same / 2 |
| `0x00637F5C` | Ghidra folds self-comparisons used to detect NaN fallback values | 3 same / 6 | 6 same / 6 |

The bookkeeping-copy experiment removed 109,350 identical-address,
identical-size literal-memory self-copies from the existing 2,885-function
lift. Replaying the same ten capture sets, 1,353 functions: 1,226 same,
71 differ, 50 `r0`-only, 6 untested. Only `0x00B66020` changed verdict
(previously 1,225 same and 72 differ); every other function's comparison
counts and verdict were unchanged. Exit status remains nonzero because
the batch still contains discrepancies. This is a broad replay check of
an experimental output transformation, not a committed lifter repair or
a random-input acceptance result.

The other three probes change only their named function. Passing two
captured entries does not cover recursion, additional children or every
memory/register merge; these need tests before adopting a general
`phiCopies` repair. A float-return fix also needs the callee's actual
result contract rather than an address-specific assignment. The NaN
probe restores three checks; an independent full-batch experiment sets
Ghidra's `NanIgnoreEnum.None` to test the upstream option instead. That
option alone does **not** fix the captured case: the lift still lacks its
NaN checks and only 3 of its 6 entries match. The installed Ghidra source
has a separate `RuleTrivialArith` self-comparison fold that does not consult
the NaN options; details are in the findings.

The option experiment lifts the same 2,885 functions and rejects the same
354. On all ten capture sets, every comparison count and verdict is
unchanged from the frame baseline: 1,225 same, 72 differ, 50 `r0`-only,
6 untested. Thus the option is not accepted as the real-entry repair.
The explicit-check probe remains evidence of the lost behavior, not a
general solution. Generated source, library, commands and hashes are in
`build/re-next/nan-preserved/`.

The option experiment's random-input run hit its 300-second batch limit
after 2,625 of 2,885 functions (200 attempted inputs each). Completed
verdicts: 1,782 same, 122 differ, 59 `r0`-only, 10 `alias-only`, 652
untested. Eight verdicts improved relative to the corresponding baseline
functions; none of these completed verdicts worsened. This is a partial
measurement, not comparable with the baseline's full-batch totals, and the
remaining 260 functions have no result from this run. The timeout and
partial comparison are retained in the receipt and summary rather than
reported as successful acceptance.

Artifacts: `build/re-next/receipt.json`, `summary.json`,
`noop-memory-receipt.json`, `noop-memory-replay.txt`,
`00B421A4-probe.txt`, `00A3F3F0-probe.txt`, `nan-sentinel-probe.txt` and
`probes/`. Focused capture headers are hashed in the receipt; their pages
remain in the original capture sets. The reference library and original
captures were not modified. The maintained recovery count remains ten.

### Maintained lifter repairs and stricter replay (14 November)

`kiu/ghidra/LiftOverrides.java` now suppresses synthetic memory self-copies,
reloads memory inputs during phi transfers even when they share a high
variable, and preserves NaN self-comparisons by disabling Ghidra's
`trivialarith` rule as well as setting `NanIgnoreEnum.None`. The configuration
is restored when the native decompiler process restarts. `DumpPcode.java`
uses the same configuration for diagnostics. Detailed anchors and the
limited float-return experiment are in the findings.

Same 3,239 input functions: 2,885 lifted and 354 rejected in every variant.
Same ten existing capture sets: 1,353 functions compared in every variant.

| Variant | Real: same | Real: differ | Real: `r0` only | Real: untested |
| --- | --- | --- | --- | --- |
| Frame baseline | 1,225 | 72 | 50 | 6 |
| Memory self-copy and phi repairs | 1,229 | 68 | 50 | 6 |
| Plus NaN-safe decompiler configuration | 1,230 | 67 | 50 | 6 |

Only five real-entry verdicts change, all from differing to matching:
`0x00B66020`, `0x00A3F3F0`, `0x0049B79C`, `0x003B5B94` and
`0x00637F5C`. No previously matching verdict regresses. This remains a
discrepant batch, not semantic-equivalence acceptance or recovered source.

A new complete random comparison uses **20** attempted inputs per function
in both variants, rather than repeating the incomplete 200-input campaign.
These totals must not be compared with earlier 200-input tables:

| Variant | Same | Differ | `r0` only | `alias-only` | Untested |
| --- | --- | --- | --- | --- | --- |
| Frame baseline, 20 inputs | 1,776 | 90 | 49 | 14 | 956 |
| Storage repairs, 20 inputs | 1,797 | 68 | 50 | 14 | 956 |
| Plus NaN-safe configuration, 20 inputs | 1,797 | 68 | 50 | 14 | 956 |

Twenty-one ordinary discrepancies become matching and one becomes
`r0`-only. No verdict worsens; the NaN configuration causes no further
random verdict change at this input count. Untested inputs still provide
no correctness evidence.

Five additional saved-state scenarios produced 31 actual entries of the
two storage-regression functions. Reference-self and repaired replay match
all 31; the old lift differs on all 31. The recorded objects still have
no parent or children. Eight separately constructed sphere hierarchies
cover parents, multiple children, child order and equality boundaries.
An independent union-of-bounds oracle agrees with the reference in all
eight. The storage repair removes all five ordinary fixture discrepancies;
seven fixtures retain `r0` differences. These fixtures are deliberately
constructed inputs, not observations of valid game-owned hierarchies.

The two-callee float-return experiment covers eight functions and 59 actual
entries. It removes five callers' memory discrepancies, but introduces
`r0` discrepancies in two previously matching callees. Only two functions
end fully matching; six are `r0`-only. These signature edits are rolled back
and not enabled in the maintained lifter. Single typed return values are
insufficient to represent the observed core and float register effects.

`kiu/scripts/replay.c` now returns failure for `r0`-only results, functions with
no comparable capture and an empty selection. Previously it only failed
ordinary discrepancies, so the float-return and hierarchy experiments
could report exit zero with unresolved returns. Regression checks confirm
matching entries exit zero, and return-only, untested and empty selections
exit nonzero. Earlier receipts retain their original exit codes; their
printed comparison counts remain the evidence.

Artifacts are under `build/repairs-20261114/`, including
`storage-summary.json`, `semantic-summary.json`, `check-receipt.json`,
`semantic-receipt.json`, `bounds-receipt.json`, `strict-receipt.json`,
generated candidates and the fixture generator. New actual captures are
under `trace/re-storage-20261114-a/`. No generated candidate was installed
in `kiu/overrides/`, and the maintained recovery count remains ten.

The Windows release build completed offline, was copied to
`dist/zakuro-kiu-mouse.exe`, and matches the build by SHA-256:
`e320f34af5f3d3d0cc03a179c70e9c86284ec7c5ec1cb389bad6485f4ebfd59d`.
That executable completed 60 interpreter/software/muted headless frames
through WSL. This checks the distributed frontend, not integration of
experimental lifted candidates, interactive controls, hardware rendering
or audio output. Build and smoke receipts are retained with the artifacts.

### Paired-result contracts (14 November continuation)

The maintained lifter accepts an optional result-contract file. The first
non-comment line must give the executable SHA-256; following lines name a
function and one or two raw register words, least significant first.
The verified file `kiu/ghidra/kiu-call-results.txt` describes the two callees
from the eight-function float-return cohort. Signature edits are
provisional and roll back at script exit. The packed unsigned return is
an analysis representation, not a recovered source type.

```
python3 scripts/lift.py <targets> build/paired-results 20 ghidra/kiu-call-results.txt
REPLAY_ONLY=00B49860 REPLAY_VFP_RESULTS=1 build/replay <reference.so> <candidate.so> <captures>...
```

The eight-function cohort now matches all 59 real entries. Both callees
also match when their `s0` results are explicitly compared. The previous
default replay excluded those caller-saved float registers: its old
matching callee verdicts did not establish correct float returns. With
the explicit check, the old candidate differs on nine of the seventeen
direct callee entries; the new candidate matches all seventeen.

Forty constructed result fixtures independently specify `r0` and `s0`:
32 packed-mask cases and eight product-list cases, including raw NaN
payloads, signed zero, infinity and underflow. Reference-self, the
constant-result oracle and the maintained candidate match all forty.
A mutant that flips one bit of `s0` is missed with the float-result check
disabled and rejected in all forty cases when it is enabled. These are
constructed inputs, not observed game-owned object graphs.

Same 3,239 targets, 2,885 lifted and 354 rejected; same ten capture sets:

| Configuration | Real: same | Real: differ | Real: `r0` only | Real: untested |
| --- | --- | --- | --- | --- |
| Previous maintained candidate | 1,230 | 67 | 50 | 6 |
| With paired-result contracts | 1,235 | 62 | 50 | 6 |

Only the five previously differing callers change verdict, all to
matching; no verdict regresses. The 20-attempt random check changes from
1,797 same / 68 differ to 1,798 same / 67 differ; 50 `r0`-only, 14
`alias-only` and 956 untested remain. That improvement has only one
comparable random input; the real entries and constructed fixtures are
the stronger evidence for this change. The batch remains discrepant.

The lifter also rejects failed/partial Ghidra generation before replacing
any previous candidate. Three unit tests pass; actual wrong-hash,
invalid-register and duplicate-register tests fail without overwriting
previous artifacts. Logs are retained even when Ghidra exits zero after
a script exception.

An additional isolation experiment established that Ghidra's automatic
script transaction delays nested rollbacks until script exit. Ending it
and isolating every function stabilizes the small cohort but regresses
other functions that relied on earlier inferred argument signatures.
That broader change was not adopted. Historical counts remain valid
measurements of their generated artifacts; they do not establish general
target-order independence. Witnesses and exact changes are recorded in
the findings and the experimental artifacts.

Evidence: `build/multi-return-20261114/`, particularly
`final-summary.json`, `final-replay-receipt.json`,
`final-cohort-receipt.json`, `fixture-receipt.json`,
`isolated-check-receipt.json`, `guard-receipt.json` and `final-full/`.
The Windows release was rebuilt and copied, and matches the existing
executable byte-for-byte. This round changes recovery tools only; it
does not install a new Windows game library or increase the maintained
recovery count of ten.

## Recovered

12 of 49,236 functions.

### Memory copy (`kiu/overrides/memory.c`)

| Address | Does | difftest |
| --- | --- | --- |
| `0x00425F00` | copies forward in pieces of 32, 16, 8, 4 and the last 3 bytes, each read whole before it is written; returns past both ends in `r0`/`r1` | 0 differ in 200,000 runs with sizes below 0x1000 and 100,000 below 70 |

It runs in all 55 saved states and generated code calls it directly from
471 places, so the whole runs below exercise it wherever the game copies.

### Input samples (`kiu/overrides/input_sample.c`)

The input manager keeps 16 samples of 0x25C bytes and folds those taken
since the last frame into one frame sample in `0x00A4245C` (not recovered).

| Address | Does | difftest, 200,000 runs |
| --- | --- | --- |
| `0x00919E90` | adds a sample's buttons and touch to the frame's | 0 differ |
| `0x00919ED0` | clears the frame's buttons and touch | 0 differ |
| `0x00919EF4` | pressed and released against the frame before; a label in `0x00A4245C` for 3dsrecomp, compared with `via 00A4245C` | 0 differ |
| `0x00919F38` | adds a sample's four sums to the frame's | 0 differ |
| `0x00919F7C` | clears the frame's sums | 0 differ |

Fields, from `trace/s1.state` with A held, a touch at (100, 60) and the
circle pad moved: `+0x00` held (A is 1), `+0x04` pressed, `+0x08` released,
`+0x204` touch (`0x003C0064`), `+0x208` touching. `+0x18`/`+0x1C`/`+0x20`
are a second mask with its own edges and `+0x20C`-`+0x218` four summed
words; what they carry is not established.

Whole runs, reference library against the override library, 120 frames with
A, a touch, the circle pad and RIGHT, dumping `0x00100000`+15 MiB,
`0x08000000`+8 MiB and `0x14000000`+16 MiB:

- With the input-sample overrides only (instruction-exact then): `s1`, `s5`,
  `s8`, `s12`, `s14` and `exo-tank` byte-identical, screenshots included.
- With the memory copy as well, all six overrides under `BUDGET_ATOMIC`: `s14`
  byte-identical; `s1`, `s5`, `s8`, `s9`, `s26` and `exo-tank` differ only
  in 1 to 11 words at `0x00D39C00`-`2C`, a thread's dead stack: in `s1` the
  reference holds the copy's own saved `r4`-`r10` and return `0x0011260C`
  there, which the recovered copy never pushes. `s12` differs in two words,
  `0x00E0BA88` and `0x00F35D10`, both tick words listed above. Heap, linear
  memory and both screenshots are identical in all eight.

The entry table and every call 3dsrecomp follows go to the overrides.

Not exercised by any saved state within 60 frames: `0x00919F38`, which only
runs when the manager's flag at `+0x314C` is set. It rests on difftest alone.

### Strings, the string hash and the set-once flag (`kiu/overrides/string.c`)

| Address | Does | difftest, 200,000 runs | Real entries |
| --- | --- | --- | --- |
| `0x00425DB4` | the length of a string: by characters to a word boundary, then by words | 0 differ | 15 same |
| `0x00425F74` | compares two strings, by words when both are word-aligned and by characters otherwise; the word path returns half the difference with the sign in the top bit, the character path the difference | 0 differ, and 0 with `DIFFTEST_SHAPE=1` | 15 same |
| `0x003B50EC` | sets a flag the first time it is asked and says whether it was the first; 19,637 direct call sites | 0 differ | 2 same |
| `0x00448770` | the hash of the string a pointer points at: each signed character combined as `hash ^= c + 0x9E3779B9 + (hash << 6) + (hash >> 2)`; 1,519 direct call sites | 0 differ in 3,715 compared, and in 25,442 with `DIFFTEST_SHAPE=2` | 3 same |

The instructions charged match in every run. Random words agree with each
other too rarely to reach the string compare's later characters: two wrong
versions of it, one ignoring a zero in a word's third character and one
dropping the sign at the fourth, passed 200,000 plain runs and failed 344
and 1,223 shaped ones.

All ten overrides replayed as a library (`build/decomp`) over the 8,887
real entries, their callers' included: no difference in any of the 1,353
functions.

Whole runs as above, the same eight states: every difference from the
reference is in the dead stack words at `0x00D39C00`-`2C` (`s1`, `s5`,
`s8`, `s9`, `s26`, `exo-tank`, the same words as with six overrides) or in
tick words (`0x00F35D10` in `s1`, `s12` and `s14`, `0x00E0BA88` in `s12`;
3 to 40 ticks earlier). Heap, linear memory and both screenshots are
identical in all eight. Against the six-override runs only those tick
words changed. Scripts and dumps are under ignored `trace/decomp-string/`.

`kiu/overrides/` is not compiled into the Windows frontend: its Recompile does
not read it, so these run only in libraries built with
`3dsrecomp build --overrides`.

### Scalar storage (`kiu/overrides/scalar.c`)

Two hand-written routines preserve paired raw `r0`/`s0` results. Field
names describe the observed layout, without assigning a game-level meaning
to the factors. Accesses, anchors, termination rules and instruction-charge
formulas are recorded in `kiu/analysis/FINDINGS.md`.

| Routine | Random inputs, with `s0` checked | Recorded entries | Deliberate fixtures |
| --- | --- | --- | --- |
| Packed optional scalar | 40,000 compared (20,000 plain, 20,000 pointer-shaped), no differences | 2 same | 32 same |
| Linked factor product | 44 comparable out of 40,000 ordinary/pointer-shaped attempts; another 20,000 valid lists of lengths 0–32, no differences | 15 same | 8 same |

The full-library random tests compare instruction charges as well as raw
float bits. Arbitrary float words in the valid lists exposed a NaN payload
selection difference in the initial direct C multiplication: 52 of 20,000
results differed. Explicit first-operand NaN propagation fixed these.
This matches the Linux generated reference; hardware/default-NaN behavior
is not independently established. The maintained directed test checks the
distinct-NaN case and packed outcomes/charges under `-O0` and `-O2`.

`DIFFTEST_VFP_RESULTS=0..16` enables explicit float-result comparison in
the random harness. A one-bit `s0` mutant passes all 200 inputs without it
and fails all 200 with it enabled. Invalid counts are rejected. Six unit
tests cover this option, the recovered routines and the existing Ghidra
output guards.

Separate `-O2` override builds also match 20,000 packed-scalar inputs and
20,000 valid factor lists. Those standalone checks compare results and
memory; exact instruction-charge checks use the full library above.

The twelve-override library replays 8,887 actual entries: 1,347 functions
match, none differ, and six have no comparable capture. The same six fail
reference-versus-reference replay too (46 entries), so the complete replay
returns status 1 and is not an all-functions pass. This also qualifies the
older ten-override replay statement above: uncomparable entries must stay
separate from matches.

Whole-game comparisons run `s1`, `s5`, `s8`, `s9`, `s12`, `s14`, `s26` and
`exo-tank` for 120 frames with the existing input scripts, through fresh
reference and twelve-override runs. Heap, linear memory and both screenshots
are identical in all eight. Image differences are confined to the previously
documented dead stack (`0x00D39C00`–`2C`) and tick words (`0x00F35D10`,
`0x00E0BA88`); the complete word list is in `whole-differences.json`.

Evidence and reproducible build/check scripts are under
`build/scalar-20261114/`; the library is
`build/decomp-scalar/0004000000030100.so`. All twelve overrides are in its
entry table and resolved direct-call paths. The incremental build reuses
525 objects only where generated C is byte-identical to the previous
ten-override build, and compiles the four changed sources with the same
ABI header and equivalent `cc-fast` flags.

The Windows release executable was rebuilt offline, replaced in `dist/`
and compared byte-for-byte with its build output. Its SHA-256 remains
`e320f34af5f3d3d0cc03a179c70e9c86284ec7c5ec1cb389bad6485f4ebfd59d`.
No new Windows game library has been installed. Interactive Windows
gameplay with these overrides remains unverified.

### Windows game library with the twelve overrides (14 November)

Windows Recompile now includes the twelve maintained C Overrides. Zakuro
embeds `kiu/overrides/*.c` as a pinned set for program `0004000000030100` and
executable code hash `D6E028C6175A2B07`; 3dsrecomp adds a pinned set only
when the code being recompiled has that hash, and otherwise leaves it out
with a note. An update or mod that changes the code therefore gets the
generated functions. Duplicate file names or addresses across sets are
refused.

The first Windows build compiled all 529 objects and then failed to link
with os error 206: objects in `hand/` disabled the single-folder path
shortening, so the link passed whole paths over the 32,767-character
limit. Linking now runs from the common folder with relative paths
(5,312 characters for this title). Frontend Recompile had the same defect.

Both Windows libraries were built with Zig 0.17.0 `cc`, as frontend
Recompile uses, from the same hints file. Their generated C reproduces
byte-for-byte on Linux: the candidate's tree equals the pinned and the
`--overrides` trees, and the reference differs from neither in any of its
527 files. Windows hints add 105 functions not in the Linux library.

| Library | Objects | SHA-256 |
| --- | --- | --- |
| Candidate, twelve overrides (installed) | 529 | `be76a2ee…8100` |
| Reference, no overrides | 525 | `04f546d6…1aa0` |
| Previous installed (28 October) | — | `f2c9a2dc…fa38` |

The eight whole-game scripts (`s1`, `s5`, `s8`, `s9`, `s12`, `s14`, `s26`,
`exo-tank`, 120 frames with input) ran through Windows kiutrace against
each library. Heap, linear memory and both screenshots are identical in
all eight. Image differences are confined to the dead stack
(`0x00D39C00`–`2C`) and tick words (`0x00F35D10`, `0x00E0BA88`), as on Linux.
The Windows candidate also matches the Linux twelve-override runs in heap,
linear memory and screens; Windows-versus-Linux image differences include
three further `s12` words (`0x00E0B9C8`, `0x00E0BA80`, `0x00F2F358`) that
hold tick-like values 17 higher and do not differ between the two Windows
libraries. That they are host timing is inferred, not established.

The candidate is installed at `%APPDATA%\3dsrecomp\0004000000030100.dll`;
the previous library is kept in `build/windows-lib-20261114/backup-before/`.
The frontend was rebuilt with the link fix (`06db1181…086b`) and replaced
in `dist/`. Receipts and logs are under `build/windows-lib-20261114/`.
Interactive Windows gameplay with this library remains unverified.

## Next

1. Interactive Windows play with the installed twelve-override library,
   especially input sampling (`kiu/overrides/input_sample.c`) and the scalar
   routines in battle. Scripted Windows comparisons pass; see above.
2. Closed functions the saved states run (3,239), ranked by how often they
   are called: `build/continue/callsites.txt` counts direct call sites.
   The next most called are the tree removal at `0x003D41E8` (2,952), the
   small constructors at `0x00BACC68`, `0x001EB990` and `0x00BACE80`, and
   the lookup at `0x0049C6EC`; their fields need naming from evidence first.
3. Give lifted calls the callee's own arguments and results; these remain
   the dominant cause among the 62 real-entry discrepancies. Retain the
   50 `r0`-only and six untested functions as separate unresolved groups.
   Replace the useful inferred argument dependencies with explicit,
   verified contracts before enabling true per-function isolation.
4. `0x00A4245C` and its callers stay open: they take the input manager's
   lock (`0x003D1C20`, `0x003ABC50`), which can wait in an svc.
5. Reconcile the two function lists and classify the unreached text.
6. Saved states for the scenes no state covers; 88% of functions have not
   run under the harness.
