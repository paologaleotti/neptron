//! A small JSON writer. The dependency rules allow no serde_json in this
//! tool, and the report needs only writing, never reading.

use std::fmt::Write as _;

/// A JSON value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Json {
    /// `null`
    Null,
    /// `true` or `false`
    Bool(bool),
    /// A whole number.
    Int(i128),
    /// A string.
    Str(String),
    /// An array.
    Array(Vec<Json>),
    /// An object, with its keys in insertion order.
    Object(Vec<(String, Json)>),
}

impl Json {
    /// An empty object.
    pub fn object() -> Json {
        Json::Object(Vec::new())
    }

    /// Adds a key to an object and returns it. On a value that is not an
    /// object it does nothing.
    pub fn with(mut self, key: &str, value: impl Into<Json>) -> Json {
        if let Json::Object(fields) = &mut self {
            fields.push((key.to_owned(), value.into()));
        }
        self
    }

    /// The value as indented JSON text with a final newline.
    pub fn pretty(&self) -> String {
        let mut out = String::new();
        self.write(&mut out, 0);
        out.push('\n');
        out
    }

    fn write(&self, out: &mut String, depth: usize) {
        match self {
            Json::Null => out.push_str("null"),
            Json::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Json::Int(n) => out.push_str(&n.to_string()),
            Json::Str(s) => write_str(out, s),
            Json::Array(items) if items.is_empty() => out.push_str("[]"),
            Json::Object(fields) if fields.is_empty() => out.push_str("{}"),
            Json::Array(items) => {
                out.push('[');
                for (i, item) in items.iter().enumerate() {
                    out.push_str(if i == 0 { "\n" } else { ",\n" });
                    indent(out, depth + 1);
                    item.write(out, depth + 1);
                }
                out.push('\n');
                indent(out, depth);
                out.push(']');
            }
            Json::Object(fields) => {
                out.push('{');
                for (i, (key, value)) in fields.iter().enumerate() {
                    out.push_str(if i == 0 { "\n" } else { ",\n" });
                    indent(out, depth + 1);
                    write_str(out, key);
                    out.push_str(": ");
                    value.write(out, depth + 1);
                }
                out.push('\n');
                indent(out, depth);
                out.push('}');
            }
        }
    }
}

fn indent(out: &mut String, depth: usize) {
    for _ in 0..depth {
        out.push_str("  ");
    }
}

fn write_str(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if u32::from(c) < 0x20 => {
                // Writing to a String cannot fail.
                let _ = write!(out, "\\u{:04x}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

impl From<bool> for Json {
    fn from(v: bool) -> Json {
        Json::Bool(v)
    }
}

impl From<u16> for Json {
    fn from(v: u16) -> Json {
        Json::Int(i128::from(v))
    }
}

impl From<u32> for Json {
    fn from(v: u32) -> Json {
        Json::Int(i128::from(v))
    }
}

impl From<u64> for Json {
    fn from(v: u64) -> Json {
        Json::Int(i128::from(v))
    }
}

impl From<i64> for Json {
    fn from(v: i64) -> Json {
        Json::Int(i128::from(v))
    }
}

impl From<usize> for Json {
    fn from(v: usize) -> Json {
        Json::Int(i128::try_from(v).unwrap_or(i128::MAX))
    }
}

impl From<&str> for Json {
    fn from(v: &str) -> Json {
        Json::Str(v.to_owned())
    }
}

impl From<String> for Json {
    fn from(v: String) -> Json {
        Json::Str(v)
    }
}

impl<T: Into<Json>> From<Option<T>> for Json {
    fn from(v: Option<T>) -> Json {
        v.map_or(Json::Null, Into::into)
    }
}

impl<T: Into<Json>> From<Vec<T>> for Json {
    fn from(v: Vec<T>) -> Json {
        Json::Array(v.into_iter().map(Into::into).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalars() {
        assert_eq!(Json::Null.pretty(), "null\n");
        assert_eq!(Json::from(true).pretty(), "true\n");
        assert_eq!(Json::from(u64::MAX).pretty(), "18446744073709551615\n");
        assert_eq!(Json::from(-3_i64).pretty(), "-3\n");
        assert_eq!(Json::from(None::<u32>).pretty(), "null\n");
    }

    #[test]
    fn strings_are_escaped() {
        assert_eq!(
            Json::from("a\"b\\c\nd\u{1}é").pretty(),
            "\"a\\\"b\\\\c\\nd\\u0001é\"\n"
        );
    }

    #[test]
    fn nested_values_are_indented_in_order() {
        let v = Json::object()
            .with("b", 1_u32)
            .with("a", vec!["x", "y"])
            .with("e", Json::object())
            .with("f", Vec::<u32>::new());
        assert_eq!(
            v.pretty(),
            "{\n  \"b\": 1,\n  \"a\": [\n    \"x\",\n    \"y\"\n  ],\n  \"e\": {},\n  \"f\": []\n}\n"
        );
    }

    #[test]
    fn with_on_a_non_object_does_nothing() {
        assert_eq!(Json::Null.with("a", 1_u32), Json::Null);
    }
}
