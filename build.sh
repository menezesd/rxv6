#!/bin/bash
# Build the complete rxv6 system: kernel + user programs + disk image
set -e

RXDIR="$(cd "$(dirname "$0")" && pwd)"
USERDIR="$RXDIR/user"
TOOLSDIR="$RXDIR/tools/mkdisk"
TARGET=i686-rxv6-user

# User programs to include in the disk image
PROGS="init sh cat echo ls wc grep mkdir-cmd rm ln-cmd kill-cmd"

echo "=== Building kernel ==="
cargo build 2>&1 | grep -v "^warning"

echo ""
echo "=== Building user programs (release, stripped) ==="
(cd "$USERDIR" && cargo build --release -p init -p sh -p cat -p echo -p ls -p wc -p grep -p mkdir-cmd -p rm -p ln-cmd -p kill-cmd 2>&1 | grep -E "Compiling|Finished|error")

echo ""
echo "=== Stripping user binaries ==="
STRIPPED_DIR="$RXDIR/build/user"
mkdir -p "$STRIPPED_DIR"

# Find llvm-strip or llvm-objcopy in the Rust toolchain
STRIP_TOOL=""
RUSTUP_HOME="${RUSTUP_HOME:-$HOME/.rustup}"
STRIP_CANDIDATE="$(find "$RUSTUP_HOME" -name 'llvm-objcopy' 2>/dev/null | head -1)"
if [ -n "$STRIP_CANDIDATE" ]; then
    STRIP_TOOL="$STRIP_CANDIDATE"
fi

for prog in $PROGS; do
    src="$USERDIR/target/$TARGET/release/$prog"
    if [ -f "$src" ]; then
        if [ -n "$STRIP_TOOL" ]; then
            "$STRIP_TOOL" --strip-all "$src" -O elf32-i386 "$STRIPPED_DIR/$prog" 2>/dev/null || cp "$src" "$STRIPPED_DIR/$prog"
        else
            cp "$src" "$STRIPPED_DIR/$prog"
        fi
        size=$(wc -c < "$STRIPPED_DIR/$prog" | tr -d ' ')
        echo "  $prog: ${size}B"
    else
        echo "  $prog: NOT FOUND (skipped)"
    fi
done

echo ""
echo "=== Building mkdisk tool ==="
(cd "$TOOLSDIR" && cargo build --release 2>&1 | grep -E "Compiling|Finished|error")
# mkdisk has its own .cargo/config.toml targeting aarch64-apple-darwin
HOST_TARGET="aarch64-apple-darwin"
MKDISK="$TOOLSDIR/target/$HOST_TARGET/release/mkdisk"
if [ ! -f "$MKDISK" ]; then
    MKDISK="$TOOLSDIR/target/$HOST_TARGET/debug/mkdisk"
fi

echo ""
echo "=== Creating disk image ==="
# Collect all stripped binaries
FILES=""
for prog in $PROGS; do
    if [ -f "$STRIPPED_DIR/$prog" ]; then
        FILES="$FILES $STRIPPED_DIR/$prog"
    fi
done

"$MKDISK" "$RXDIR/fs.img" 8 $FILES
echo ""
echo "=== Build complete ==="
echo "Kernel: target/i686-rxv6/debug/rxv6"
echo "Disk:   fs.img"
echo ""
echo "Run with QEMU:"
echo "  qemu-system-i386 -kernel target/i686-rxv6/debug/rxv6 -drive file=fs.img,format=raw,if=ide -serial stdio -display none"
