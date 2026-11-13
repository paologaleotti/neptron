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

# The kernel must set no mode of its own: check-kernel-config.sh reads its
# final configuration. The headers package has a folder of the same name
# and no .config, so the glob asks for a version number first.
kconfig=$(ls -d "$BUILD_DIR"/linux-[0-9]*/.config 2>/dev/null | head -n 1)
"$BR2_EXTERNAL_NEPTRON_PATH/board/pc/check-kernel-config.sh" "$kconfig" || exit 1

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
	for path in usr/sbin/dropbear root/.ssh etc/default/dropbear etc/init.d/S50dropbear \
		etc/init.d/S50telnet usr/sbin/telnetd etc/neptron/development \
		etc/profile.d/neptron-hot.sh etc/init.d/S41neptron-address \
		etc/init.d/S49neptron-ssh-keys; do
		if [ -e "$target/$path" ] || [ -L "$target/$path" ]; then
			echo "post-build: the release image holds /$path of the development variant; build it in a clean output" >&2
			exit 1
		fi
	done
	if grep -q /state/hot "$target/usr/libexec/neptron/launch"; then
		echo "post-build: the release launch script reads the hot folder of the development variant" >&2
		exit 1
	fi
fi
