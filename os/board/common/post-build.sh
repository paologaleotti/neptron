#!/bin/sh
# Post-build step of both images. Buildroot calls it with the target
# directory as $1.
#
# The mdnsd package installs its own init script, which would start mdnsd
# whatever the switch of the network share says, with the services of
# /etc/mdns.d. S45neptron-share starts mdnsd only when the share is on,
# with its own service folder, so the package script goes.
#
# S40neptron-network starts DHCP in the background. Buildroot does not
# install ifupdown-scripts again when BR2_SYSTEM_DHCP changes, so an output
# built with it keeps a DHCP stanza in etc/network/interfaces: a second
# DHCP client, which S40network waits for at boot. The file is written
# here with the loopback only, in every build.
#
# A wired port that appears after S40neptron-network ran (a USB Ethernet
# chip, such as the one of a Raspberry Pi, or a USB adapter) gets its DHCP
# client from mdev: the rule below runs usr/libexec/neptron/net-hotplug
# when a port is added or removed ("*"). The busybox package writes
# /etc/mdev.conf. mdev takes the first rule that matches, so every older
# line of net-hotplug goes before the rule is appended: an output that
# runs this step again keeps one copy, the current one.
#
# The hook of udhcpc (usr/share/udhcpc/default.script.d/neptron-address)
# writes /run/neptron/network.toml at each lease event. Buildroot's busybox
# package installs the udhcpc script that runs the hooks; the build fails
# when that script or the hook is missing. An output built before keeps
# the old address script of the development overlay, which wrote
# /share/neptron-ip.txt once at boot, so it goes. The address script takes
# its lock with the flock command (util-linux or the BusyBox applet; the PC
# and Pi images install the util-linux one); the build fails when the
# image has no /usr/bin/flock.
#
# check-arcade-index.sh fails the build when the FinalBurn Neo core is in
# the image and the arcade index of the frontend is not.
# check-gamecontrollerdb.sh fails it when the configuration enables the
# community gamepad database and the image has no database file.
#
# S05neptron-mounts mounts the 9p share of the QEMU checks at /mnt/host.
# The root of the PC and Pi images is read-only EROFS, so the mount point
# must be in the image.

set -eu

target=$1

"$(dirname "$0")/check-arcade-index.sh" "$target"
"$(dirname "$0")/check-gamecontrollerdb.sh" "$BR2_CONFIG" "$target"

rm -f "$target/etc/init.d/S50mdnsd" "$target/etc/init.d/S41neptron-address"
mkdir -p "$target/mnt/host"

interfaces=$target/etc/network/interfaces
mkdir -p "${interfaces%/*}"
if ! printf 'auto lo\niface lo inet loopback\n' 2>/dev/null > "$interfaces"; then
	echo "post-build: cannot write /etc/network/interfaces of the image" >&2
	exit 1
fi
if grep -q 'inet dhcp' "$interfaces"; then
	echo "post-build: /etc/network/interfaces of the image still starts DHCP; S40neptron-network does it" >&2
	exit 1
fi
for path in etc/init.d/S45neptron-share etc/ksmbd/ksmbd.conf etc/ksmbd/ksmbd.subauth etc/ksmbd/ksmbdpwd.db \
	etc/neptron/mdns.d/smb.service; do
	if [ ! -f "$target/$path" ]; then
		echo "post-build: the image has no /$path of the network share" >&2
		exit 1
	fi
done

mdev_conf=$target/etc/mdev.conf
mdev_rule='SUBSYSTEM=net;(eth|en).*	root:root 600 */usr/libexec/neptron/net-hotplug'
if [ ! -f "$mdev_conf" ]; then
	echo "post-build: the image has no /etc/mdev.conf for the network port rule" >&2
	exit 1
fi
sed '\|/usr/libexec/neptron/net-hotplug$|d' "$mdev_conf" > "$mdev_conf.new"
mv "$mdev_conf.new" "$mdev_conf"
if [ -s "$mdev_conf" ] && [ -n "$(tail -c 1 "$mdev_conf")" ]; then
	echo >> "$mdev_conf"
fi
printf '%s\n' "$mdev_rule" >> "$mdev_conf"

udhcpc_script=$target/usr/share/udhcpc/default.script
if ! grep -q 'HOOK_DIR="$0.d"' "$udhcpc_script" 2>/dev/null; then
	echo "post-build: the image has no /usr/share/udhcpc/default.script that runs the hooks in default.script.d" >&2
	exit 1
fi
if [ ! -x "$udhcpc_script.d/neptron-address" ] || [ ! -x "$target/usr/libexec/neptron/net-address" ]; then
	echo "post-build: the image has no executable udhcpc hook neptron-address or /usr/libexec/neptron/net-address" >&2
	exit 1
fi
if [ ! -e "$target/usr/bin/flock" ] && [ ! -L "$target/usr/bin/flock" ]; then
	echo "post-build: the image has no /usr/bin/flock for the lock of /usr/libexec/neptron/net-address" >&2
	exit 1
fi
