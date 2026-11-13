#!/bin/sh
# Prints the kernel command line of the boot partition: cmdline.txt with
# the disk signature of the build variant in place of @SIGNATURE@. The
# development variant drops "quiet", so its serial console shows the whole
# kernel log from the start; the release image keeps it.
#
# Usage: cmdline.sh TEMPLATE SIGNATURE release|dev

set -eu

template=$1
signature=$2
variant=$3
case $variant in
release) sed "s/@SIGNATURE@/$signature/g" "$template" ;;
dev) sed -e "s/@SIGNATURE@/$signature/g" -e 's/ quiet / /' "$template" ;;
*)
	echo "cmdline.sh: the variant is release or dev, not '$variant'" >&2
	exit 1
	;;
esac
