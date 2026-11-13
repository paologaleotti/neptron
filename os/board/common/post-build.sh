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
for path in etc/init.d/S45neptron-share etc/ksmbd/ksmbd.conf etc/ksmbd/ksmbd.subauth \
	etc/neptron/mdns.d/smb.service; do
	if [ ! -f "$target/$path" ]; then
		echo "post-build: the image has no /$path of the network share" >&2
		exit 1
	fi
done
