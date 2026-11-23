################################################################################
#
# neptron
#
################################################################################

NEPTRON_VERSION = 0.1.0
NEPTRON_SITE = $(NEPTRON_WORKSPACE)
NEPTRON_SITE_METHOD = local
# Copy the whole source again on each rsync; see local-source.mk.
NEPTRON_PRE_RSYNC_HOOKS += NEPTRON_LOCAL_SOURCE_CLEAR
NEPTRON_POST_RSYNC_HOOKS += NEPTRON_LOCAL_SOURCE_TOUCH
NEPTRON_OVERRIDE_SRCDIR_RSYNC_EXCLUSIONS = $(NEPTRON_WORKSPACE_EXCLUSIONS)
# The legal-info copy of a local package ignores the exclusions above and
# would take the build output of the checkout. The source is the public
# NEPTRON repository at the commit of the image.
NEPTRON_REDISTRIBUTE = NO
NEPTRON_LICENSE = MIT or Apache-2.0, GPL-2.0+ (monitor presets)
NEPTRON_LICENSE_FILES = LICENSE-MIT LICENSE-APACHE data/monitors/LICENSE
NEPTRON_DEPENDENCIES = host-rustc host-pkgconf libdrm mesa3d sdl3 alsa-lib
NEPTRON_PRE_CONFIGURE_HOOKS += NEPTRON_WORKSPACE_VENDOR

NEPTRON_FEATURES = kms$(if $(BR2_PACKAGE_NEPTRON_CONTROL),$(comma)control)

define NEPTRON_BUILD_CMDS
	cd $(@D) && $(TARGET_MAKE_ENV) $(TARGET_CONFIGURE_OPTS) $(NEPTRON_WORKSPACE_CARGO_ENV) \
		cargo build --offline --locked --release \
			--package neptron --features $(NEPTRON_FEATURES) --bin neptron
endef

# The arcade index of the frontend comes from the DAT in the source of the
# FinalBurn Neo package, so a new pin of the core makes a new index in the
# same build and the two always agree (docs/spec/arcade.md, section 6).
# This package makes it in its own install step, because that step makes
# /usr/share/neptron anew: a rebuild of this package alone (the rebuild
# list of CI, `neptron-reconfigure`) writes the index again. The generator
# is `cargo xtask arcade-index`, built for the host in this build folder,
# which holds the vendored crates, so it runs offline.
ifeq ($(BR2_PACKAGE_LIBRETRO_FBNEO),y)
NEPTRON_DEPENDENCIES += libretro-fbneo
NEPTRON_ARCADE_DAT = $(LIBRETRO_FBNEO_DIR)/dats/FinalBurn Neo (ClrMame Pro XML, Arcade only).dat
define NEPTRON_INSTALL_ARCADE_INDEX
	cd $(@D) && $(HOST_MAKE_ENV) CARGO_HOME=$(BR_CARGO_HOME) \
		cargo run --offline --locked --quiet --package xtask -- arcade-index \
			--commit $(LIBRETRO_FBNEO_VERSION) \
			"$(NEPTRON_ARCADE_DAT)" \
			$(TARGET_DIR)/usr/share/neptron/arcade/fbneo.idx
endef
endif

# The frontend reads the monitor presets, the skin and the gamepad
# mapping file at run time (Paths::image); the binary embeds no GPL data.
# The other folders are there for the tools and the person who reads the
# image.
define NEPTRON_INSTALL_TARGET_CMDS
	$(INSTALL) -D -m 0755 $(@D)/target/$(RUSTC_TARGET_NAME)/release/neptron \
		$(TARGET_DIR)/usr/bin/neptron
	rm -rf $(TARGET_DIR)/usr/share/neptron
	mkdir -p $(TARGET_DIR)/usr/share/neptron
	for dir in monitors cores systems skins hardware input; do \
		cp -r $(@D)/data/$$dir $(TARGET_DIR)/usr/share/neptron/$$dir || exit 1; \
	done
	$(NEPTRON_INSTALL_ARCADE_INDEX)
endef

$(eval $(generic-package))
