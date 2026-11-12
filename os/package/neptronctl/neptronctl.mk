################################################################################
#
# neptronctl
#
################################################################################

NEPTRONCTL_VERSION = 0.1.0
NEPTRONCTL_SITE = $(NEPTRON_WORKSPACE)
NEPTRONCTL_SITE_METHOD = local
NEPTRONCTL_OVERRIDE_SRCDIR_RSYNC_EXCLUSIONS = $(NEPTRON_WORKSPACE_EXCLUSIONS)
# The legal-info copy of a local package ignores the exclusions above and
# would take the build output of the checkout. The source is the public
# NEPTRON repository at the commit of the image.
NEPTRONCTL_REDISTRIBUTE = NO
NEPTRONCTL_LICENSE = MIT or Apache-2.0
NEPTRONCTL_LICENSE_FILES = LICENSE-MIT LICENSE-APACHE
NEPTRONCTL_DEPENDENCIES = host-rustc host-pkgconf libdrm mesa3d neptron
NEPTRONCTL_PRE_CONFIGURE_HOOKS += NEPTRON_WORKSPACE_VENDOR

define NEPTRONCTL_BUILD_CMDS
	cd $(@D) && $(TARGET_MAKE_ENV) $(TARGET_CONFIGURE_OPTS) $(NEPTRON_WORKSPACE_CARGO_ENV) \
		cargo build --offline --locked --release \
			--package neptronctl --features kms --bin neptronctl
endef

define NEPTRONCTL_INSTALL_TARGET_CMDS
	$(INSTALL) -D -m 0755 $(@D)/target/$(RUSTC_TARGET_NAME)/release/neptronctl \
		$(TARGET_DIR)/usr/bin/neptronctl
endef

$(eval $(generic-package))
