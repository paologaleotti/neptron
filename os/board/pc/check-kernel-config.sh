#!/bin/sh
# Checks the final configuration of the PC kernel: the kernel must set no
# mode of its own (the CRT safety model, layer 8). The fragments turn these
# options off, but another option can select one again. Exits with 0 when
# all are off, else with 1 and one line for each fault. A missing or
# unreadable file is a fault.
#
# Usage: check-kernel-config.sh FILE

OPTIONS="VT FB DRM_FBDEV_EMULATION FRAMEBUFFER_CONSOLE DRM_SIMPLEDRM SYSFB_SIMPLEFB \
DRM_PANIC DRM_EFIDRM DRM_VESADRM DRM_CLIENT_LOG"

config=$1
if [ -z "$config" ] || [ ! -f "$config" ] || [ ! -r "$config" ]; then
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
