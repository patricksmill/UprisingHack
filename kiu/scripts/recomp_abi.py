#!/usr/bin/env python3
"""Finds 3dsrecomp's abi folder, the one with recomp.h, for scripts that
compile against it.

In order: $RECOMP_ABI; a local 3dsrecomp clone at kiu/tools/3dsrecomp, for
working on the recompiler; otherwise the recomp-abi Cargo checked out for
the commit Cargo.lock pins.

usage: recomp_abi.py      prints the folder, as in cc -I "$(python3 scripts/recomp_abi.py)"
"""
import json
import os
import subprocess
import sys
from pathlib import Path

KIU = Path(__file__).resolve().parent.parent


def abi_dir() -> Path:
    if os.environ.get("RECOMP_ABI"):
        return Path(os.environ["RECOMP_ABI"])
    local = KIU / "tools/3dsrecomp/abi"
    if (local / "recomp.h").exists():
        return local
    metadata = subprocess.run(["cargo", "metadata", "--format-version", "1", "--locked"], cwd=KIU.parent,
                              capture_output=True, text=True, check=True).stdout
    for package in json.loads(metadata)["packages"]:
        if package["name"] == "recomp-abi":
            return Path(package["manifest_path"]).parent
    sys.exit("recomp-abi is not among the workspace's dependencies")


if __name__ == "__main__":
    print(abi_dir())
