#!/bin/bash
# Run rxv6 in QEMU
RXDIR="$(cd "$(dirname "$0")" && pwd)"
KERNEL="$RXDIR/target/i686-rxv6/debug/rxv6"
DISK="$RXDIR/fs.img"

if [ ! -f "$KERNEL" ]; then
    echo "Kernel not found. Run ./build.sh first."
    exit 1
fi

if [ ! -f "$DISK" ]; then
    echo "Disk image not found. Run ./build.sh first."
    exit 1
fi

exec qemu-system-i386 \
    -kernel "$KERNEL" \
    -drive file="$DISK",format=raw,if=ide \
    -serial stdio \
    -display none \
    -m 64 \
    "$@"
