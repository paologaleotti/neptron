#!/bin/sh
# Post-build step of the Raspberry Pi image. Buildroot calls it with the
# target directory as $1 and BR2_CONFIG, BUILD_DIR, BASE_DIR and
# BR2_EXTERNAL_NEPTRON_PATH in the environment.
#
# It checks the kernel (no mode of its own, no kernel patch), adds the
# mount points of the read-only root, the licence texts of `make
# legal-info`, and the SSH key of the development variant. It fails a
# release build that holds a network login.

set -eu

target=$1
board=$BR2_EXTERNAL_NEPTRON_PATH/board/rpi

"$board/check-kernel-config.sh" "$BR2_CONFIG" "$BUILD_DIR" || exit 1
"$board/check-kernel-patches.sh" "$BR2_CONFIG" "$BR2_EXTERNAL_NEPTRON_PATH" || exit 1

mkdir -p "$target/state" "$target/share"

legal=$BASE_DIR/legal-info
if [ ! -f "$legal/manifest.csv" ]; then
	echo "post-build: $legal/manifest.csv is missing; run make legal-info before the build" >&2
	exit 1
fi
rm -rf "$target/usr/share/licenses"
mkdir -p "$target/usr/share/licenses"
cp -r "$legal/licenses/." "$target/usr/share/licenses/"
cp "$legal/manifest.csv" "$target/usr/share/licenses/manifest.csv"
install -D -m 0644 "$board/LICENCES.txt" "$target/usr/share/licenses/LICENCES.txt"

if grep -qx 'BR2_PACKAGE_DROPBEAR=y' "$BR2_CONFIG"; then
	# The development variant: SSH with the one public key given to
	# `cargo xtask os build --dev-key`, and no password login.
	key=/work/dev-key/authorized_keys
	if [ ! -s "$key" ]; then
		echo "post-build: the development variant needs a public key at $key" >&2
		exit 1
	fi
	install -D -m 0600 "$key" "$target/root/.ssh/authorized_keys"
	chmod 0700 "$target/root/.ssh"
	mkdir -p "$target/etc/default"
	echo 'DROPBEAR_ARGS="-s"' > "$target/etc/default/dropbear"
	# The host keys live on the state partition (S49neptron-ssh-keys).
	ln -snf /state/dropbear "$target/etc/dropbear"
else
	# The release image holds no file of the development variant: the SSH
	# server and its settings, a telnet server, and every file of the
	# development overlays that the release overlays do not have too. The
	# lists of the overlays are read from the tree, so a file added to one
	# later is checked as well.
	ext=$BR2_EXTERNAL_NEPTRON_PATH/board
	dev_files=$(for dir in "$ext/pc/dev-overlay" "$ext/rpi/dev-overlay"; do
		(cd "$dir" && find . -type f -o -type l) | sed 's|^\./||'
	done)
	for path in usr/sbin/dropbear root/.ssh etc/default/dropbear etc/init.d/S50dropbear \
		etc/init.d/S50telnet usr/sbin/telnetd $dev_files; do
		if [ -e "$ext/common/rootfs-overlay/$path" ] || [ -e "$ext/pc/rootfs-overlay/$path" ]; then
			continue
		fi
		if [ -e "$target/$path" ] || [ -L "$target/$path" ]; then
			echo "post-build: the release image holds /$path of the development variant; build it in a clean output" >&2
			exit 1
		fi
	done
	launch=$target/usr/libexec/neptron/launch
	if ! cmp -s "$launch" "$ext/common/rootfs-overlay/usr/libexec/neptron/launch"; then
		echo "post-build: the release launch script differs from board/common/rootfs-overlay/usr/libexec/neptron/launch" >&2
		exit 1
	fi
fi
