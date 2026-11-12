################################################################################
#
# neptron-capture
#
################################################################################

NEPTRON_CAPTURE_VERSION = 1
NEPTRON_CAPTURE_SITE = $(BR2_EXTERNAL_NEPTRON_PATH)/package/neptron-capture/src
NEPTRON_CAPTURE_SITE_METHOD = local
NEPTRON_CAPTURE_LICENSE = MIT or Apache-2.0
NEPTRON_CAPTURE_DEPENDENCIES = libdrm host-pkgconf

define NEPTRON_CAPTURE_BUILD_CMDS
	$(TARGET_MAKE_ENV) $(TARGET_CC) $(TARGET_CFLAGS) -std=gnu11 -Wall -Wextra -Werror \
		`$(PKG_CONFIG_HOST_BINARY) --cflags libdrm` \
		-o $(@D)/neptron-capture $(@D)/capture.c \
		$(TARGET_LDFLAGS) `$(PKG_CONFIG_HOST_BINARY) --libs libdrm`
endef

define NEPTRON_CAPTURE_INSTALL_TARGET_CMDS
	$(INSTALL) -D -m 0755 $(@D)/neptron-capture $(TARGET_DIR)/usr/bin/neptron-capture
endef

$(eval $(generic-package))
