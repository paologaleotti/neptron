#!/bin/sh
# Checks that the Raspberry Pi kernel gets no kernel patch. The 15 kHz
# patch set of the PC image has an open licence question, and the Pi needs
# nothing from it. A patch can reach the kernel in two ways, and both are
# faults:
#
# - BR2_LINUX_KERNEL_PATCH is not empty (the PC defconfig sets it);
# - a global patch directory other than board/rpi/patches is set (the PC
#   uses os/patches), or board/rpi/patches/linux holds a file other than
#   its hash file.
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

dirs=$(sed -n 's/^BR2_GLOBAL_PATCH_DIR="\(.*\)"$/\1/p' "$br2_config")
if [ -z "$dirs" ]; then
	echo "check-kernel-patches: BR2_GLOBAL_PATCH_DIR is not set; the Pi kernel hash is in board/rpi/patches" >&2
	status=1
fi
for dir in $dirs; do
	case $dir in
	*/board/rpi/patches) ;;
	*)
		echo "check-kernel-patches: the global patch directory '$dir' is not board/rpi/patches" >&2
		status=1
		;;
	esac
done

for pkg in linux linux-headers; do
	for file in "$external/board/rpi/patches/$pkg"/* "$external/board/rpi/patches/$pkg"/.[!.]*; do
		[ -e "$file" ] || [ -L "$file" ] || continue
		if [ "${file##*/}" != "$pkg.hash" ]; then
			echo "check-kernel-patches: $file is in the patch directory of $pkg; it may hold only $pkg.hash" >&2
			status=1
		fi
	done
done
exit $status
