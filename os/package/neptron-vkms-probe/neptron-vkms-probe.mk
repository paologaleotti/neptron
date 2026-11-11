################################################################################
#
# neptron-vkms-probe
#
################################################################################

NEPTRON_VKMS_PROBE_VERSION = 1
NEPTRON_VKMS_PROBE_SITE = $(BR2_EXTERNAL_NEPTRON_PATH)/package/neptron-vkms-probe/src
NEPTRON_VKMS_PROBE_SITE_METHOD = local
NEPTRON_VKMS_PROBE_LICENSE = MIT or Apache-2.0
NEPTRON_VKMS_PROBE_DEPENDENCIES = libdrm host-pkgconf

define NEPTRON_VKMS_PROBE_BUILD_CMDS
	$(TARGET_MAKE_ENV) $(TARGET_CC) $(TARGET_CFLAGS) -std=gnu11 -Wall -Wextra -Werror \
		`$(PKG_CONFIG_HOST_BINARY) --cflags libdrm` \
		-o $(@D)/neptron-vkms-probe $(@D)/vkms-probe.c \
		$(TARGET_LDFLAGS) `$(PKG_CONFIG_HOST_BINARY) --libs libdrm`
endef

define NEPTRON_VKMS_PROBE_INSTALL_TARGET_CMDS
	$(INSTALL) -D -m 0755 $(@D)/neptron-vkms-probe $(TARGET_DIR)/usr/bin/neptron-vkms-probe
endef

$(eval $(generic-package))
