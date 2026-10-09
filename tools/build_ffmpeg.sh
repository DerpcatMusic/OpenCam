#!/usr/bin/env bash
# A small, dynamically linked LGPL decoder, built identically on Linux and macOS.
set -euo pipefail
script=$(cd "$(dirname "$0")" && pwd)/$(basename "$0")
prefix=$(mkdir -p "$1" && cd "$1" && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
curl --fail --location --retry 3 https://ffmpeg.org/releases/ffmpeg-9.0.2.tar.xz -o "$work/source.tar.xz"
python3 - "$work/source.tar.xz" <<'PY'
import hashlib, sys
from pathlib import Path
assert hashlib.sha256(Path(sys.argv[1]).read_bytes()).hexdigest() == '8c3850283eb25fa026482078a04051e0be17347b09ef81a0849bec15a96e002e', 'FFmpeg checksum mismatch'
PY
tar -xf "$work/source.tar.xz" -C "$work"
cd "$work/ffmpeg-9.0.2"
./configure --prefix="$prefix" --disable-everything --disable-autodetect \
  --disable-programs --disable-doc --disable-network --disable-static --enable-shared \
  --enable-pic --enable-decoder=h264,hevc --enable-parser=h264,hevc \
  --disable-avdevice --disable-avfilter --disable-swresample --enable-avformat --enable-swscale
make -j 2
make install
mkdir -p "$prefix/notices"
cp COPYING.LGPLv2.1 "$prefix/notices/LICENSE"
cp "$work/source.tar.xz" "$prefix/notices/ffmpeg-9.0.2-source.tar.xz"
cp "$script" "$prefix/notices/build_ffmpeg.sh"
