#!/usr/bin/env python3
import os
import sys
import json
import re
import urllib.request
import subprocess
from python.runfiles import runfiles

def find_my_keys():
    # 1. Find the manifest file
    manifest_path = os.environ.get("RUNFILES_MANIFEST_FILE")

    if not manifest_path:
        # Fallback for Linux/macOS if manifest isn't env-set
        # It's usually right next to the executable
        manifest_path = "bazel-bin/server/serve.runfiles_manifest"
        # (Adjust this path to where your actual bin is if running manually)

    print("--- VALID RUNFILES KEYS ---")
    if manifest_path and os.path.exists(manifest_path):
        with open(manifest_path, 'r') as f:
            for line in f:
                # The first part of each line is the 'key' for Rlocation
                key = line.split(' ')[0]
                if "server" in key or "tarball" in key or "yaml" in key:
                    print(f"MATCH: {key}")
    else:
        # If no manifest, list the directory tree
        rf_dir = os.environ.get("RUNFILES_DIR")
        print(f"No manifest found. Checking directory: {rf_dir}")
        # ... (os.walk logic from before)
    print("---------------------------")


def main():
    # 1. Initialize Bazel Runfiles
    r = runfiles.Create()
    if not r:
        print("Error: Could not initialize Bazel runfiles.")
        sys.exit(1)

    # Note: Bzlmod uses "_main" as the default workspace name.
    # If using a legacy WORKSPACE file, change this to your workspace name.
    workspace = "_main"

    # 2. Resolve data dependencies
    # The paths must match the Bazel package + target output name.
    thockflow_tar = r.Rlocation(f"{workspace}/server/image-nonroot-amd64-load/tarball.tar")
    cloudflared_tar = r.Rlocation(f"{workspace}/server/cloudflared-load/tarball.tar")
    compose_file = r.Rlocation(f"{workspace}/podman-compose.yaml")

    missing = []
    if not thockflow_tar: missing.append("thockflow_tar")
    if not cloudflared_tar: missing.append("cloudflared_tar")
    if not compose_file: missing.append("compose_file")

    if missing:
        print(f"Error: runfiles.Rlocation failed to find: {', '.join(missing)}")
        print("Check your workspace name and the paths in serve.py.")
        find_my_keys()
        sys.exit(1)

    print("Successfully resolved all Bazel runfiles.")

    # 5. Load OCI Images into Podman
    print("\nLoading thockflow image into Podman...")
    subprocess.run(["podman", "load", "-i", thockflow_tar], check=True)

    print("Loading cloudflared image into Podman...")
    subprocess.run(["podman", "load", "-i", cloudflared_tar], check=True)

    # 6. Validate Environment
    if "CF_TUNNEL_THOCKFLOW" not in os.environ:
        print("\nError: CF_TUNNEL_THOCKFLOW environment variable is not set.")
        print("Usage: envgpg -e CF_TUNNEL_THOCKFLOW bazel run //:serve")
        sys.exit(1)

    # 7. Start the Stack
    print("\nStarting isolated Kata VMs via Podman Compose...")
    # Pass along any extra arguments provided to `bazel run //:serve -- [args]`
    compose_args = ["podman-compose", "-f", compose_file, "up"] + sys.argv[1:]

    try:
        subprocess.run(compose_args, check=True)
    except KeyboardInterrupt:
        print("\nStopping services...")
        subprocess.run(["podman-compose", "-f", compose_file, "down"])

if __name__ == "__main__":
    main()
