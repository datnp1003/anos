#!/bin/bash
# 🦾 AnosOS ISO Builder — Simple initrd approach
# Kernel boots → initrd in RAM → anosd as init → anos-cli
set -euo pipefail

OUTPUT="${1:-/output/anos-os-arm64.iso}"
WORK="/tmp/anos-build"

echo "🦾 AnosOS ISO Builder (initrd approach)"
echo ""

rm -rf "$WORK"
mkdir -p "$WORK"/{bin,boot,sbin,etc,dev,proc,sys,tmp,usr/bin,opt/anos/{config,skills}}

# ── 1. Copy Anos + busybox + deps ──
echo "📦 Preparing rootfs..."
cp /build/anosd "$WORK/usr/bin/"
cp /build/anos-cli "$WORK/usr/bin/"
cp /build/ANOS-SYSTEM-PROMPT.md "$WORK/opt/anos/"
cp -r /build/skills/* "$WORK/opt/anos/skills/"
cp /usr/bin/genisoimage "$WORK/usr/bin/" 2>/dev/null || true

# Busybox symlinks
cp /bin/busybox "$WORK/bin/"
for util in sh ls cat echo mount umount ip ping hostname modprobe mknod sleep grep mkdir ln rm mv cp; do
    ln -sf /bin/busybox "$WORK/bin/$util" 2>/dev/null || true
done
cp /usr/sbin/udhcpc "$WORK/sbin/" 2>/dev/null || true

# ── 2. Init script ──
cp /build/anos-init "$WORK/sbin/init"
chmod +x "$WORK/sbin/init" "$WORK/usr/bin/anosd" "$WORK/usr/bin/anos-cli"

# ── 3. Build initrd (cpio → gzip) ──
echo "📦 Building initrd..."
INITRD="$WORK/boot/initrd.img"
(cd "$WORK" && find . -path ./boot -prune -o -print | cpio -o -H newc 2>/dev/null) | gzip > "$INITRD"
INITRD_SIZE=$(du -h "$INITRD" | cut -f1)
echo "   Initrd: $INITRD_SIZE"

# ── 4. Copy kernel ──
echo "🐧 Copying kernel..."
KERNEL=$(ls /boot/vmlinuz-* 2>/dev/null | head -1)
cp "$KERNEL" "$WORK/boot/vmlinuz"

# ── 5. Build ISO ──
echo "💿 Building ISO..."
rm -f "$OUTPUT"
mkdir -p "$(dirname "$OUTPUT")"

# GRUB config
mkdir -p "$WORK/boot/grub"
cat > "$WORK/boot/grub/grub.cfg" << 'GRUB'
set timeout=5
menuentry "🦾 AnosOS" {
    linux /boot/vmlinuz console=tty1 quiet
    initrd /boot/initrd.img
}
GRUB

# Use mkisofs/genisoimage
genisoimage -R -r -J \
    -V "ANOS_OS" \
    -o "$OUTPUT" \
    -b boot/grub/grub.cfg \
    -no-emul-boot \
    "$WORK" 2>&1 | tail -3

echo ""
echo "╔══════════════════════════════════╗"
echo "║  ✅ AnosOS ISO Ready!           ║"
echo "╚══════════════════════════════════╝"
ISO_SIZE=$(du -h "$OUTPUT" 2>/dev/null | cut -f1)
echo "   Size: $ISO_SIZE"
echo "   Initrd: $INITRD_SIZE"
echo ""
echo "Test: qemu-system-aarch64 -M virt -cpu cortex-a57 -m 1024 -cdrom $OUTPUT"
