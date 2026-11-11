################################################################################
#
# libretro-fceumm
#
################################################################################

LIBRETRO_FCEUMM_VERSION = e0f3086a8104f8fbbe49a2615b10401504a9533f
LIBRETRO_FCEUMM_SITE = $(call github,libretro,libretro-fceumm,$(LIBRETRO_FCEUMM_VERSION))
LIBRETRO_FCEUMM_LICENSE = GPL-2.0+
LIBRETRO_FCEUMM_LICENSE_FILES = Copying

# The makefile asks git for the version, and the build directory is no git
# checkout. The value keeps the quotes and the leading space of upstream.
define LIBRETRO_FCEUMM_BUILD_CMDS
	$(TARGET_CONFIGURE_OPTS) $(MAKE) -C $(@D) -f Makefile.libretro \
		platform=unix GIT_VERSION='" $(shell echo $(LIBRETRO_FCEUMM_VERSION) | cut -c 1-7)"'
endef

define LIBRETRO_FCEUMM_INSTALL_TARGET_CMDS
	$(INSTALL) -D -m 0755 $(@D)/fceumm_libretro.so \
		$(TARGET_DIR)/usr/lib/libretro/fceumm_libretro.so
endef

$(eval $(generic-package))
