################################################################################
#
# neptron-launch
#
################################################################################

NEPTRON_LAUNCH_VERSION = 0.1.0
NEPTRON_LAUNCH_SITE = $(NEPTRON_WORKSPACE)
NEPTRON_LAUNCH_SITE_METHOD = local
NEPTRON_LAUNCH_OVERRIDE_SRCDIR_RSYNC_EXCLUSIONS = $(NEPTRON_WORKSPACE_EXCLUSIONS)
# The legal-info copy of a local package ignores the exclusions above and
# would take the build output of the checkout. The source is the public
# NEPTRON repository at the commit of the image.
NEPTRON_LAUNCH_REDISTRIBUTE = NO
NEPTRON_LAUNCH_LICENSE = MIT or Apache-2.0
NEPTRON_LAUNCH_LICENSE_FILES = LICENSE-MIT LICENSE-APACHE
NEPTRON_LAUNCH_DEPENDENCIES = host-rustc
NEPTRON_LAUNCH_PRE_CONFIGURE_HOOKS += NEPTRON_WORKSPACE_VENDOR

define NEPTRON_LAUNCH_BUILD_CMDS
	cd $(@D) && $(TARGET_MAKE_ENV) $(TARGET_CONFIGURE_OPTS) $(NEPTRON_WORKSPACE_CARGO_ENV) \
		cargo build --offline --locked --release \
			--package neptron-launch --bin neptron-launch
endef

define NEPTRON_LAUNCH_INSTALL_TARGET_CMDS
	$(INSTALL) -D -m 0755 $(@D)/target/$(RUSTC_TARGET_NAME)/release/neptron-launch \
		$(TARGET_DIR)/usr/bin/neptron-launch
endef

$(eval $(generic-package))
