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
# client from mdev: the rule below runs usr/libexec/neptron/net-hotplug.
# The busybox package writes /etc/mdev.conf; the rule is appended once, so
# an output that runs this step again keeps one copy.

set -eu

target=$1

rm -f "$target/etc/init.d/S50mdnsd"

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
mdev_rule='SUBSYSTEM=net;(eth|en).*	root:root 600 @/usr/libexec/neptron/net-hotplug'
if [ ! -f "$mdev_conf" ]; then
	echo "post-build: the image has no /etc/mdev.conf for the network port rule" >&2
	exit 1
fi
if ! grep -qxF "$mdev_rule" "$mdev_conf"; then
	printf '%s\n' "$mdev_rule" >> "$mdev_conf"
fi
