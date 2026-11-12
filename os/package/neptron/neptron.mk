################################################################################
#
# neptron
#
################################################################################

NEPTRON_VERSION = 0.1.0
NEPTRON_SITE = $(NEPTRON_WORKSPACE)
NEPTRON_SITE_METHOD = local
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

# The frontend reads the monitor presets and the skin at run time
# (Paths::image); the binary embeds no GPL data. The other folders are
# there for the tools and the person who reads the image.
define NEPTRON_INSTALL_TARGET_CMDS
	$(INSTALL) -D -m 0755 $(@D)/target/$(RUSTC_TARGET_NAME)/release/neptron \
		$(TARGET_DIR)/usr/bin/neptron
	rm -rf $(TARGET_DIR)/usr/share/neptron
	mkdir -p $(TARGET_DIR)/usr/share/neptron
	for dir in monitors cores systems skins hardware; do \
		cp -r $(@D)/data/$$dir $(TARGET_DIR)/usr/share/neptron/$$dir || exit 1; \
	done
endef

$(eval $(generic-package))
