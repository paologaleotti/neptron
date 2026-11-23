#!/bin/sh
# Puts the licence files of the PC and Pi images in /usr/share/licenses of
# the target folder $1: the texts and the manifest of `make legal-info`,
# the notes of the board ($2, its LICENCES.txt), and the notices of the
# Rust crates in each NEPTRON program of the image. It also writes
# /etc/neptron-release, which names the source of the image. The package
# of each program writes its notices to $BASE_DIR/neptron-notices (see
# NEPTRON_WORKSPACE_NOTICES in external.mk). The post-build step of the
# board calls it with BASE_DIR and BR2_CONFIG in the environment.
#
# The folder is made anew on each run, so it holds only the texts of the
# current legal-info output. A NEPTRON Rust program in the image with no
# notices file fails the build.
#
# `cargo xtask os build` passes the NEPTRON commit, the commit date and the
# release tag in NEPTRON_SOURCE_COMMIT, NEPTRON_SOURCE_DATE and
# NEPTRON_SOURCE_RELEASE. A build without them (make run by hand) names
# them "unknown" and "none". A value of another form fails the build, so
# no text reaches the notes unchecked. The notes get the values in place
# of @NEPTRON_COMMIT@, @NEPTRON_DATE@, @NEPTRON_RELEASE@ and
# @NEPTRON_VARIANT@.

set -eu

target=$1
notes=$2
legal=$BASE_DIR/legal-info
notices=$BASE_DIR/neptron-notices
licenses=$target/usr/share/licenses

fail() {
	echo "install-licences: $*" >&2
	exit 1
}

commit=${NEPTRON_SOURCE_COMMIT:-unknown}
date=${NEPTRON_SOURCE_DATE:-unknown}
release=${NEPTRON_SOURCE_RELEASE:-none}
hash=${commit%-modified}
case $commit in
unknown) ;;
*)
	case $hash in
	*[!0-9a-f]*) fail "NEPTRON_SOURCE_COMMIT is not a commit: $commit" ;;
	esac
	[ ${#hash} -eq 40 ] || fail "NEPTRON_SOURCE_COMMIT is not a commit: $commit"
	;;
esac
case $date in
unknown) ;;
[0-9][0-9][0-9][0-9]-[01][0-9]-[0-3][0-9]T[0-2][0-9]:[0-5][0-9]:[0-5][0-9]Z) ;;
*) fail "NEPTRON_SOURCE_DATE is not a UTC date: $date" ;;
esac
case $release in
none) ;;
v[0-9]*.[0-9]*.[0-9]*)
	case ${release#v} in
	*[!0-9A-Za-z.-]*) fail "NEPTRON_SOURCE_RELEASE is not a release tag: $release" ;;
	esac
	;;
*) fail "NEPTRON_SOURCE_RELEASE is not a release tag: $release" ;;
esac
variant=release
if grep -qx 'BR2_PACKAGE_DROPBEAR=y' "$BR2_CONFIG"; then
	variant=development
fi

if [ ! -f "$legal/manifest.csv" ]; then
	fail "$legal/manifest.csv is missing; run make legal-info before the build"
fi
rm -rf "$licenses"
mkdir -p "$licenses"
# Plain cp and mkdir, so the test of this script runs on macOS too.
cp -r "$legal/licenses/." "$licenses/"
cp "$legal/manifest.csv" "$licenses/manifest.csv"
sed -e "s/@NEPTRON_COMMIT@/$commit/g" -e "s/@NEPTRON_DATE@/$date/g" \
	-e "s/@NEPTRON_RELEASE@/$release/g" -e "s/@NEPTRON_VARIANT@/$variant/g" \
	"$notes" > "$licenses/LICENCES.txt"
if grep -q '@NEPTRON_[A-Z]*@' "$licenses/LICENCES.txt"; then
	fail "$notes holds a placeholder that this script does not fill"
fi

for program in neptron neptron-launch neptronctl; do
	[ -e "$target/usr/bin/$program" ] || continue
	if [ ! -s "$notices/$program.txt" ]; then
		fail "/usr/bin/$program has no Rust crate notices at $notices/$program.txt"
	fi
	mkdir -p "$licenses/neptron-rust"
	cp "$notices/$program.txt" "$licenses/neptron-rust/$program.txt"
done

mkdir -p "$target/etc"
printf 'NEPTRON_RELEASE=%s\nNEPTRON_COMMIT=%s\nNEPTRON_COMMIT_DATE=%s\nNEPTRON_VARIANT=%s\n' \
	"$release" "$commit" "$date" "$variant" > "$target/etc/neptron-release"
