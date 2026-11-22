# Autonomous KIU recovery

## Execution contract

Run `python3 scripts/overnight.py` from the repository root. It runs sequential coordinator rounds for at most eight hours, retains transcripts and logs under `build/overnight/`, and stops after a failed round. Create `build/overnight/STOP` to stop before the next round; use `omp ps` to stop the supervised service immediately. WSL and the host must stay awake. A completed round is not permission to stop the campaign: the runner starts the next round with the saved handoff. No work runs after the deadline; in-flight work may be interrupted, so checkpoint before expensive actions.

Routing is project-scoped in `.omp/config.yml`. Named agents use fixed role mappings and OAuth account pools. Model fallback and paid search are disabled. Cursor is permitted, not required: its subscription entitlement and billing settings must be verified before assigning it work. Never enable usage-based overflow, redeem resets, change account/routing settings, install dependencies, or route through API-key providers. This is routing enforcement, not an OS sandbox; shell access retains the user's privileges.

The setup smoke on 14 November found Antigravity disabled with HTTP 403
`TOS_VIOLATION` and Claude quota depleted. Tonight's explicit routes therefore
use Codex Sol for coordination/semantic work, Luna for mechanical work and Terra
for review. The other permitted subscriptions are not automatic fallbacks.

## Coordinator round

1. Read `docs/decomp/ROADMAP.md`, `docs/decomp/LEDGER.md`, the latest `build/overnight/HANDOFF.md` if present, and relevant sections of `kiu/analysis/FINDINGS.md`. Inspect existing artifacts before selecting work. Respect pre-existing edits as user-owned.
2. Select the highest-leverage reachable recovery task in the roadmap: classify discrepancy causes, widen actual-entry evidence, recover evidenced types, or verify a bounded subsystem. Prefer eliminating a failure class to bulk-producing unnamed C. Do not repeat completed measurements merely to confirm documented results.
3. Define work packets: goal, address ranges, source evidence, exclusive file ownership, observable acceptance and output destination. Root owns shared findings, ledger and handoff. Use `kiu-lead` for an independently substantial subsystem, `kiu-analysis` for uncertain semantics, and `kiu-mechanical` only for completely specified transformations. Read-only exploratory research uses `scout`. Review uses `reviewer`.
4. Delegate only when at least two independent substantial slices exist. At most four active subagents across the entire tree, including leads; reserve slots explicitly in each lead packet. One owner per file. Nested agents inherit slot allocations, not four new slots. Otherwise work inline. Workers skip builds/tests/lint until fan-in.
5. Integrate and exercise the changed path using existing Ghidra, difftest, replay and kiutrace workflows. Keep experimental outputs under ignored `build/` or `trace/`. Label comparison counts, untested paths and inferred semantics. Never count lifted candidates as recovered source without every ledger gate. Have uncertain semantic changes independently reviewed before promotion.
6. Update permanent docs only for verified permanent changes. Any game/runtime code change requires the Windows release rebuild and replacement of `dist/zakuro-kiu-mouse.exe`, with matching build/distribution hashes. If that pipeline is unavailable, retain work as experimental candidates outside production paths; do not deliver partial production code. Do not change gameplay behavior, add success stubs or conceal unsupported paths.
7. Before ending every round, write `build/overnight/HANDOFF.md`: completed work, files owned/changed, exact verification output, unresolved risks, active-agent status, and the next nonduplicate packet. Write `build/overnight/REPORT.md` as the current morning report. Finish reachable work, then return; the runner handles continuation.

## Safety and stop conditions

No commits, pushes, rebases, tags, destructive git commands, unrelated cleanup, dump uploads or credential reads. Stay in native Linux paths. Preserve original dumps, saved states and reference artifacts. New capture sets use unique directories; check free disk before large captures. Do not run multiple Ghidra writers against one project. Use existing tooling and dependencies only.

Stop the round and report when subscription credentials/quota fail, evidence needed for safe progress is unavailable, free disk is insufficient, or every reachable independent task is exhausted. Write `build/overnight/STOP` with the reason to prevent another round. An authentication or quota failure is a blocker, not permission to use another billing route.
