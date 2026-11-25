# Kid Icarus: Uprising PC Port (`icarus`)

PC recompilation and enhancement project for Kid Icarus: Uprising (USA, 0004000000030100) using `3dsrecomp` and `zakuro`.

## Key Directories & Components

This repository is a fork of upstream Zakuro (`fearkov/zakuro`, remote `upstream`). Merge `upstream/main` regularly; keep port-only files in `kiu/` so merges stay clean. Upstream's `.github/workflows/ci.yml`, `release.yml` and `ISSUE_TEMPLATE/game-report.yml` stay deleted: `.github/workflows/build.yml` replaces them. Upstream's `COMPATIBILITY.md`, `docs/screenshots/` and `packaging/` stay deleted too: the port ships only the Windows zip and Linux tarball, and has its own README. If a merge conflicts on any of these, `git rm` it again.

- `crates/`: the Zakuro runtime, with the port's hooks (`zakuro-core/src/aim_mouse/`) and PC frontend (`zakuro/`).
- `kiu/overrides/`: C overrides, embedded by `crates/zakuro/src/recompile.rs` and passed to `3dsrecomp port --overrides overrides/`.
- `kiu/ghidra/`: analysis scripts and headless disassembly pipelines for `code.bin`.
- `kiu/tools/kiutrace`, `kiu/tools/funclist`: workspace members.
- `docs/adr/`: architectural and porting decisions.
- 3dsrecomp: the `patricksmill/3dsrecomp` fork (branch `kiu`), a Cargo git dependency pinned in `Cargo.lock`. For recompiler work, clone it to `kiu/tools/3dsrecomp` (ignored) and override the `[patch]` with `--config`; push fork commits, then `cargo update -p recomp3ds`.

## Paths

- Code anchors in docs are repository paths (`crates/...`).
- Scripts in `kiu/scripts/` and shell commands in `docs/` and `PLAN.md` run from `kiu/`; `dumps/`, `build/`, `trace/` and `dist/` mean `kiu/dumps/` and so on.

## Windows delivery

After code changes, rebuild the Windows release executable and replace `kiu/dist/UprisingHack.exe` before reporting the changes ready to use. Verify the distributed executable matches the new build; report build or runtime-verification blockers explicitly.

## Agent skills

### Issue tracker

GitHub Issues on `patricksmill/UprisingHack`. See `docs/agents/issue-tracker.md`. This repository is a fork: pass `-R patricksmill/UprisingHack` (or `gh repo set-default`) so PRs and issues never go to `fearkov/zakuro`.

### Pull requests

Pick the body from `.github/PULL_REQUEST_TEMPLATE/`: `screen.md` (one Screen's look or input), `feature.md`, `bugfix.md` or `chore.md` (build, CI, docs, refactors, upstream merges). Pass it with `gh pr create --template <name>.md`, or fill its sections into `--body`.

### Triage labels

Canonical 5-role triage vocabulary (`needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`). See `docs/agents/triage-labels.md`.

### Domain docs

Single-context repo layout (`GLOSSARY.md` + `docs/adr/`). See `docs/agents/domain.md`.

### Vocabulary

- Use `GLOSSARY.md` terms; never substitute a word marked _Avoid_.
- Every change request follows `docs/agents/change-requests.md`: Operation, Target, Screen, before/after, Acceptance.
- Every game-owned Screen follows `docs/agents/screen-transform.md`: one pipeline, one `ScreenSpec`, one `Placement` list shared by draw and hit-test.
- Code anchors stay in `kiu/analysis/FINDINGS.md`; the glossary holds concepts only.

### Screen status

`docs/screen-status.md` tracks each Screen as Layerized, Controller, Mouse and Rearranged. Pick work from it, and update the Screen's row in the same commit as the work.
