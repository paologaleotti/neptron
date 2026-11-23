#!/bin/sh
# Checks the SD card image of the Raspberry Pi release variant without a
# Pi: the partition table that os/board/rpi/genimage.cfg describes, the
# release signature, the files of the boot partition, and the release
# command line. Needs sfdisk (util-linux) and mtools.

set -eu
export MTOOLS_SKIP_CHECK=1

img=$1
fail() {
	echo "check-pi-image: $*" >&2
	exit 1
}

[ -f "$img" ] || fail "no image at $img"
dump=$(sfdisk --dump "$img")
echo "$dump"

echo "$dump" | grep -qx 'label: dos' || fail "the table is not MBR"
echo "$dump" | grep -qx 'label-id: 0x4e505052' || fail "the disk signature is not 4e505052 (release)"
# Sectors of 512 bytes: 4 MiB, 132 MiB and 516 MiB; sizes 128, 384, 256 MiB.
for row in '1 8192 262144 c' '2 270336 786432 83' '3 1056768 524288 83'; do
	set -- $row
	echo "$dump" | grep -Eq "^$img$1 : start= *$2, size= *$3, type=$4" ||
		fail "partition $1 is not at sector $2 with $3 sectors and type $4"
done
if echo "$dump" | grep -q "^${img}4 "; then
	fail "the image has a partition 4; the first boot makes the share"
fi

boot="$img@@$((8192 * 512))"
for file in bootcode.bin start.elf fixup.dat config.txt cmdline.txt Image \
	bcm2710-rpi-3-b.dtb bcm2710-rpi-3-b-plus.dtb bcm2710-rpi-zero-2-w.dtb \
	overlays/vc4-kms-v3d.dtbo LICENCES.txt legal/manifest.csv legal/LICENCE.broadcom \
	legal/neptron-rust/neptron.txt legal/neptron-rust/neptron-launch.txt \
	legal/neptron-rust/neptronctl.txt; do
	mdir -i "$boot" "::$file" > /dev/null 2>&1 || fail "the boot partition has no $file"
done
if mdir -i "$boot" ::overlays/disable-bt.dtbo > /dev/null 2>&1; then
	fail "the release image holds the overlay of the development variant"
fi

cmdline=$(mtype -i "$boot" ::cmdline.txt)
echo "cmdline.txt: $cmdline"
case $cmdline in
*root=PARTUUID=4e505052-02*) ;;
*) fail "cmdline.txt does not name the release root" ;;
esac
case " $cmdline " in
*" quiet "*) ;;
*) fail "cmdline.txt of the release image has no quiet" ;;
esac
echo "check-pi-image: the layout of $img is the release layout"
