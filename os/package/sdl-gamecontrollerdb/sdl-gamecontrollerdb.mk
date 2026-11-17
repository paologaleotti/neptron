################################################################################
#
# sdl-gamecontrollerdb
#
################################################################################

SDL_GAMECONTROLLERDB_VERSION = a69b1ac650f4faab73596c2d561edc2c7636978e
SDL_GAMECONTROLLERDB_SITE = $(call github,mdqinc,SDL_GameControllerDB,$(SDL_GAMECONTROLLERDB_VERSION))
SDL_GAMECONTROLLERDB_LICENSE = Zlib
SDL_GAMECONTROLLERDB_LICENSE_FILES = LICENSE

# Not under /usr/share/neptron: the install step of the neptron package
# empties that folder, and a rebuild of that package alone would lose the
# file.
define SDL_GAMECONTROLLERDB_INSTALL_TARGET_CMDS
	$(INSTALL) -D -m 0644 $(@D)/gamecontrollerdb.txt \
		$(TARGET_DIR)/usr/share/sdl-gamecontrollerdb/gamecontrollerdb.txt
endef

$(eval $(generic-package))
