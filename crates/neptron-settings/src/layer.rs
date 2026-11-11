use std::collections::BTreeMap;

use crate::schema::{Invalid, Level, Schema, Value};

/// The valid values of one settings file. Every value in a layer passed the schema check for
/// its key and its level.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layer {
    level: Level,
    values: BTreeMap<&'static str, Value>,
}

/// Why a value cannot go into a layer.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SetError {
    /// The schema has no such key.
    #[error("unknown key \"{key}\"")]
    UnknownKey {
        /// The key.
        key: String,
    },
    /// The key may not be set at this level.
    #[error("key \"{key}\" cannot be set at the {level} level")]
    LevelNotAllowed {
        /// The key.
        key: &'static str,
        /// The level of the layer.
        level: Level,
    },
    /// The value has the wrong type or is out of range.
    #[error("key \"{key}\": {reason}")]
    Invalid {
        /// The key.
        key: &'static str,
        /// Why the value is invalid.
        reason: Invalid,
    },
}

impl Layer {
    /// An empty layer for `level`.
    pub fn new(level: Level) -> Self {
        Layer {
            level,
            values: BTreeMap::new(),
        }
    }

    /// The level of this layer.
    pub fn level(&self) -> Level {
        self.level
    }

    /// The value of `key`, when this layer sets it.
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.values.get(key)
    }

    /// Sets `key` after a check against `schema`, and returns the value it replaces.
    pub fn set(
        &mut self,
        schema: &Schema,
        key: &str,
        value: Value,
    ) -> Result<Option<Value>, SetError> {
        let entry = schema.get(key).ok_or_else(|| SetError::UnknownKey {
            key: key.to_owned(),
        })?;
        if !entry.levels.allows(self.level) {
            return Err(SetError::LevelNotAllowed {
                key: entry.key,
                level: self.level,
            });
        }
        entry
            .kind
            .check(&value)
            .map_err(|reason| SetError::Invalid {
                key: entry.key,
                reason,
            })?;
        Ok(self.values.insert(entry.key, value))
    }

    /// Removes `key`, so the value is inherited from the level below. Returns the old value.
    pub fn remove(&mut self, key: &str) -> Option<Value> {
        self.values.remove(key)
    }

    /// Every key and value, in key order.
    pub fn iter(&self) -> impl Iterator<Item = (&'static str, &Value)> {
        self.values.iter().map(|(k, v)| (*k, v))
    }

    /// The number of keys set.
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// True when the layer sets no key.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DISPLAY_OUTPUT, DISPLAY_STYLE};

    fn text(v: &str) -> Value {
        Value::Text(v.to_owned())
    }

    #[test]
    fn set_get_and_remove_round_trip() {
        let schema = Schema::builtin();
        let mut layer = Layer::new(Level::Game);
        assert!(layer.is_empty());
        assert_eq!(layer.set(&schema, DISPLAY_STYLE, text("2x")), Ok(None));
        assert_eq!(
            layer.set(&schema, DISPLAY_STYLE, text("3x")),
            Ok(Some(text("2x")))
        );
        assert_eq!(layer.get(DISPLAY_STYLE), Some(&text("3x")));
        assert_eq!(layer.len(), 1);
        assert_eq!(
            layer.iter().collect::<Vec<_>>(),
            [(DISPLAY_STYLE, &text("3x"))]
        );
        assert_eq!(layer.remove(DISPLAY_STYLE), Some(text("3x")));
        assert_eq!(layer.remove(DISPLAY_STYLE), None);
        assert!(layer.get(DISPLAY_STYLE).is_none());
    }

    #[test]
    fn set_rejects_unknown_keys_wrong_levels_and_bad_values() {
        let schema = Schema::builtin();
        let mut layer = Layer::new(Level::Game);
        assert_eq!(
            layer.set(&schema, "display.nope", text("x")),
            Err(SetError::UnknownKey {
                key: "display.nope".into()
            })
        );
        assert_eq!(
            layer.set(&schema, DISPLAY_OUTPUT, text("VGA-1")),
            Err(SetError::LevelNotAllowed {
                key: DISPLAY_OUTPUT,
                level: Level::Game
            })
        );
        assert!(matches!(
            layer.set(&schema, DISPLAY_STYLE, text("5x")),
            Err(SetError::Invalid {
                reason: Invalid::NotAChoice { .. },
                ..
            })
        ));
        assert!(matches!(
            layer.set(&schema, DISPLAY_STYLE, Value::Int(2)),
            Err(SetError::Invalid {
                reason: Invalid::WrongType { .. },
                ..
            })
        ));
        assert!(layer.is_empty());

        let mut global = Layer::new(Level::Global);
        assert_eq!(global.set(&schema, DISPLAY_OUTPUT, text("VGA-1")), Ok(None));
    }
}
