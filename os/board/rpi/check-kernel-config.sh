#!/bin/sh
# Checks the final configuration of the Raspberry Pi kernel. The kernel
# must set no mode of its own (the CRT safety model, layer 8), the SMB
# server of the network share must be built in, with no SMB1, no RDMA
# transport and no Kerberos, the watchdog driver must be built in (the
# power-off and the reboot of a Pi go through it), and vc4 must be built
# in with the I2C controller of the HDMI DDC bus. The fragments set these
# options, but another option can select one again. Exits with 0 when all
# hold, else with 1 and one line for each fault. A missing or unreadable
# file is a fault.
#
# Usage: check-kernel-config.sh BR2_CONFIG BUILD_DIR
#
# The kernel of this board is a custom tarball, which Buildroot builds in
# BUILD_DIR/linux-custom. A configuration with another kernel source is a
# fault: the file checked would not be the kernel of the image.

OPTIONS="VT FB DRM_FBDEV_EMULATION FRAMEBUFFER_CONSOLE DRM_SIMPLEDRM SYSFB_SIMPLEFB \
DRM_PANIC DRM_EFIDRM DRM_VESADRM DRM_CLIENT_LOG"
SHARE_OFF="SMB_INSECURE_SERVER SMB_SERVER_SMBDIRECT SMB_SERVER_KERBEROS5"
SHARE_ON="SMB_SERVER"
POWER_ON="BCM2835_WDT"
DISPLAY_ON="DRM_VC4 I2C_BCM2835"

br2_config=$1
build_dir=$2
if [ ! -f "$br2_config" ] || ! grep -qx 'BR2_LINUX_KERNEL_CUSTOM_TARBALL=y' "$br2_config"; then
	echo "check-kernel-config: '$br2_config' does not build the kernel from a custom tarball" >&2
	exit 1
fi
config=$build_dir/linux-custom/.config
if [ ! -f "$config" ] || [ ! -r "$config" ]; then
	echo "check-kernel-config: the kernel configuration '$config' is missing" >&2
	exit 1
fi
status=0

# 0 when CONFIG_$1 has a value that matches $2; exits on a read error.
has() {
	grep -q "^CONFIG_$1=$2\$" "$config"
	code=$?
	if [ "$code" -gt 1 ]; then
		echo "check-kernel-config: cannot read $config" >&2
		exit 1
	fi
	return "$code"
}

for option in $OPTIONS; do
	if has "$option" '[ym]'; then
		echo "check-kernel-config: CONFIG_$option is on in $config; the kernel must set no mode" >&2
		status=1
	fi
done
for option in $SHARE_OFF; do
	if has "$option" '[ym]'; then
		echo "check-kernel-config: CONFIG_$option is on in $config; the network share serves guests over SMB2 or newer only" >&2
		status=1
	fi
done
for option in $SHARE_ON; do
	if ! has "$option" y; then
		echo "check-kernel-config: CONFIG_$option is not built in in $config; the network share needs it" >&2
		status=1
	fi
done
for option in $POWER_ON; do
	if ! has "$option" y; then
		echo "check-kernel-config: CONFIG_$option is not built in in $config; the power-off and the reboot of a Pi need it" >&2
		status=1
	fi
done
for option in $DISPLAY_ON; do
	if ! has "$option" y; then
		echo "check-kernel-config: CONFIG_$option is not built in in $config; the display of a Pi needs it" >&2
		status=1
	fi
done
exit $status
