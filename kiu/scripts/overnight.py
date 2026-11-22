#!/usr/bin/env python3
"""Run bounded, sequential OMP recovery rounds using project subscription routing."""

import argparse
import datetime
import os
from pathlib import Path
import signal
import subprocess
import time


ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "build" / "overnight"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--hours", type=float, default=8)
    parser.add_argument("--smoke", action="store_true")
    args = parser.parse_args()
    def interrupted(signum: int, frame: object) -> None:
        raise KeyboardInterrupt

    signal.signal(signal.SIGTERM, interrupted)
    if not 0 < args.hours <= 8:
        parser.error("--hours must be greater than zero and at most eight")
    OUT.mkdir(parents=True, exist_ok=True)
    if (OUT / "STOP").exists():
        print("STOP exists; refusing to start. Read its reason before removing it.", flush=True)
        return 1
    deadline = time.monotonic() + args.hours * 3600
    stamp = datetime.datetime.now(datetime.timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    prompt = (
        "Read docs/decomp/OVERNIGHT.md and execute one coordinator round. "
        "You are the root integration owner. Continue the existing KIU recovery "
        "roadmap, not UI enhancements. Checkpoint before expensive operations. "
        "Do not alter orchestration configuration or this runner."
    )
    if args.smoke:
        prompt = (
            "Routing smoke only. Read docs/decomp/OVERNIGHT.md. Spawn exactly two "
            "independent read-only tasks together: kiu-mechanical inventories the "
            "verification categories in docs/decomp/ROADMAP.md; kiu-analysis reads "
            "docs/decomp/LEDGER.md and identifies one evidence-backed unresolved "
            "semantic risk. Each reports its active provider/model and one concrete "
            "finding. Neither edits files or spawns children. Wait for both. "
            "Report their observed models and findings. Do not perform recovery work."
            " Print KIU_SMOKE_OK only if both children successfully return concrete "
            "document findings; otherwise report failure without that marker."
        )
    print(f"KIU overnight started: {stamp}; hours={args.hours}; smoke={args.smoke}", flush=True)
    round_number = 0
    while time.monotonic() < deadline and not (OUT / "STOP").exists():
        round_number += 1
        remaining = int(deadline - time.monotonic())
        if remaining < 30:
            break
        limit = min(2700, remaining)
        log = OUT / f"{stamp}-round-{round_number:03d}.log"
        command = [
            "omp", "--print", "--wait-async", "--no-title", "--no-extensions",
            "--model", "openai-codex/gpt-6.1-sol", "--thinking", "high",
            "--max-time", f"{limit}s", "--auto-approve",
            "--session-dir", str(OUT / "sessions"),
            prompt + f" Round deadline is in {limit} seconds; leave time for handoff.",
        ]
        print(f"Round {round_number}: {log}", flush=True)
        with log.open("w") as stream:
            process = subprocess.Popen(
                command, cwd=ROOT, stdin=subprocess.DEVNULL,
                stdout=stream, stderr=subprocess.STDOUT, start_new_session=True,
            )
            try:
                result = process.wait(timeout=limit)
            except (subprocess.TimeoutExpired, KeyboardInterrupt):
                os.killpg(process.pid, signal.SIGTERM)
                try:
                    process.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    os.killpg(process.pid, signal.SIGKILL)
                    process.wait()
                print("Stopped at deadline or interrupt; inspect the last checkpoint.", flush=True)
                return 124
        print(f"Round {round_number} exit={result}", flush=True)
        if args.smoke and "KIU_SMOKE_OK" not in log.read_text():
            print("Smoke failed: both worker findings were not confirmed.", flush=True)
            return 1
        if result != 0 or args.smoke:
            return result
    print("Overnight campaign stopped; read build/overnight/REPORT.md and logs.", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
