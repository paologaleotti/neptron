#!/bin/sh
# Post-image step of the PC image: the boot partition, the empty state
# partition, and the USB image. Buildroot calls it with the images
# directory as $1 and HOST_DIR, BUILD_DIR and BASE_DIR in the environment.

set -eu

images=$1
board=$BR2_EXTERNAL_NEPTRON_PATH/board/pc

# The disk signature is fixed for each build variant, so the root PARTUUID
# of an image never changes from build to build: "NPT" and R (release),
# D (development), N (release without the 15 kHz patches) or E
# (development without them).
dev=no
khz=no
grep -qx 'BR2_PACKAGE_DROPBEAR=y' "$BR2_CONFIG" && dev=yes
grep -q '^BR2_LINUX_KERNEL_PATCH=".*15khz' "$BR2_CONFIG" && khz=yes
case $dev-$khz in
no-yes) signature=4e505452 ;;
yes-yes) signature=4e505444 ;;
no-no) signature=4e50544e ;;
yes-no) signature=4e505445 ;;
esac
echo "post-image: disk signature $signature, root PARTUUID $signature-02"

# Buildroot installs the GRUB core image but not the boot sector.
boot_img=$(ls "$BUILD_DIR"/grub2-*/build-i386-pc/grub-core/boot.img)
cp "$boot_img" "$images/boot.img"

part=$images/boot-part
rm -rf "$part"
mkdir -p "$part/EFI/BOOT" "$part/boot/grub" "$part/legal"
cp "$images/efi-part/EFI/BOOT/bootx64.efi" "$part/EFI/BOOT/bootx64.efi"
sed "s/@SIGNATURE@/$signature/g" "$board/grub.cfg" > "$part/EFI/BOOT/grub.cfg"
cp "$part/EFI/BOOT/grub.cfg" "$part/boot/grub/grub.cfg"
cp "$board/LICENCES.txt" "$part/LICENCES.txt"
legal=$BASE_DIR/legal-info
cp "$legal/manifest.csv" "$legal/README" "$part/legal/"
cp -r "$legal/licenses" "$part/legal/licenses"

rm -f "$images/state.ext4" "$images/share.exfat"
truncate -s 256M "$images/state.ext4"
"$HOST_DIR/sbin/mkfs.ext4" -q -L neptron-state "$images/state.ext4"

sed "s/@SIGNATURE@/0x$signature/" "$board/genimage.cfg" > "$images/genimage.cfg"
support/scripts/genimage.sh -c "$images/genimage.cfg"
