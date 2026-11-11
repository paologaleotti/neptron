use crate::layer::Layer;
use crate::schema::{Entry, Level, Schema, Source, Value};

/// The layers that apply in one context: none, global, global and system, or all three.
#[derive(Debug, Clone, Copy, Default)]
pub struct Layers<'a> {
    game: Option<&'a Layer>,
    system: Option<&'a Layer>,
    global: Option<&'a Layer>,
}

/// Why a layer does not fit the slot it is given for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("a {found} layer was given as the {slot} layer")]
pub struct LayerMismatch {
    /// The slot.
    pub slot: Level,
    /// The level of the layer given.
    pub found: Level,
}

/// One resolved setting: the value that applies and the level it comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved<'s> {
    /// The schema entry.
    pub entry: &'s Entry,
    /// The value that applies.
    pub value: Value,
    /// Where the value comes from.
    pub source: Source,
}

impl<'a> Layers<'a> {
    /// Layers for a context. Each layer must have the level of its slot. A missing layer is
    /// the same as an empty one.
    pub fn try_new(
        global: Option<&'a Layer>,
        system: Option<&'a Layer>,
        game: Option<&'a Layer>,
    ) -> Result<Self, LayerMismatch> {
        for (slot, layer) in [
            (Level::Global, global),
            (Level::System, system),
            (Level::Game, game),
        ] {
            if let Some(layer) = layer
                && layer.level() != slot
            {
                return Err(LayerMismatch {
                    slot,
                    found: layer.level(),
                });
            }
        }
        Ok(Layers {
            game,
            system,
            global,
        })
    }

    /// The layer for `level`.
    pub fn get(&self, level: Level) -> Option<&'a Layer> {
        match level {
            Level::Game => self.game,
            Level::System => self.system,
            Level::Global => self.global,
        }
    }
}

/// Resolves every key of `schema`, in menu order.
pub fn resolve<'s>(schema: &'s Schema, layers: &Layers<'_>) -> Vec<Resolved<'s>> {
    schema
        .entries()
        .iter()
        .map(|entry| resolve_entry(entry, layers))
        .collect()
}

/// Resolves one key. Returns `None` when the schema has no such key.
pub fn resolve_key<'s>(schema: &'s Schema, layers: &Layers<'_>, key: &str) -> Option<Resolved<'s>> {
    schema.get(key).map(|entry| resolve_entry(entry, layers))
}

/// The first level in resolution order whose layer holds a value that the entry allows at
/// that level, or else the default. The check repeats here so that a layer checked against
/// a different schema still cannot give an invalid value.
fn resolve_entry<'s>(entry: &'s Entry, layers: &Layers<'_>) -> Resolved<'s> {
    for level in Level::ORDER {
        if !entry.levels.allows(level) {
            continue;
        }
        let value = layers.get(level).and_then(|layer| layer.get(entry.key));
        if let Some(value) = value
            && entry.kind.check(value).is_ok()
        {
            return Resolved {
                entry,
                value: value.clone(),
                source: level.into(),
            };
        }
    }
    Resolved {
        entry,
        value: entry.kind.default_value(),
        source: Source::Default,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{Kind, Levels, Safety};

    const KEY: &str = "t.k";
    const DEFAULT: i64 = 5;
    const OUTSIDE_NARROW: i64 = 50;

    fn int_schema(min: i64, max: i64, levels: Levels) -> Schema {
        Schema::from_entries(vec![Entry {
            key: KEY,
            label: "Test",
            kind: Kind::Int {
                min,
                max,
                default: DEFAULT,
            },
            levels,
            live: true,
            safety: Safety::None,
        }])
        .unwrap()
    }

    /// What one slot holds in a case.
    #[derive(Clone, Copy, Debug)]
    enum Slot {
        NoLayer,
        Empty,
        Valid,
        Invalid,
    }

    const SLOTS: [Slot; 4] = [Slot::NoLayer, Slot::Empty, Slot::Valid, Slot::Invalid];

    fn valid_value(level: Level) -> i64 {
        match level {
            Level::Game => 1,
            Level::System => 2,
            Level::Global => 3,
        }
    }

    fn make_layer(wide: &Schema, level: Level, slot: Slot) -> Option<Layer> {
        let mut layer = Layer::new(level);
        let value = match slot {
            Slot::NoLayer => return None,
            Slot::Empty => return Some(layer),
            Slot::Valid => valid_value(level),
            Slot::Invalid => OUTSIDE_NARROW,
        };
        layer.set(wide, KEY, Value::Int(value)).unwrap();
        Some(layer)
    }

    /// Every combination of slot contents and allowed levels. The layers are filled through
    /// a wide schema and resolved with a narrow one, so a value can be out of range or at a
    /// level that the narrow schema does not allow.
    #[test]
    fn resolution_is_correct_for_every_combination() {
        let wide = int_schema(-100, 100, Levels::ALL);
        let mut cases = 0;
        for bits in 1..8u8 {
            let levels = Levels::new(bits & 1 != 0, bits & 2 != 0, bits & 4 != 0);
            let narrow = int_schema(0, 10, levels);
            for game in SLOTS {
                for system in SLOTS {
                    for global in SLOTS {
                        let g = make_layer(&wide, Level::Game, game);
                        let s = make_layer(&wide, Level::System, system);
                        let gl = make_layer(&wide, Level::Global, global);
                        let layers = Layers::try_new(gl.as_ref(), s.as_ref(), g.as_ref()).unwrap();

                        let winner = [
                            (Level::Game, game),
                            (Level::System, system),
                            (Level::Global, global),
                        ]
                        .into_iter()
                        .find(|(level, slot)| matches!(slot, Slot::Valid) && levels.allows(*level));
                        let (value, source) = match winner {
                            Some((level, _)) => (valid_value(level), Source::from(level)),
                            None => (DEFAULT, Source::Default),
                        };

                        let got = resolve_key(&narrow, &layers, KEY).unwrap();
                        let case = format!("{levels:?} {game:?} {system:?} {global:?}");
                        assert_eq!(got.value, Value::Int(value), "{case}");
                        assert_eq!(got.source, source, "{case}");
                        assert_eq!(resolve(&narrow, &layers), vec![got], "{case}");
                        cases += 1;
                    }
                }
            }
        }
        assert_eq!(cases, 7 * 4 * 4 * 4);
    }

    #[test]
    fn builtin_keys_resolve_in_game_system_global_default_order() {
        let schema = Schema::builtin();
        for entry in schema.entries() {
            let choice = |n: usize| match entry.kind {
                Kind::Choice { choices, .. } => Value::Text(choices[n % choices.len()].into()),
                Kind::Int { min, max, .. } => {
                    Value::Int(min + i64::try_from(n).unwrap() % (max - min + 1))
                }
                Kind::Bool { .. } => Value::Bool(n % 2 == 1),
                Kind::Name { .. } => Value::Text(format!("name{n}")),
            };
            for bits in 0..8u8 {
                let mut layers = [
                    Layer::new(Level::Game),
                    Layer::new(Level::System),
                    Layer::new(Level::Global),
                ];
                for (i, layer) in layers.iter_mut().enumerate() {
                    if bits & (1 << i) != 0 && entry.levels.allows(layer.level()) {
                        layer.set(&schema, entry.key, choice(i + 1)).unwrap();
                    }
                }
                let [game, system, global] = &layers;
                let set = Layers::try_new(Some(global), Some(system), Some(game)).unwrap();
                let got = resolve_key(&schema, &set, entry.key).unwrap();
                let expected = (0..3)
                    .find(|i| layers[*i].get(entry.key).is_some())
                    .map(|i| (choice(i + 1), Source::from(Level::ORDER[i])))
                    .unwrap_or((entry.kind.default_value(), Source::Default));
                assert_eq!((got.value, got.source), expected, "{} {bits}", entry.key);
            }
        }
    }

    #[test]
    fn no_layers_give_every_default() {
        let schema = Schema::builtin();
        let all = resolve(&schema, &Layers::default());
        assert_eq!(all.len(), schema.entries().len());
        for (got, entry) in all.iter().zip(schema.entries()) {
            assert_eq!(got.entry, entry);
            assert_eq!(got.value, entry.kind.default_value());
            assert_eq!(got.source, Source::Default);
        }
    }

    #[test]
    fn unknown_key_resolves_to_none() {
        let schema = Schema::builtin();
        assert!(resolve_key(&schema, &Layers::default(), "display.nope").is_none());
    }

    #[test]
    fn a_layer_in_the_wrong_slot_is_rejected() {
        let game = Layer::new(Level::Game);
        let global = Layer::new(Level::Global);
        assert_eq!(
            Layers::try_new(Some(&game), None, None).unwrap_err(),
            LayerMismatch {
                slot: Level::Global,
                found: Level::Game
            }
        );
        assert_eq!(
            Layers::try_new(None, Some(&global), None).unwrap_err(),
            LayerMismatch {
                slot: Level::System,
                found: Level::Global
            }
        );
        assert_eq!(
            Layers::try_new(None, None, Some(&global)).unwrap_err(),
            LayerMismatch {
                slot: Level::Game,
                found: Level::Global
            }
        );
    }
}
