#!/bin/sh
# Checks that the Raspberry Pi kernel gets no kernel patch. The 15 kHz
# patch set of the PC image has an open licence question, and the Pi needs
# nothing from it. A patch can reach the kernel in two ways, and both are
# faults:
#
# - BR2_LINUX_KERNEL_PATCH is not empty (the PC defconfig sets it);
# - BR2_GLOBAL_PATCH_DIR is not exactly the patch directory of this board
#   (the PC uses os/patches, and a second directory would add its patches),
#   or that directory holds any file other than the two hash files.
#
# Exits with 0 when no patch can reach the kernel, else with 1 and one
# line for each fault.
#
# Usage: check-kernel-patches.sh BR2_CONFIG EXTERNAL_DIR

br2_config=$1
external=$2
if [ ! -r "$br2_config" ]; then
	echo "check-kernel-patches: cannot read '$br2_config'" >&2
	exit 1
fi
status=0

patch=$(sed -n 's/^BR2_LINUX_KERNEL_PATCH="\(.*\)"$/\1/p' "$br2_config")
if [ -n "$patch" ]; then
	echo "check-kernel-patches: BR2_LINUX_KERNEL_PATCH is '$patch'; the Pi kernel takes no patch" >&2
	status=1
fi

# The one patch directory of this board, as the defconfig writes it.
PATCH_DIR='$(BR2_EXTERNAL_NEPTRON_PATH)/board/rpi/patches'
# The only files it may hold: the hashes of the Pi kernel.
ALLOWED="linux/linux.hash linux-headers/linux-headers.hash"

dirs=$(sed -n 's/^BR2_GLOBAL_PATCH_DIR="\(.*\)"$/\1/p' "$br2_config")
if [ "$dirs" != "$PATCH_DIR" ]; then
	echo "check-kernel-patches: BR2_GLOBAL_PATCH_DIR is '$dirs', not '$PATCH_DIR'" >&2
	status=1
fi

# Every file and folder below the patch directory, by its path in it.
patches=$external/board/rpi/patches
if [ ! -d "$patches" ]; then
	echo "check-kernel-patches: $patches is missing" >&2
	exit 1
fi
for path in $(cd "$patches" && find . -mindepth 1 | sed 's|^\./||'); do
	[ -d "$patches/$path" ] && [ ! -L "$patches/$path" ] && continue
	case " $ALLOWED " in
	*" $path "*) ;;
	*)
		echo "check-kernel-patches: $patches/$path is in the patch directory; it may hold only $ALLOWED" >&2
		status=1
		;;
	esac
done
exit $status
