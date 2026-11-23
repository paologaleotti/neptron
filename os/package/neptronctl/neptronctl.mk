################################################################################
#
# neptronctl
#
################################################################################

NEPTRONCTL_VERSION = 0.1.0
NEPTRONCTL_SITE = $(NEPTRON_WORKSPACE)
NEPTRONCTL_SITE_METHOD = local
# Copy the whole source again on each rsync; see local-source.mk.
NEPTRONCTL_PRE_RSYNC_HOOKS += NEPTRON_LOCAL_SOURCE_CLEAR
NEPTRONCTL_POST_RSYNC_HOOKS += NEPTRON_LOCAL_SOURCE_TOUCH
NEPTRONCTL_OVERRIDE_SRCDIR_RSYNC_EXCLUSIONS = $(NEPTRON_WORKSPACE_EXCLUSIONS)
# The legal-info copy of a local package ignores the exclusions above and
# would take the build output of the checkout, so legal-info does not save
# this source. The sources archive of CI holds it: the NEPTRON tree at the
# commit of the image and the vendored crates of the build (docs/ci.md).
NEPTRONCTL_REDISTRIBUTE = NO
# The binary links Switchres (GPL-2.0-or-later, vendor/switchres) through
# neptron-modeline, so it is GPL-3.0-or-later (ADR 0002). The NEPTRON
# source is MIT or Apache-2.0.
NEPTRONCTL_LICENSE = GPL-3.0+ (binary), MIT or Apache-2.0 (NEPTRON source), \
	GPL-2.0+ (Switchres)
NEPTRONCTL_LICENSE_FILES = LICENSE-GPL-3.0 LICENSE-MIT LICENSE-APACHE \
	vendor/switchres/LICENSE
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
	$(call NEPTRON_WORKSPACE_NOTICES,neptronctl,--package neptronctl --features kms)
endef

$(eval $(generic-package))
