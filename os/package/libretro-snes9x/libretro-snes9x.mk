################################################################################
#
# libretro-snes9x
#
################################################################################

LIBRETRO_SNES9X_VERSION = fae2fea08f74180759ef540ee94259213f503480
LIBRETRO_SNES9X_SITE = $(call github,libretro,snes9x,$(LIBRETRO_SNES9X_VERSION))
# The Snes9x licence allows use and distribution for non-commercial
# purposes only. It has no SPDX id.
LIBRETRO_SNES9X_LICENSE = LicenseRef-Snes9x (non-commercial)
LIBRETRO_SNES9X_LICENSE_FILES = LICENSE

# The makefile asks git for the version, and the build directory is no git
# checkout. The value keeps the quotes and the leading space of upstream.
define LIBRETRO_SNES9X_BUILD_CMDS
	$(TARGET_CONFIGURE_OPTS) $(MAKE) -C $(@D)/libretro \
		platform=unix GIT_VERSION='" $(shell echo $(LIBRETRO_SNES9X_VERSION) | cut -c 1-7)"'
endef

define LIBRETRO_SNES9X_INSTALL_TARGET_CMDS
	$(INSTALL) -D -m 0755 $(@D)/libretro/snes9x_libretro.so \
		$(TARGET_DIR)/usr/lib/libretro/snes9x_libretro.so
endef

$(eval $(generic-package))
