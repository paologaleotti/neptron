#!/bin/sh
# Post-image step of the Raspberry Pi image: the boot partition, the empty
# state partition, and the SD card image. Buildroot calls it with the
# images directory as $1 and HOST_DIR, BUILD_DIR, BASE_DIR and BR2_CONFIG
# in the environment.

set -eu

images=$1
board=$BR2_EXTERNAL_NEPTRON_PATH/board/rpi

# The disk signature is fixed for each build variant, so the root PARTUUID
# of an image never changes from build to build: "NPP" and R (release) or
# D (development).
dev=no
grep -qx 'BR2_PACKAGE_DROPBEAR=y' "$BR2_CONFIG" && dev=yes
case $dev in
no) signature=4e505052 ;;
yes) signature=4e505044 ;;
esac
echo "post-image: disk signature $signature, root PARTUUID $signature-02"

part=$images/boot-part
rm -rf "$part"
mkdir -p "$part/overlays" "$part/legal"
for file in bootcode.bin start.elf fixup.dat; do
	cp "$images/rpi-firmware/$file" "$part/$file"
done
cp "$images/Image" "$part/Image"
for dtb in bcm2710-rpi-3-b bcm2710-rpi-3-b-plus bcm2710-rpi-zero-2-w; do
	cp "$images/$dtb.dtb" "$part/$dtb.dtb"
done
cp "$images/vc4-kms-v3d.dtbo" "$part/overlays/vc4-kms-v3d.dtbo"
cp "$board/config.txt" "$part/config.txt"
if [ "$dev" = yes ]; then
	cp "$images/disable-bt.dtbo" "$part/overlays/disable-bt.dtbo"
	cat "$board/config-dev.txt" >> "$part/config.txt"
fi
variant=release
[ "$dev" = yes ] && variant=dev
"$board/cmdline.sh" "$board/cmdline.txt" "$signature" "$variant" > "$part/cmdline.txt"

cp "$board/LICENCES.txt" "$part/LICENCES.txt"
legal=$BASE_DIR/legal-info
cp "$legal/manifest.csv" "$legal/README" "$part/legal/"
cp -r "$legal/licenses" "$part/legal/licenses"
# The firmware licence asks that its notice and disclaimer come with the
# files; it is also in legal/licenses, under the package version.
broadcom=$(ls "$BUILD_DIR"/rpi-firmware-*/boot/LICENCE.broadcom)
cp "$broadcom" "$part/legal/LICENCE.broadcom"

rm -f "$images/state.ext4"
truncate -s 256M "$images/state.ext4"
"$HOST_DIR/sbin/mkfs.ext4" -q -L neptron-state "$images/state.ext4"

sed "s/@SIGNATURE@/0x$signature/" "$board/genimage.cfg" > "$images/genimage.cfg"
support/scripts/genimage.sh -c "$images/genimage.cfg"
