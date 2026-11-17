#!/bin/sh
# Fails when the image holds the FinalBurn Neo core and no arcade index.
# The frontend still starts with no index, but it then lists each arcade
# zip by its file name and checks no set before launch, so an image built
# that way is a broken build, not a valid one. Buildroot gives the target
# directory as $1.

set -eu

target=$1
core=$target/usr/lib/libretro/fbneo_libretro.so
index=$target/usr/share/neptron/arcade/fbneo.idx

if [ -e "$core" ] && [ ! -s "$index" ]; then
	echo "post-build: the image has the FinalBurn Neo core and no arcade index /usr/share/neptron/arcade/fbneo.idx; the neptron package makes it in its install step" >&2
	exit 1
fi
