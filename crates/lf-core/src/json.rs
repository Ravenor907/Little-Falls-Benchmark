//! Just enough JSON writing for the results file. Output is canonical
//! (keys keep insertion order, fixed number formatting) so the same data
//! always hashes the same way.

use std::fmt::Write;

#[derive(Clone, Debug)]
pub enum Json {
    Null,
    Bool(bool),
    Num(f64),
    Int(i64),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

impl Json {
    pub fn obj() -> Self {
        Json::Obj(Vec::new())
    }

    /// Builder helper: `Json::obj().with("a", 1)`.
    pub fn with(mut self, key: &str, v: impl Into<Json>) -> Self {
        if let Json::Obj(ref mut f) = self {
            f.push((key.to_string(), v.into()));
        }
        self
    }

    pub fn to_string_pretty(&self) -> String {
        let mut s = String::new();
        self.write(&mut s, 0);
        s.push('\n');
        s
    }

    pub fn to_string_compact(&self) -> String {
        let mut s = String::new();
        self.write(&mut s, usize::MAX);
        s
    }

    fn write(&self, s: &mut String, indent: usize) {
        let pretty = indent != usize::MAX;
        let nl = |s: &mut String, n: usize| {
            if pretty {
                s.push('\n');
                for _ in 0..n {
                    s.push_str("  ");
                }
            }
        };
        let child = if pretty { indent + 1 } else { indent };
        match self {
            Json::Null => s.push_str("null"),
            Json::Bool(b) => s.push_str(if *b { "true" } else { "false" }),
            Json::Int(i) => {
                let _ = write!(s, "{i}");
            }
            Json::Num(n) => {
                if n.is_finite() {
                    // Fixed precision keeps output stable and readable.
                    let _ = write!(s, "{:.4}", n);
                } else {
                    s.push_str("null");
                }
            }
            Json::Str(v) => escape(s, v),
            Json::Arr(items) => {
                s.push('[');
                for (i, it) in items.iter().enumerate() {
                    if i > 0 {
                        s.push(',');
                    }
                    nl(s, child);
                    it.write(s, child);
                }
                if !items.is_empty() {
                    nl(s, indent);
                }
                s.push(']');
            }
            Json::Obj(fields) => {
                s.push('{');
                for (i, (k, v)) in fields.iter().enumerate() {
                    if i > 0 {
                        s.push(',');
                    }
                    nl(s, child);
                    escape(s, k);
                    s.push(':');
                    if pretty {
                        s.push(' ');
                    }
                    v.write(s, child);
                }
                if !fields.is_empty() {
                    nl(s, indent);
                }
                s.push('}');
            }
        }
    }
}

fn escape(s: &mut String, v: &str) {
    s.push('"');
    for c in v.chars() {
        match c {
            '"' => s.push_str("\\\""),
            '\\' => s.push_str("\\\\"),
            '\n' => s.push_str("\\n"),
            '\r' => s.push_str("\\r"),
            '\t' => s.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(s, "\\u{:04x}", c as u32);
            }
            c => s.push(c),
        }
    }
    s.push('"');
}

impl From<&str> for Json {
    fn from(v: &str) -> Self {
        Json::Str(v.to_string())
    }
}
impl From<String> for Json {
    fn from(v: String) -> Self {
        Json::Str(v)
    }
}
impl From<f64> for Json {
    fn from(v: f64) -> Self {
        Json::Num(v)
    }
}
impl From<f32> for Json {
    fn from(v: f32) -> Self {
        Json::Num(v as f64)
    }
}
impl From<i64> for Json {
    fn from(v: i64) -> Self {
        Json::Int(v)
    }
}
impl From<u64> for Json {
    fn from(v: u64) -> Self {
        Json::Int(v as i64)
    }
}
impl From<u32> for Json {
    fn from(v: u32) -> Self {
        Json::Int(v as i64)
    }
}
impl From<usize> for Json {
    fn from(v: usize) -> Self {
        Json::Int(v as i64)
    }
}
impl From<bool> for Json {
    fn from(v: bool) -> Self {
        Json::Bool(v)
    }
}
impl From<Vec<Json>> for Json {
    fn from(v: Vec<Json>) -> Self {
        Json::Arr(v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn writes_json() {
        let j = Json::obj()
            .with("a", 1u32)
            .with("b", "x\"y")
            .with("c", vec![Json::Bool(true), Json::Null]);
        assert_eq!(j.to_string_compact(), r#"{"a":1,"b":"x\"y","c":[true,null]}"#);
    }
}
