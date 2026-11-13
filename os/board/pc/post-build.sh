#!/bin/sh
# Post-build step of the PC image. Buildroot calls it with the target
# directory as $1 and BR2_CONFIG, BASE_DIR and BINARIES_DIR in the
# environment.
#
# It adds the mount points of the read-only root, the licence texts of
# `make legal-info`, and the SSH key of the development variant. It fails a
# release build that holds a network login.

set -eu

target=$1

# The kernel must set no mode of its own: check-kernel-config.sh reads the
# final configuration of the configured kernel version.
"$BR2_EXTERNAL_NEPTRON_PATH/board/pc/check-kernel-config.sh" "$BR2_CONFIG" "$BUILD_DIR" || exit 1

mkdir -p "$target/state" "$target/share"
# GRUB reads its configuration from the boot partition, not from here.
rm -rf "$target/boot/grub"

legal=$BASE_DIR/legal-info
if [ ! -f "$legal/manifest.csv" ]; then
	echo "post-build: $legal/manifest.csv is missing; run make legal-info before the build" >&2
	exit 1
fi
rm -rf "$target/usr/share/licenses"
mkdir -p "$target/usr/share/licenses"
cp -r "$legal/licenses/." "$target/usr/share/licenses/"
cp "$legal/manifest.csv" "$target/usr/share/licenses/manifest.csv"
install -D -m 0644 "$BR2_EXTERNAL_NEPTRON_PATH/board/pc/LICENCES.txt" \
	"$target/usr/share/licenses/LICENCES.txt"

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
	# development overlay that the release overlays do not have too. The
	# list of the overlay is read from the tree, so a file added to it
	# later is checked as well.
	ext=$BR2_EXTERNAL_NEPTRON_PATH/board
	dev_files=$(cd "$ext/pc/dev-overlay" && find . -type f -o -type l | sed 's|^\./||')
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
	if [ ! -f "$launch" ]; then
		echo "post-build: the release image has no launch script at /usr/libexec/neptron/launch" >&2
		exit 1
	fi
	if grep -q /state/hot "$launch"; then
		echo "post-build: the release launch script reads the hot folder of the development variant" >&2
		exit 1
	fi
fi
