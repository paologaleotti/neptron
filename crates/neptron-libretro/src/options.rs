//! Core options, as `SET_VARIABLES` declares them (the version 0 API).

use std::ffi::{CStr, CString};

/// The most options that the host keeps for one core.
pub const MAX_OPTIONS: usize = 1024;

/// One option of the core, with its current value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreOption {
    key: String,
    description: String,
    values: Vec<CString>,
    current: usize,
}

/// Why an option value could not be set.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OptionError {
    /// The core declared no option with this key.
    #[error("the core has no option {key}")]
    UnknownKey {
        /// The key given.
        key: String,
    },
    /// The value is not in the list of the option.
    #[error("option {key} has no value {value}")]
    UnknownValue {
        /// The key.
        key: String,
        /// The value given.
        value: String,
    },
}

impl CoreOption {
    /// The key, as the core asks for it.
    pub fn key(&self) -> &str {
        &self.key
    }

    /// The text that describes the option.
    pub fn description(&self) -> &str {
        &self.description
    }

    /// The values, the default first.
    pub fn values(&self) -> impl Iterator<Item = &str> {
        self.values.iter().filter_map(|v| v.to_str().ok())
    }

    /// The current value.
    pub fn value(&self) -> &str {
        self.current_c().to_str().unwrap_or_default()
    }

    fn current_c(&self) -> &CStr {
        self.values.get(self.current).map_or(c"", CString::as_c_str)
    }

    /// Parses `"Description; default|value2|value3"`.
    fn parse(key: &[u8], declaration: &[u8]) -> Result<Self, &'static str> {
        let key = std::str::from_utf8(key).map_err(|_| "the key is not UTF-8")?;
        if key.is_empty() {
            return Err("the key is empty");
        }
        let declaration =
            std::str::from_utf8(declaration).map_err(|_| "the declaration is not UTF-8")?;
        let (description, list) = declaration
            .split_once("; ")
            .ok_or("the declaration has no \"; \"")?;
        let values = list
            .split('|')
            .map(|v| {
                if v.is_empty() {
                    Err("a value is empty")
                } else {
                    // A value from a C string holds no NUL.
                    CString::new(v).map_err(|_| "a value holds a NUL")
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            key: key.to_owned(),
            description: description.to_owned(),
            values,
            current: 0,
        })
    }
}

/// The declared options and the update flag of `GET_VARIABLE_UPDATE`.
#[derive(Debug, Default)]
pub(crate) struct OptionStore {
    options: Vec<CoreOption>,
    updated: bool,
}

impl OptionStore {
    pub(crate) fn options(&self) -> &[CoreOption] {
        &self.options
    }

    /// Replaces the declared options. A key that was declared before keeps
    /// its value when the new list still has it. Returns the entries that
    /// were skipped, with the reason.
    pub(crate) fn declare<'a>(
        &mut self,
        entries: &[(&'a [u8], &'a [u8])],
    ) -> Vec<(&'a [u8], &'static str)> {
        let mut skipped = Vec::new();
        let mut options = Vec::with_capacity(entries.len().min(MAX_OPTIONS));
        for &(key, declaration) in entries.iter().take(MAX_OPTIONS) {
            match CoreOption::parse(key, declaration) {
                Ok(mut option) => {
                    if let Some(old) = self.find(&option.key) {
                        let old_value = old.current_c();
                        if let Some(i) =
                            option.values.iter().position(|v| v.as_c_str() == old_value)
                        {
                            option.current = i;
                        }
                    }
                    options.push(option);
                }
                Err(reason) => skipped.push((key, reason)),
            }
        }
        for &(key, _) in entries.iter().skip(MAX_OPTIONS) {
            skipped.push((key, "the core declares more options than the host keeps"));
        }
        self.options = options;
        skipped
    }

    /// The current value of `key` for `GET_VARIABLE`. The string lives
    /// until the option changes or the options are declared again.
    pub(crate) fn value(&self, key: &[u8]) -> Option<&CStr> {
        let key = std::str::from_utf8(key).ok()?;
        self.find(key).map(CoreOption::current_c)
    }

    pub(crate) fn set(&mut self, key: &str, value: &str) -> Result<(), OptionError> {
        let option = self
            .options
            .iter_mut()
            .find(|o| o.key == key)
            .ok_or_else(|| OptionError::UnknownKey {
                key: key.to_owned(),
            })?;
        let index = option
            .values
            .iter()
            .position(|v| v.to_str() == Ok(value))
            .ok_or_else(|| OptionError::UnknownValue {
                key: key.to_owned(),
                value: value.to_owned(),
            })?;
        if option.current != index {
            option.current = index;
            self.updated = true;
        }
        Ok(())
    }

    /// True once after any value changed.
    pub(crate) fn take_update(&mut self) -> bool {
        std::mem::take(&mut self.updated)
    }

    fn find(&self, key: &str) -> Option<&CoreOption> {
        self.options.iter().find(|o| o.key == key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> OptionStore {
        let mut s = OptionStore::default();
        let skipped = s.declare(&[
            (b"mode", b"Test mode; alpha|beta|gamma"),
            (b"level", b"Test level; 1|2|3"),
        ]);
        assert!(skipped.is_empty());
        s
    }

    #[test]
    fn declarations_parse_with_the_default_first() {
        let s = store();
        let o = &s.options()[0];
        assert_eq!(o.key(), "mode");
        assert_eq!(o.description(), "Test mode");
        assert_eq!(o.values().collect::<Vec<_>>(), ["alpha", "beta", "gamma"]);
        assert_eq!(o.value(), "alpha");
        assert_eq!(s.value(b"level"), Some(c"1"));
        assert_eq!(s.value(b"nothing"), None);
        assert_eq!(s.value(b"\xFF"), None);
    }

    #[test]
    fn set_checks_key_and_value_and_flags_the_update() {
        let mut s = store();
        assert!(!s.take_update());
        s.set("mode", "gamma").unwrap();
        assert_eq!(s.value(b"mode"), Some(c"gamma"));
        assert!(s.take_update());
        assert!(!s.take_update(), "the flag is read once");
        s.set("mode", "gamma").unwrap();
        assert!(!s.take_update(), "no change, no update");
        assert_eq!(
            s.set("speed", "1"),
            Err(OptionError::UnknownKey {
                key: "speed".to_owned()
            })
        );
        assert_eq!(
            s.set("mode", "delta"),
            Err(OptionError::UnknownValue {
                key: "mode".to_owned(),
                value: "delta".to_owned()
            })
        );
    }

    #[test]
    fn bad_declarations_are_skipped() {
        let mut s = OptionStore::default();
        let skipped = s.declare(&[
            (b"", b"Empty key; a|b"),
            (b"nosep", b"No separator|a|b"),
            (b"empty", b"Empty value; a||b"),
            (b"utf", b"Bad \xFF; a"),
            (b"good", b"Good; on|off"),
        ]);
        let reasons: Vec<_> = skipped.iter().map(|(_, r)| *r).collect();
        assert_eq!(
            reasons,
            [
                "the key is empty",
                "the declaration has no \"; \"",
                "a value is empty",
                "the declaration is not UTF-8"
            ]
        );
        assert_eq!(s.options().len(), 1);
    }

    #[test]
    fn a_new_declaration_keeps_values_that_still_exist() {
        let mut s = store();
        s.set("mode", "beta").unwrap();
        s.set("level", "3").unwrap();
        s.declare(&[(b"mode", b"Mode; beta|alpha"), (b"level", b"Level; 1|2")]);
        assert_eq!(s.value(b"mode"), Some(c"beta"));
        assert_eq!(s.value(b"level"), Some(c"1"), "3 is gone: the default");
    }

    #[test]
    fn options_beyond_the_limit_are_skipped() {
        let keys: Vec<String> = (0..MAX_OPTIONS + 2).map(|i| format!("k{i}")).collect();
        let entries: Vec<(&[u8], &[u8])> =
            keys.iter().map(|k| (k.as_bytes(), &b"K; a"[..])).collect();
        let mut s = OptionStore::default();
        assert_eq!(s.declare(&entries).len(), 2);
        assert_eq!(s.options().len(), MAX_OPTIONS);
    }
}
