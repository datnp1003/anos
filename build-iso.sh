#!/bin/bash
# 🦾 AnosOS ISO Builder
# Builds a bootable ISO with Anos as PID 1
set -euo pipefail

OUTPUT="${1:-anos-os.iso}"
ARCH="${2:-amd64}"
ROOTFS="/tmp/anos-rootfs"
ISO_LABEL="ANOS_OS"

echo "🦾 AnosOS ISO Builder"
echo "  Output: $OUTPUT"
echo "  Arch:   $ARCH"
echo ""

# ── 1. Prepare rootfs ──
echo "📦 Creating rootfs..."
rm -rf "$ROOTFS"
mkdir -p "$ROOTFS"/{bin,boot,dev,etc,opt/anos/{config,skills},proc,run,sys,tmp,usr/bin,var/log}

# Copy Anos binaries (must be built first)
cp /usr/local/bin/anosd "$ROOTFS/usr/bin/"
cp /usr/local/bin/anos-cli "$ROOTFS/usr/bin/"
cp /opt/anos/ANOS-SYSTEM-PROMPT.md "$ROOTFS/opt/anos/"
cp -r /opt/anos/skills/* "$ROOTFS/opt/anos/skills/"
cp /opt/anos/anos-init "$ROOTFS/sbin/init"
chmod +x "$ROOTFS/sbin/init"

# Copy busybox
cp /bin/busybox "$ROOTFS/bin/"
for util in sh ls cat echo mount umount ip ping hostname modprobe mknod sleep grep; do
    ln -sf /bin/busybox "$ROOTFS/bin/$util" 2>/dev/null || true
done

# Copy DHCP client
if command -v udhcpc &>/dev/null; then
    cp $(which udhcpc) "$ROOTFS/usr/bin/"
elif [ -f /sbin/udhcpc ]; then
    cp /sbin/udhcpc "$ROOTFS/usr/bin/"
fi

# ── 2. Create GRUB config ──
echo "🖥️ Creating GRUB config..."
mkdir -p "$ROOTFS/boot/grub"

cat > "$ROOTFS/boot/grub/grub.cfg" << 'GRUB'
set timeout=5
set default=0
loadfont unicode

menuentry "🦾 AnosOS — AI Native OS" {
    linux /boot/vmlinuz console=tty1 quiet
    initrd /boot/initrd.img
}

menuentry "AnosOS — Verbose boot" {
    linux /boot/vmlinuz console=tty1
    initrd /boot/initrd.img
}
GRUB

# ── 3. Copy kernel ──
echo "🐧 Copying kernel..."
if [ -f /boot/vmlinuz ]; then
    cp /boot/vmlinuz "$ROOTFS/boot/vmlinuz"
elif [ -f /vmlinuz ]; then
    cp /vmlinuz "$ROOTFS/boot/vmlinuz"
else
    KERNEL=$(ls /boot/vmlinuz-* 2>/dev/null | head -1)
    [ -n "$KERNEL" ] && cp "$KERNEL" "$ROOTFS/boot/vmlinuz"
fi

# ── 4. Create initrd from rootfs ──
echo "📦 Creating squashfs from rootfs..."
mksquashfs "$ROOTFS" /tmp/anos-root.squashfs -comp xz -noappend -quiet

# Create initrd (just squashfs → cpio)
mkdir -p /tmp/initrd-root
cp /tmp/anos-root.squashfs /tmp/initrd-root/anos.squashfs
(cd /tmp/initrd-root && find . | cpio -o -H newc) > /tmp/initrd.img
cp /tmp/initrd.img "$ROOTFS/boot/initrd.img"

# ── 5. Build ISO ──
echo "💿 Building ISO..."
grub-mkrescue -o "$OUTPUT" "$ROOTFS" \
    --modules="part_gpt fat iso9660 normal boot linux configfile" \
    --fonts="" \
    2>&1 | grep -v "^xorriso\|^GNU\|^Disk\|^libisofs" || true

# ── 6. Show result ──
echo ""
echo "✅ Done!"
ls -lh "$OUTPUT"
echo ""
echo "Test with: qemu-system-x86_64 -cdrom $OUTPUT -m 1024"
