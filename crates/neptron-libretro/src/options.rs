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

    fn index_of(&self, value: &str) -> Option<usize> {
        self.values.iter().position(|v| v.to_str() == Ok(value))
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

/// The declared options, the held values and the update flag of
/// `GET_VARIABLE_UPDATE`.
#[derive(Debug, Default)]
pub(crate) struct OptionStore {
    options: Vec<CoreOption>,
    /// The initial values of the frontend, one per key.
    held: Vec<Held>,
    updated: bool,
}

/// One initial value of the frontend.
#[derive(Debug)]
struct Held {
    key: String,
    value: String,
    /// The value was applied once, or `set` changed the key. The key then
    /// keeps its current value at a later declaration.
    applied: bool,
}

impl OptionStore {
    pub(crate) fn options(&self) -> &[CoreOption] {
        &self.options
    }

    /// Keeps `values` as the held values, in place of the ones before, at
    /// most [`MAX_OPTIONS`] keys; a later pair for the same key wins. Each
    /// declared key that lists its held value gets it at once, as `set`
    /// gives it. Returns true when pairs were cut.
    pub(crate) fn hold(&mut self, values: &[(String, String)]) -> bool {
        let mut held: Vec<Held> = Vec::with_capacity(values.len().min(MAX_OPTIONS));
        let mut cut = false;
        for (key, value) in values {
            if let Some(slot) = held.iter_mut().find(|h| h.key == *key) {
                slot.value.clone_from(value);
            } else if held.len() < MAX_OPTIONS {
                held.push(Held {
                    key: key.clone(),
                    value: value.clone(),
                    applied: false,
                });
            } else {
                cut = true;
            }
        }
        for entry in &mut held {
            if let Some(option) = self.options.iter_mut().find(|o| o.key == entry.key)
                && let Some(index) = option.index_of(&entry.value)
            {
                if option.current != index {
                    option.current = index;
                    self.updated = true;
                }
                entry.applied = true;
            }
        }
        self.held = held;
        cut
    }

    /// The held values that never applied: a key that the core has not
    /// declared, or a value that the list of its key does not have.
    pub(crate) fn held_report(&self) -> Vec<OptionError> {
        self.held
            .iter()
            .filter(|h| !h.applied)
            .map(|h| match self.find(&h.key) {
                None => OptionError::UnknownKey { key: h.key.clone() },
                Some(_) => OptionError::UnknownValue {
                    key: h.key.clone(),
                    value: h.value.clone(),
                },
            })
            .collect()
    }

    /// Replaces the declared options. A held value that never applied is
    /// applied when the new list has it, also for a key that was declared
    /// before; that sets no update flag, because the core reads its values
    /// after it declares them. Any other key that was declared before keeps
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
                    let pending = self
                        .held
                        .iter_mut()
                        .filter(|h| !h.applied && h.key == option.key)
                        .find_map(|h| option.index_of(&h.value).map(|i| (h, i)));
                    let start = match pending {
                        Some((held, index)) => {
                            held.applied = true;
                            Some(index)
                        }
                        None => self.find(&option.key).and_then(|old| {
                            let old_value = old.current_c();
                            option.values.iter().position(|v| v.as_c_str() == old_value)
                        }),
                    };
                    if let Some(i) = start {
                        option.current = i;
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
            .index_of(value)
            .ok_or_else(|| OptionError::UnknownValue {
                key: key.to_owned(),
                value: value.to_owned(),
            })?;
        if option.current != index {
            option.current = index;
            self.updated = true;
        }
        if let Some(held) = self.held.iter_mut().find(|h| h.key == key) {
            held.applied = true;
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

    fn pairs(list: &[(&str, &str)]) -> Vec<(String, String)> {
        list.iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }

    #[test]
    fn held_values_apply_at_once_to_declared_keys() {
        let mut s = store();
        assert!(!s.hold(&pairs(&[("mode", "gamma"), ("level", "9")])));
        assert_eq!(s.value(b"mode"), Some(c"gamma"));
        assert_eq!(s.value(b"level"), Some(c"1"), "9 is not listed");
        assert!(
            s.take_update(),
            "a change at once is a change as `set` makes"
        );
        assert_eq!(
            s.held_report(),
            [OptionError::UnknownValue {
                key: "level".to_owned(),
                value: "9".to_owned()
            }]
        );
    }

    #[test]
    fn held_values_apply_to_keys_declared_later() {
        let mut s = OptionStore::default();
        s.hold(&pairs(&[
            ("dip", "on"),
            ("mode", "beta"),
            ("mode", "gamma"),
            ("level", "9"),
            ("speed", "2"),
        ]));
        assert!(s.options().is_empty());
        assert_eq!(s.held_report().len(), 4, "nothing is declared yet");
        s.declare(&[
            (b"mode", b"Mode; alpha|beta|gamma"),
            (b"level", b"Level; 1|2|3"),
            (b"dip", b"DIP; off|on"),
        ]);
        assert_eq!(s.value(b"mode"), Some(c"gamma"), "the later pair wins");
        assert_eq!(s.value(b"dip"), Some(c"on"));
        assert_eq!(
            s.value(b"level"),
            Some(c"1"),
            "9 is not listed: the default"
        );
        assert!(!s.take_update(), "a declaration sets no update flag");
        assert_eq!(
            s.held_report(),
            [
                OptionError::UnknownValue {
                    key: "level".to_owned(),
                    value: "9".to_owned()
                },
                OptionError::UnknownKey {
                    key: "speed".to_owned()
                },
            ]
        );
    }

    /// A value from the quick menu is never undone when the core declares
    /// its options again.
    #[test]
    fn a_second_declaration_keeps_the_current_value_over_the_held_one() {
        let mut s = OptionStore::default();
        s.hold(&pairs(&[("mode", "beta")]));
        s.declare(&[(b"mode", b"Mode; alpha|beta|gamma")]);
        assert_eq!(s.value(b"mode"), Some(c"beta"));
        s.set("mode", "alpha").unwrap();
        s.declare(&[
            (b"mode", b"Mode; alpha|beta|gamma"),
            (b"dip", b"DIP; off|on"),
        ]);
        assert_eq!(s.value(b"mode"), Some(c"alpha"));
        assert!(s.held_report().is_empty());
    }

    /// A key declared at open whose list lacks the held value, declared
    /// again at load with a list that has it: the core reads the held
    /// value, and nothing is reported.
    #[test]
    fn a_held_value_applies_when_a_later_declaration_offers_it() {
        let mut s = OptionStore::default();
        s.declare(&[(b"k", b"K; a|b")]);
        s.hold(&pairs(&[("k", "c")]));
        assert_eq!(s.value(b"k"), Some(c"a"));
        assert_eq!(s.held_report().len(), 1);
        s.declare(&[(b"k", b"K; a|b|c")]);
        assert_eq!(s.value(b"k"), Some(c"c"));
        assert!(s.held_report().is_empty());
        assert!(!s.take_update(), "a declaration sets no update flag");
    }

    /// A key set by `set` keeps its value, also when the held value never
    /// applied and a later list offers it.
    #[test]
    fn a_set_value_wins_over_a_held_value_that_never_applied() {
        let mut s = OptionStore::default();
        s.declare(&[(b"k", b"K; a|b")]);
        s.hold(&pairs(&[("k", "c")]));
        s.set("k", "b").unwrap();
        s.declare(&[(b"k", b"K; a|b|c")]);
        assert_eq!(s.value(b"k"), Some(c"b"));
        assert!(s.held_report().is_empty(), "the key was set");
    }

    #[test]
    fn held_values_beyond_the_limit_are_cut() {
        let keys: Vec<(String, String)> = (0..MAX_OPTIONS + 3)
            .map(|i| (format!("k{i}"), "a".to_owned()))
            .collect();
        let mut s = OptionStore::default();
        assert!(s.hold(&keys));
        assert_eq!(s.held_report().len(), MAX_OPTIONS);
        let mut again = keys.clone();
        again.truncate(MAX_OPTIONS);
        again.push(("k0".to_owned(), "b".to_owned()));
        assert!(!s.hold(&again), "a repeated key takes no room");
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
