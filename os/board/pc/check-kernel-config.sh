#!/bin/sh
# Checks the final configuration of the PC kernel: the kernel must set no
# mode of its own (the CRT safety model, layer 8). The fragments turn these
# options off, but another option can select one again. Exits with 0 when
# all are off, else with 1 and one line for each fault. A missing or
# unreadable file is a fault.
#
# Usage: check-kernel-config.sh BR2_CONFIG BUILD_DIR
#
# The kernel version comes from BR2_CONFIG, and the file checked is exactly
# BUILD_DIR/linux-<version>/.config: Buildroot keeps the folder of an older
# kernel after a version change, and the outputs are reused.

OPTIONS="VT FB DRM_FBDEV_EMULATION FRAMEBUFFER_CONSOLE DRM_SIMPLEDRM SYSFB_SIMPLEFB \
DRM_PANIC DRM_EFIDRM DRM_VESADRM DRM_CLIENT_LOG"

br2_config=$1
build_dir=$2
version=
if [ -f "$br2_config" ]; then
	version=$(sed -n 's/^BR2_LINUX_KERNEL_CUSTOM_VERSION_VALUE="\(.*\)"$/\1/p' "$br2_config")
fi
if [ -z "$version" ]; then
	echo "check-kernel-config: no kernel version in '$br2_config'" >&2
	exit 1
fi
config=$build_dir/linux-$version/.config
if [ ! -f "$config" ] || [ ! -r "$config" ]; then
	echo "check-kernel-config: the kernel configuration '$config' is missing" >&2
	exit 1
fi
status=0
for option in $OPTIONS; do
	grep -q "^CONFIG_$option=[ym]" "$config"
	case $? in
	0)
		echo "check-kernel-config: CONFIG_$option is on in $config; the kernel must set no mode" >&2
		status=1
		;;
	1) ;;
	*)
		echo "check-kernel-config: cannot read $config" >&2
		exit 1
		;;
	esac
done
exit $status
