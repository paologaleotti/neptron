#!/bin/sh
# Post-build step of both images. Buildroot calls it with the target
# directory as $1.
#
# The mdnsd package installs its own init script, which would start mdnsd
# whatever the switch of the network share says, with the services of
# /etc/mdns.d. S45neptron-share starts mdnsd only when the share is on,
# with its own service folder, so the package script goes.

set -eu

target=$1

rm -f "$target/etc/init.d/S50mdnsd"
for path in etc/init.d/S45neptron-share etc/ksmbd/ksmbd.conf etc/ksmbd/ksmbd.subauth \
	etc/neptron/mdns.d/smb.service; do
	if [ ! -f "$target/$path" ]; then
		echo "post-build: the image has no /$path of the network share" >&2
		exit 1
	fi
done
