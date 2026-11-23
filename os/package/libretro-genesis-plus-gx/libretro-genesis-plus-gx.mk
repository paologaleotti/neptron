################################################################################
#
# libretro-genesis-plus-gx
#
################################################################################

LIBRETRO_GENESIS_PLUS_GX_VERSION = b9c2316441978d4015ba30e9a95d9b0e6f8ee981
LIBRETRO_GENESIS_PLUS_GX_SITE = $(call github,libretro,Genesis-Plus-GX,$(LIBRETRO_GENESIS_PLUS_GX_VERSION))
# The Genesis Plus GX licence forbids sale and commercial use. The bundled
# libraries keep their own licences, which LICENSE.txt also holds: the
# entries after the first are the parts that LICENSE.txt names for the
# core itself, at this pin (the GameCube and Wii parts are left out). The
# LGPL parts are why the written offer covers this package.
LIBRETRO_GENESIS_PLUS_GX_LICENSE = LicenseRef-GenesisPlusGX (non-commercial), \
	LGPL-2.1+ (Nuked OPN2, NTSC filter, Blip Buffer), \
	BSD-3-Clause (Tremor, libchdr, zstd), CC0-1.0 (minimp3), \
	MIT (dr_flac), Zlib (zlib), Public Domain (LZMA SDK)
LIBRETRO_GENESIS_PLUS_GX_LICENSE_FILES = LICENSE.txt

# The makefile asks git for the version, and the build directory is no git
# checkout. The value keeps the quotes and the leading space of upstream.
define LIBRETRO_GENESIS_PLUS_GX_BUILD_CMDS
	$(TARGET_CONFIGURE_OPTS) $(MAKE) -C $(@D) -f Makefile.libretro \
		platform=unix GIT_VERSION='" $(shell echo $(LIBRETRO_GENESIS_PLUS_GX_VERSION) | cut -c 1-7)"'
endef

define LIBRETRO_GENESIS_PLUS_GX_INSTALL_TARGET_CMDS
	$(INSTALL) -D -m 0755 $(@D)/genesis_plus_gx_libretro.so \
		$(TARGET_DIR)/usr/lib/libretro/genesis_plus_gx_libretro.so
endef

$(eval $(generic-package))
