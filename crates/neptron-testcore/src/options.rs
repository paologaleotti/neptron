//! The core options of the test core.

#![forbid(unsafe_code)]

/// One core option, as `SET_VARIABLES` declares it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoreOption {
    /// The key.
    pub key: &'static str,
    /// The description shown to the user.
    pub description: &'static str,
    /// The values. The first one is the default.
    pub values: &'static [&'static str],
}

/// The core options that the core declares in `retro_set_environment`.
pub const CORE_OPTIONS: [CoreOption; 2] = [
    CoreOption {
        key: "neptron_testcore_mode",
        description: "Test mode",
        values: &["alpha", "beta", "gamma"],
    },
    CoreOption {
        key: "neptron_testcore_level",
        description: "Test level",
        values: &["1", "2", "3"],
    },
];

/// The option that the core declares only in `retro_load_game`, with
/// [`CORE_OPTIONS`], when the script has `options load`: a core that learns
/// options from its game, as FinalBurn Neo learns the DIP switches.
pub const LOAD_OPTION: CoreOption = CoreOption {
    key: "neptron_testcore_dip",
    description: "Test DIP",
    values: &["off", "on"],
};

/// The option with index `option`: the [`CORE_OPTIONS`], then
/// [`LOAD_OPTION`].
pub fn option(option: usize) -> Option<&'static CoreOption> {
    CORE_OPTIONS
        .get(option)
        .or_else(|| (option == CORE_OPTIONS.len()).then_some(&LOAD_OPTION))
}

/// The save RAM byte when `GET_VARIABLE` returns false or no value.
pub const VALUE_NOT_SET: u8 = 0;
/// The save RAM byte when `GET_VARIABLE` returns a value that the option
/// does not list.
pub const VALUE_NOT_LISTED: u8 = 0xFF;

/// The index of the option with this key, as [`option`] numbers them.
pub fn option_index(key: &str) -> Option<usize> {
    (0..=CORE_OPTIONS.len()).find(|&i| option(i).is_some_and(|o| o.key == key))
}

/// The save RAM byte that records the answer to `GET_VARIABLE` for option
/// `option`: [`VALUE_NOT_SET`] for no value, the position of the value in the
/// list plus 1, or [`VALUE_NOT_LISTED`].
pub fn value_code(option: usize, value: Option<&str>) -> u8 {
    let Some(value) = value else {
        return VALUE_NOT_SET;
    };
    self::option(option)
        .and_then(|o| o.values.iter().position(|v| *v == value))
        .and_then(|i| u8::try_from(i + 1).ok())
        .unwrap_or(VALUE_NOT_LISTED)
}

/// The `value` string of the `retro_variable` that declares an option:
/// `"<description>; <value>|<value>|..."`.
pub fn declaration(option: &CoreOption) -> String {
    format!("{}; {}", option.description, option.values.join("|"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_found() {
        assert_eq!(option_index("neptron_testcore_mode"), Some(0));
        assert_eq!(option_index("neptron_testcore_level"), Some(1));
        assert_eq!(option_index("neptron_testcore_dip"), Some(2));
        assert_eq!(option_index("other"), None);
        assert_eq!(option(2), Some(&LOAD_OPTION));
        assert_eq!(option(3), None);
    }

    #[test]
    fn answers_become_codes() {
        assert_eq!(value_code(0, None), VALUE_NOT_SET);
        assert_eq!(value_code(0, Some("alpha")), 1);
        assert_eq!(value_code(0, Some("gamma")), 3);
        assert_eq!(value_code(1, Some("2")), 2);
        assert_eq!(value_code(0, Some("2")), VALUE_NOT_LISTED);
        assert_eq!(value_code(2, Some("on")), 2);
        assert_eq!(value_code(9, Some("alpha")), VALUE_NOT_LISTED);
    }

    #[test]
    fn declarations_follow_the_libretro_form() {
        assert_eq!(declaration(&CORE_OPTIONS[0]), "Test mode; alpha|beta|gamma");
        assert_eq!(declaration(&CORE_OPTIONS[1]), "Test level; 1|2|3");
        assert_eq!(declaration(&LOAD_OPTION), "Test DIP; off|on");
    }
}
