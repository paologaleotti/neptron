#!/bin/sh
# The linker of the Rust programs of the image: the Buildroot compiler
# wrapper, given in NEPTRON_LINK_CC, without the library path /usr/local/lib.
#
# The build script of the sdl3 crate adds /usr/local/lib to the link search
# on every Linux host. In a cross build that path is a directory of the
# build machine, and the wrapper refuses it as an error. The image has
# nothing there, so the path is dropped. Every other argument passes
# unchanged, and the wrapper still checks them.

pending=0
for arg do
	shift
	if [ "$pending" = 1 ]; then
		pending=0
		[ "$arg" = /usr/local/lib ] && continue
		set -- "$@" -L "$arg"
		continue
	fi
	case $arg in
	-L)
		pending=1
		continue
		;;
	-L/usr/local/lib)
		continue
		;;
	esac
	set -- "$@" "$arg"
done
[ "$pending" = 1 ] && set -- "$@" -L

exec $NEPTRON_LINK_CC "$@"
