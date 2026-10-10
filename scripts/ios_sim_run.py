#!/usr/bin/env python3
"""Cargo runner for aarch64-apple-ios-sim: runs a test, bench or example binary in an iOS simulator.

The device is TESSSTEP_IOS_SIMULATOR, a UDID or a device name; it is booted when shut down.
`simctl spawn` starts the process from launchd with neither the caller's working directory nor
its environment, so RUST_* and TESSSTEP_* variables are forwarded and the binary must not rely
on its working directory. Host paths stay readable from inside the simulator.
"""
import json
import os
import subprocess
import sys


def find_device(wanted):
    listing = subprocess.run(["xcrun", "simctl", "list", "devices", "available", "--json"],
                             check=True, capture_output=True, text=True).stdout
    devices = [d for runtime in json.loads(listing)["devices"].values() for d in runtime
               if wanted in (d["udid"], d["name"])]
    if not devices:
        sys.exit(f"ios_sim_run: no available simulator named or with UDID '{wanted}'")
    booted = [d for d in devices if d["state"] == "Booted"]
    return (booted or devices)[0]


def main():
    wanted = os.environ.get("TESSSTEP_IOS_SIMULATOR")
    if not wanted:
        sys.exit("ios_sim_run: set TESSSTEP_IOS_SIMULATOR to the UDID or name of an iOS simulator")
    device = find_device(wanted)
    if device["state"] != "Booted":
        subprocess.run(["xcrun", "simctl", "boot", device["udid"]], check=True)
    env = {k: v for k, v in os.environ.items() if not k.startswith("SIMCTL_CHILD_")}
    for key, value in os.environ.items():
        if key.startswith(("RUST_", "TESSSTEP_")):
            env["SIMCTL_CHILD_" + key] = value
    command = ["xcrun", "simctl", "spawn", device["udid"], *sys.argv[1:]]
    sys.exit(subprocess.run(command, env=env).returncode)


if __name__ == "__main__":
    main()
