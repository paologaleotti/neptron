################################################################################
#
# The copy of the source of the NEPTRON packages that build from this tree
#
################################################################################

# Buildroot copies the source of a local package into its build folder with
# `rsync -au`. The copy keeps the file times of the source, and it skips a
# file whose copy in the build folder is newer. A file of another checkout,
# or a file that changed before the last build, can then keep its old copy,
# or get a time older than the build output. Cargo and make compare file
# times, so they take the old output as fresh, and the image keeps old code
# of a changed crate.
#
# The two hooks below run on each rsync of the package, which a
# `<pkg>-rebuild` or `<pkg>-reconfigure` does again. The first removes the
# old copy of each top-level entry of the source, so that rsync copies every
# file again. The second gives every copied file the current time, so that
# Cargo and make build all of the source again. The entries in
# NEPTRON_LOCAL_SOURCE_KEEP stay: the Cargo build output, the vendored
# crates and the Cargo settings with the vendor lines. So the dependencies
# do not compile again, and only the crates of the workspace do.
#
# SRCDIR is the source folder that the rsync step of Buildroot sets for its
# hooks.

NEPTRON_LOCAL_SOURCE_KEEP = target VENDOR .cargo

define NEPTRON_LOCAL_SOURCE_CLEAR
	cd $(call qstrip,$(SRCDIR)) && for name in * .[!.]*; do \
		case " $(NEPTRON_LOCAL_SOURCE_KEEP) " in *" $$name "*) continue ;; esac; \
		rm -rf "$(@D)/$$name" || exit 1; \
	done
endef

define NEPTRON_LOCAL_SOURCE_TOUCH
	cd $(call qstrip,$(SRCDIR)) && for name in * .[!.]*; do \
		case " $(NEPTRON_LOCAL_SOURCE_KEEP) " in *" $$name "*) continue ;; esac; \
		if [ -e "$(@D)/$$name" ]; then \
			find "$(@D)/$$name" -exec touch -h {} + || exit 1; \
		fi; \
	done
endef
