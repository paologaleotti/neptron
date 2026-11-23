#!/bin/sh
# Checks the final configuration of the PC kernel. The kernel must set no
# mode of its own (the CRT safety model, layer 8), the SMB server of the
# network share must be built in, with no SMB1, no RDMA transport and no
# Kerberos, ACPI must be built in (the power-off of a PC goes through
# it), the default CPU governor must be performance, and IPv6 must be off.
# The fragments set these options, but another option can select one
# again. Exits with 0 when all hold, else with 1 and one line for each
# fault. A missing or unreadable file is a fault.
#
# Usage: check-kernel-config.sh BR2_CONFIG BUILD_DIR [CMDLINE]
#
# CMDLINE is the file with the kernel command line, by default grub.cfg
# beside this script. A test gives another one.
#
# The kernel version comes from BR2_CONFIG, and the file checked is exactly
# BUILD_DIR/linux-<version>/.config: Buildroot keeps the folder of an older
# kernel after a version change, and the outputs are reused.

OPTIONS="VT FB DRM_FBDEV_EMULATION FRAMEBUFFER_CONSOLE DRM_SIMPLEDRM SYSFB_SIMPLEFB \
DRM_PANIC DRM_EFIDRM DRM_VESADRM DRM_CLIENT_LOG"
SHARE_OFF="SMB_INSECURE_SERVER SMB_SERVER_SMBDIRECT SMB_SERVER_KERBEROS5"
SHARE_ON="SMB_SERVER"
POWER_ON="ACPI"
GOVERNOR_ON="CPU_FREQ_DEFAULT_GOV_PERFORMANCE"

br2_config=$1
build_dir=$2
cmdline=${3:-$(dirname "$0")/grub.cfg}
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

# 0 when the file $1 turns IPv6 off on the kernel command line. A missing
# file turns nothing off.
disables_ipv6() {
	grep -v '^[[:space:]]*#' "$1" 2>/dev/null | grep -Eq '(^|[[:space:]])ipv6\.disable=1([[:space:]]|$)'
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
		echo "check-kernel-config: CONFIG_$option is not built in in $config; the power-off of a PC needs it" >&2
		status=1
	fi
done
for option in $GOVERNOR_ON; do
	if ! has "$option" y; then
		echo "check-kernel-config: CONFIG_$option is not set in $config; a lower clock makes a heavy core miss fields" >&2
		status=1
	fi
done
# The guest share is for the local network only, and IPv6
# autoconfiguration can give the port a global address. A module is
# accepted only when the command line turns it off, because the first IPv6
# socket loads it on demand.
if has IPV6 y; then
	echo "check-kernel-config: CONFIG_IPV6 is built in in $config; the network share must not be reachable over IPv6" >&2
	status=1
elif has IPV6 m && ! disables_ipv6 "$cmdline"; then
	echo "check-kernel-config: CONFIG_IPV6 is a module in $config and $cmdline does not set ipv6.disable=1; the network share must not be reachable over IPv6" >&2
	status=1
fi
exit $status
