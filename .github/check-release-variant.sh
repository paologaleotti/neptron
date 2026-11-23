#!/bin/sh
# Checks that an image that CI gives out is the release variant, and what
# it names. Arguments: the image file, the Buildroot output that made it,
# and, for a release, the tag and the commit.
#
# Three checks, from the cheapest: the configuration of the output holds
# no SSH server; the target folder of the output (the root file system of
# the image) holds no file of the SSH server, of a telnet server or of the
# key of the development variant, and its /etc/neptron-release says
# "release"; and the boot partition of the image file itself says
# "Variant: release" in LICENCES.txt. The last one reads the file that is
# given out, so an image from another output fails it. With a tag, the
# two files must name that release and exactly that commit (no
# "-modified"). Needs sudo (the output belongs to the container user),
# sfdisk, jq and mtools.

set -eu
export MTOOLS_SKIP_CHECK=1

img=$1
out=$2
tag=${3:-}
sha=${4:-}
fail() {
	echo "check-release-variant: $*" >&2
	exit 1
}

[ -f "$img" ] || fail "no image at $img"
if sudo grep -qx 'BR2_PACKAGE_DROPBEAR=y' "$out/.config"; then
	fail "the output $out is configured with the SSH server of the development variant"
fi
for path in usr/sbin/dropbear root/.ssh etc/default/dropbear etc/dropbear \
	etc/init.d/S50dropbear usr/sbin/telnetd etc/init.d/S50telnet; do
	if sudo test -e "$out/target/$path" || sudo test -L "$out/target/$path"; then
		fail "the root of the image holds /$path of the development variant"
	fi
done
release=$(sudo cat "$out/target/etc/neptron-release") || fail "the root of the image has no /etc/neptron-release"
echo "$release"
echo "$release" | grep -qx 'NEPTRON_VARIANT=release' || fail "/etc/neptron-release does not say release"

start=$(sfdisk --json "$img" | jq -e '.partitiontable.partitions[0].start') ||
	fail "cannot read the first partition of $img"
notes=$(mtype -i "$img@@$((start * 512))" ::LICENCES.txt) || fail "the boot partition of $img has no LICENCES.txt"
echo "$notes" | grep -qx -- '- Variant: release' || fail "LICENCES.txt of $img does not say Variant: release"

if [ -n "$tag" ]; then
	echo "$release" | grep -qx "NEPTRON_RELEASE=$tag" || fail "/etc/neptron-release does not name $tag"
	echo "$release" | grep -qx "NEPTRON_COMMIT=$sha" || fail "/etc/neptron-release does not name commit $sha"
	echo "$notes" | grep -qx -- "- NEPTRON release: $tag" || fail "LICENCES.txt of $img does not name $tag"
	echo "$notes" | grep -qx -- "- NEPTRON commit: $sha" || fail "LICENCES.txt of $img does not name commit $sha"
fi
echo "check-release-variant: $img is the release variant${tag:+ of $tag}"
