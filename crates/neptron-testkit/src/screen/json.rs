//! A small JSON writer for the measures and the survey. The crate does not
//! use `serde_json`: its `PartialEq` impls for integers would reach every
//! crate whose tests use this kit and break the type inference of
//! `assert_eq!(v, [])` there.

use std::fmt::Write;

/// A JSON value. Every number here is a whole number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Json {
    /// `null`.
    Null,
    /// `true` or `false`.
    Bool(bool),
    /// A whole number.
    Int(i128),
    /// A string.
    Str(String),
    /// An array.
    Array(Vec<Json>),
    /// An object, its keys in order.
    Object(Vec<(String, Json)>),
}

/// An object of `pairs`.
pub fn object<const N: usize>(pairs: [(&str, Json); N]) -> Json {
    Json::Object(pairs.into_iter().map(|(k, v)| (k.to_owned(), v)).collect())
}

impl Json {
    /// The value of `key` of an object, or `None`.
    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Self::Object(pairs) => pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    /// Sets `key` of an object; a value that is not an object is kept.
    pub fn insert(&mut self, key: &str, value: Json) {
        if let Self::Object(pairs) = self {
            match pairs.iter_mut().find(|(k, _)| k == key) {
                Some((_, v)) => *v = value,
                None => pairs.push((key.to_owned(), value)),
            }
        }
    }

    /// The items of an array, or `None`.
    pub fn as_array(&self) -> Option<&[Json]> {
        match self {
            Self::Array(items) => Some(items),
            _ => None,
        }
    }

    /// The value as text on one line.
    pub fn compact(&self) -> String {
        let mut out = String::new();
        self.write(&mut out, None, 0);
        out
    }

    /// The value as text, two spaces per level.
    pub fn pretty(&self) -> String {
        let mut out = String::new();
        self.write(&mut out, Some(2), 0);
        out.push('\n');
        out
    }

    fn write(&self, out: &mut String, indent: Option<usize>, level: usize) {
        let newline = |out: &mut String, level: usize| {
            if let Some(n) = indent {
                out.push('\n');
                out.push_str(&" ".repeat(n * level));
            }
        };
        match self {
            Self::Null => out.push_str("null"),
            Self::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Self::Int(n) => {
                // Writing to a String does not fail.
                let _ = write!(out, "{n}");
            }
            Self::Str(s) => string(out, s),
            Self::Array(items) if items.is_empty() => out.push_str("[]"),
            Self::Object(pairs) if pairs.is_empty() => out.push_str("{}"),
            Self::Array(items) => {
                out.push('[');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    newline(out, level + 1);
                    item.write(out, indent, level + 1);
                }
                newline(out, level);
                out.push(']');
            }
            Self::Object(pairs) => {
                out.push('{');
                for (i, (key, value)) in pairs.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    newline(out, level + 1);
                    string(out, key);
                    out.push_str(if indent.is_some() { ": " } else { ":" });
                    value.write(out, indent, level + 1);
                }
                newline(out, level);
                out.push('}');
            }
        }
    }
}

fn string(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c if u32::from(c) < 0x20 => {
                // Writing to a String does not fail.
                let _ = write!(out, "\\u{:04x}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

macro_rules! from_int {
    ($($t:ty),*) => {$(
        impl From<$t> for Json {
            fn from(n: $t) -> Self {
                Self::Int(i128::from(n))
            }
        }
    )*};
}
from_int!(u8, u16, u32, u64, i32, i64);

impl From<bool> for Json {
    fn from(b: bool) -> Self {
        Self::Bool(b)
    }
}

impl From<&str> for Json {
    fn from(s: &str) -> Self {
        Self::Str(s.to_owned())
    }
}

impl From<String> for Json {
    fn from(s: String) -> Self {
        Self::Str(s)
    }
}

impl From<&String> for Json {
    fn from(s: &String) -> Self {
        Self::Str(s.clone())
    }
}

impl<T: Into<Json>> From<Option<T>> for Json {
    fn from(v: Option<T>) -> Self {
        v.map_or(Self::Null, Into::into)
    }
}

impl<T: Into<Json>> From<Vec<T>> for Json {
    fn from(v: Vec<T>) -> Self {
        Self::Array(v.into_iter().map(Into::into).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_print_as_json() {
        let mut v = object([
            ("a", 1_u32.into()),
            ("b", Json::from(vec!["x\"y", "z"])),
            ("c", Json::from(None::<u32>)),
            ("d", true.into()),
            ("e", Json::Array(Vec::new())),
        ]);
        assert_eq!(
            v.compact(),
            r#"{"a":1,"b":["x\"y","z"],"c":null,"d":true,"e":[]}"#
        );
        v.insert("a", (-3_i64).into());
        assert_eq!(v.get("a"), Some(&Json::Int(-3)));
        assert_eq!(
            object([("k", Json::from(vec![1_u8]))]).pretty(),
            "{\n  \"k\": [\n    1\n  ]\n}\n"
        );
        assert!(Json::Null.as_array().is_none());
    }
}
