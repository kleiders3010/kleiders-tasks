#!/bin/bash
# Build natlab inside WSL Ubuntu with a Linux-native target dir (fast, no cross-FS builds).
set -e
source /root/.cargo/env
cd "$(dirname "$0")/natlab"
CARGO_TARGET_DIR=/root/natlab-target cargo build --release "$@" 2>&1 | grep -E "^(error|warning: unused)|^\s+-->|^[0-9]+ \||Finished|error\[" -A0 | head -150
install -m755 /root/natlab-target/release/natlab /usr/local/bin/natlab
