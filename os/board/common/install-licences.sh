#!/bin/sh
# Puts the licence files of the PC and Pi images in /usr/share/licenses of
# the target folder $1: the texts and the manifest of `make legal-info`,
# the notes of the board ($2, its LICENCES.txt), and the notices of the
# Rust crates in each NEPTRON program of the image. The package of each
# program writes those notices to $BASE_DIR/neptron-notices (see
# NEPTRON_WORKSPACE_NOTICES in external.mk). The post-build step of the
# board calls it with BASE_DIR in the environment.
#
# The folder is made anew on each run, so it holds only the texts of the
# current legal-info output. A NEPTRON Rust program in the image with no
# notices file fails the build.

set -eu

target=$1
notes=$2
legal=$BASE_DIR/legal-info
notices=$BASE_DIR/neptron-notices
licenses=$target/usr/share/licenses

if [ ! -f "$legal/manifest.csv" ]; then
	echo "install-licences: $legal/manifest.csv is missing; run make legal-info before the build" >&2
	exit 1
fi
rm -rf "$licenses"
mkdir -p "$licenses"
# Plain cp and mkdir, so the test of this script runs on macOS too.
cp -r "$legal/licenses/." "$licenses/"
cp "$legal/manifest.csv" "$licenses/manifest.csv"
cp "$notes" "$licenses/LICENCES.txt"

for program in neptron neptron-launch neptronctl; do
	[ -e "$target/usr/bin/$program" ] || continue
	if [ ! -s "$notices/$program.txt" ]; then
		echo "install-licences: /usr/bin/$program has no Rust crate notices at $notices/$program.txt" >&2
		exit 1
	fi
	mkdir -p "$licenses/neptron-rust"
	cp "$notices/$program.txt" "$licenses/neptron-rust/$program.txt"
done
