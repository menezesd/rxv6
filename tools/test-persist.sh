#!/bin/bash
# Persistence test: run kernel twice on the same disk image
# First boot: runs persist-write (creates persist.dat)
# Second boot: runs persist-read (verifies persist.dat survived)
set -e

KERNEL=/Users/dean/rustos/target/i686-rustos/debug/rustos
DISK=/tmp/rustos-persist-test.img

# Create fresh disk with both test programs
cd /Users/dean/rustos/user
MKDISK=/tmp/mkdisk-target/aarch64-apple-darwin/release/mkdisk
U=target/i686-rustos-user/release
$MKDISK "$DISK" 32 $U/persist-write $U/persist-read $U/hello

echo "=== Boot 1: Writing persistent data ==="
cd /Users/dean/rustos
timeout 30 qemu-system-i386 -kernel "$KERNEL" -serial stdio -display none -no-reboot -m 128 \
  -drive file="$DISK",format=raw,if=ide 2>/dev/null | grep -E "persist|PASSED|FAILED"

echo ""
echo "=== Boot 2: Reading persistent data (same disk) ==="
timeout 30 qemu-system-i386 -kernel "$KERNEL" -serial stdio -display none -no-reboot -m 128 \
  -drive file="$DISK",format=raw,if=ide 2>/dev/null | grep -E "persist|PASSED|FAILED"
