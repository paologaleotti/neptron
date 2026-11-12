# The Rust programs of the workspace are built from the checkout that holds
# this tree. The checkout is copied without its build output, then `cargo
# vendor` fetches the crates that Cargo.lock names, each checked against
# the SHA-256 in Cargo.lock, so the build itself runs offline.
NEPTRON_WORKSPACE = $(BR2_EXTERNAL_NEPTRON_PATH)/..
NEPTRON_WORKSPACE_EXCLUSIONS = \
	--exclude=/target --exclude=/.worktrees --exclude=/.git --exclude=/VENDOR

# Runs as a pre-configure hook of a package of the workspace, after
# host-rustc is built. The vendor settings follow the settings of the
# checkout's own .cargo/config.toml.
define NEPTRON_WORKSPACE_VENDOR
	mkdir -p $(BR_CARGO_HOME) $(@D)/.cargo
	cp $(NEPTRON_WORKSPACE)/.cargo/config.toml $(@D)/.cargo/config.toml
	cd $(@D) && $(HOST_MAKE_ENV) CARGO_HOME=$(BR_CARGO_HOME) \
		flock $(BR_CARGO_HOME)/.br-lock \
		cargo vendor --locked VENDOR >> $(@D)/.cargo/config.toml
endef

# The workspace release profile (fat LTO, one codegen unit, abort on panic)
# replaces the profile values that Buildroot sets for every Cargo package.
# The linker is the Buildroot compiler behind link-filter.sh; see there.
NEPTRON_WORKSPACE_CARGO_ENV = \
	$(PKG_CARGO_ENV) \
	CARGO_PROFILE_RELEASE_OPT_LEVEL=3 \
	CARGO_PROFILE_RELEASE_LTO=fat \
	CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1 \
	PKG_CONFIG_ALLOW_CROSS=1 \
	NEPTRON_LINK_CC="$(TARGET_CC)" \
	CARGO_TARGET_$(call UPPERCASE,$(RUSTC_TARGET_NAME))_LINKER=$(BR2_EXTERNAL_NEPTRON_PATH)/package/neptron/link-filter.sh

include $(sort $(wildcard $(BR2_EXTERNAL_NEPTRON_PATH)/package/*/*.mk))
