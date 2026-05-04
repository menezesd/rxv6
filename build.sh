#!/bin/bash
# Build the complete rxv6 system: kernel + user programs + disk image
set -e

RXDIR="$(cd "$(dirname "$0")" && pwd)"
USERDIR="$RXDIR/user"
TOOLSDIR="$RXDIR/tools/mkdisk"
TARGET=i686-rxv6-user

# User programs: "crate_name:disk_name" (or just "name" if same)
PROGS=(
    init sh cat echo ls wc grep ed cp mv tee sort uniq od rev seq
    mkdir-cmd:mkdir rm ln-cmd:ln kill-cmd:kill head-cmd:head
    tail-cmd:tail sleep-cmd:sleep date-cmd:date true-cmd:true
    false-cmd:false clear-cmd:clear tr-cmd:tr uname-cmd:uname
    chess dc expr cal factor yes-cmd:yes tetris nano
    printf-cmd:printf basename-cmd:basename dirname-cmd:dirname
    xargs-cmd:xargs find-cmd:find
)

echo "=== Building kernel ==="
cargo build 2>&1 | grep -v "^warning"

echo ""
echo "=== Building user programs (release, stripped) ==="
CARGO_PKGS=""
for entry in "${PROGS[@]}"; do
    crate="${entry%%:*}"
    CARGO_PKGS="$CARGO_PKGS -p $crate"
done
(cd "$USERDIR" && cargo build --release $CARGO_PKGS 2>&1 | grep -E "Compiling|Finished|error")

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

for entry in "${PROGS[@]}"; do
    crate="${entry%%:*}"
    if [[ "$entry" == *:* ]]; then
        disk_name="${entry#*:}"
    else
        disk_name="$crate"
    fi
    src="$USERDIR/target/$TARGET/release/$crate"
    dst="$STRIPPED_DIR/$disk_name"
    if [ -f "$src" ]; then
        if [ -n "$STRIP_TOOL" ]; then
            "$STRIP_TOOL" --strip-all "$src" -O elf32-i386 "$dst" 2>/dev/null || cp "$src" "$dst"
        else
            cp "$src" "$dst"
        fi
        size=$(wc -c < "$dst" | tr -d ' ')
        echo "  $disk_name: ${size}B"
    else
        echo "  $disk_name: NOT FOUND (skipped)"
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
for entry in "${PROGS[@]}"; do
    crate="${entry%%:*}"
    if [[ "$entry" == *:* ]]; then
        disk_name="${entry#*:}"
    else
        disk_name="$crate"
    fi
    if [ -f "$STRIPPED_DIR/$disk_name" ]; then
        FILES="$FILES $STRIPPED_DIR/$disk_name"
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
