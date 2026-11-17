#!/bin/sh
# Fails when the configuration enables the community gamepad database and
# the image has no database, or an empty one. The frontend still starts
# with no file, but every pad that only the database knows then gets the
# generic layout, so an image built that way is a broken build. Buildroot
# gives the configuration as $BR2_CONFIG; this script takes it as $1 and
# the target directory as $2.

set -eu

config=$1
target=$2
db=$target/usr/share/sdl-gamecontrollerdb/gamecontrollerdb.txt

if grep -qx 'BR2_PACKAGE_SDL_GAMECONTROLLERDB=y' "$config" && [ ! -s "$db" ]; then
	echo "post-build: the configuration enables sdl-gamecontrollerdb and the image has no /usr/share/sdl-gamecontrollerdb/gamecontrollerdb.txt" >&2
	exit 1
fi
