#!/usr/bin/env python3
"""Fetch the pinned CPU ONNX Runtime C library, without installing Python packages."""
import argparse
import hashlib
import io
import json
import platform
from pathlib import Path
import urllib.request
import zipfile

VERSION = "1.30.0"

def main(argv=None):
    parser = argparse.ArgumentParser(__doc__)
    parser.add_argument("destination", type=Path)
    args = parser.parse_args(argv)
    system, arch = platform.system(), platform.machine().lower()
    # ponytail: Intel Mac stays on the last prebuilt runtime until upstream ships Intel binaries again.
    version = "1.23.2" if (system, arch) == ("Darwin", "x86_64") else VERSION
    tag = {("Linux", "x86_64"): "manylinux_2_28_x86_64", ("Linux", "aarch64"): "manylinux_2_28_aarch64", ("Windows", "amd64"): "win_amd64", ("Darwin", "arm64"): "macosx_", ("Darwin", "x86_64"): "macosx_"}.get((system, arch))
    if tag is None:
        raise SystemExit(f"No bundled runtime for {system}/{arch}; set ORT_DYLIB_PATH to your own runtime")
    metadata = json.load(urllib.request.urlopen(f"https://pypi.org/pypi/onnxruntime/{version}/json", timeout=30))
    candidates = [f for f in metadata["urls"] if "cp311-cp311" in f["filename"] and tag in f["filename"] and (system != "Darwin" or "universal2" in f["filename"] or arch in f["filename"])]
    if len(candidates) != 1:
        raise SystemExit(f"Expected one matching runtime wheel, found {len(candidates)}")
    package = candidates[0]
    data = urllib.request.urlopen(package["url"], timeout=60).read()
    assert hashlib.sha256(data).hexdigest() == package["digests"]["sha256"], "Runtime checksum mismatch"
    args.destination.mkdir(parents=True, exist_ok=True)
    libraries = []
    with zipfile.ZipFile(io.BytesIO(data)) as archive:
        for name in archive.namelist():
            base = Path(name).name
            if name.startswith("onnxruntime/capi/") and (base.endswith((".dll", ".dylib", ".so")) or ".so." in base) and not base.startswith("onnxruntime_pybind"):
                if base.startswith("libonnxruntime.so"):
                    base = "libonnxruntime.so"
                elif base.startswith("libonnxruntime.") and base.endswith(".dylib"):
                    base = "libonnxruntime.dylib"
                (args.destination / base).write_bytes(archive.read(name))
                libraries.append(base)
            elif base in {"LICENSE", "ThirdPartyNotices.txt", "Privacy.md"}:
                notices = args.destination / "onnxruntime-notices"
                notices.mkdir(exist_ok=True)
                (notices / base).write_bytes(archive.read(name))
    expected = {"Linux": "libonnxruntime.so", "Windows": "onnxruntime.dll", "Darwin": "libonnxruntime.dylib"}[system]
    assert expected in libraries, "Wheel did not contain the C runtime"
    (args.destination / "onnxruntime-package.json").write_text(json.dumps({"version": version, "source": package["url"], "sha256": package["digests"]["sha256"], "libraries": libraries}, indent=2) + "\n")
    print(f"ONNX Runtime {version}: {', '.join(libraries)}")

if __name__ == "__main__":
    main()
