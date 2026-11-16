################################################################################
#
# libretro-fbneo
#
################################################################################

LIBRETRO_FBNEO_VERSION = d7450cf138ef553c9ba00c86dd4fb5fb1d7646bb
# The author date of the pin, as yymmdd. It changes with the pin.
LIBRETRO_FBNEO_DATE = 261115
LIBRETRO_FBNEO_SITE = $(call github,libretro,FBNeo,$(LIBRETRO_FBNEO_VERSION))
# The FB Neo licence forbids sale, any monetary profit and requests for
# donations, and it includes the MAME licence. It has no SPDX id. The
# other entries are the libraries that the build compiles in, read from
# the dependency files of the build at this pin. A library with no
# licence file gives the source file whose header holds its notice:
# zlib.h, the MiniZip headers unzip.h and ioapi.h, file_path.c of
# libretro-common (each of its files carries the same MIT notice),
# zstd.h and stb_vorbis.c.
LIBRETRO_FBNEO_LICENSE = LicenseRef-FBNeo (non-commercial), \
	Zlib (zlib, MiniZip), MIT (libretro-common), LicenseRef-uGUI (uGUI), \
	OFL-1.1 (Source Han Sans font), BSD-3-Clause (libchdr), \
	Public Domain (LZMA SDK), BSD-3-Clause or GPL-2.0-only (zstd), \
	Unlicense or MIT-0 (dr_libs), MIT or Unlicense (stb_vorbis)
LIBRETRO_FBNEO_LICENSE_FILES = \
	src/license.txt \
	src/dep/libs/zlib/zlib.h \
	src/burner/unzip.h \
	src/burner/ioapi.h \
	src/burner/libretro/libretro-common/file/file_path.c \
	src/burner/libretro/deps/ugui/LICENSE.md \
	src/dep/libs/libchdr/LICENSE.txt \
	src/dep/libs/lzma/LICENSE \
	src/dep/libs/zstd/zstd.h \
	src/dep/libs/dr_libs/LICENSE.txt \
	src/dep/libs/stb_vorbis/stb_vorbis.c

# Buildroot selects BR2_cortex_a53 by default for aarch64, so the QEMU
# aarch64 image takes the Pi flags too. The rpi3_64 platform adds
# -march=armv8-a+crc+simd -mtune=cortex-a53. The Cyclone 68000 core is
# ARM32 code, so no aarch64 platform has it.
ifeq ($(BR2_cortex_a53),y)
LIBRETRO_FBNEO_PLATFORM = rpi3_64
else
LIBRETRO_FBNEO_PLATFORM = unix
endif

# NEPTRON reads zip sets only, so the 7z code and the LZMA SDK files of
# lib7z stay out of the binary. The CHD code stays in: at this pin
# intf/cd/cd_img.inc uses it with no condition, and the build fails with
# INCLUDE_CHD_SUPPORT=0. USE_X64_DRC stays unset: no source reads it at
# this pin, and the tree has no Xbyak.
LIBRETRO_FBNEO_MAKE_OPTS = \
	platform=$(LIBRETRO_FBNEO_PLATFORM) \
	INCLUDE_7Z_SUPPORT=0

# The makefile asks git for the version and the author date, and the
# build directory is no git checkout. The values have the upstream form.
define LIBRETRO_FBNEO_BUILD_CMDS
	$(TARGET_CONFIGURE_OPTS) $(MAKE) -C $(@D)/src/burner/libretro \
		$(LIBRETRO_FBNEO_MAKE_OPTS) \
		GIT_VERSION=$(shell echo $(LIBRETRO_FBNEO_VERSION) | cut -c 1-7) \
		GIT_DATE=$(LIBRETRO_FBNEO_DATE)
endef

define LIBRETRO_FBNEO_INSTALL_TARGET_CMDS
	$(INSTALL) -D -m 0755 $(@D)/src/burner/libretro/fbneo_libretro.so \
		$(TARGET_DIR)/usr/lib/libretro/fbneo_libretro.so
endef

$(eval $(generic-package))
