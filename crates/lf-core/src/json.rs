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

impl Json {
    /// Field lookup on objects.
    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Obj(f) => f.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Json::Num(n) => Some(*n),
            Json::Int(i) => Some(*i as f64),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Json::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[Json]> {
        match self {
            Json::Arr(a) => Some(a),
            _ => None,
        }
    }

    /// Path lookup: `j.at(&["scores", "overall"])`.
    pub fn at(&self, path: &[&str]) -> Option<&Json> {
        path.iter().try_fold(self, |j, k| j.get(k))
    }
}

/// Parses JSON text. Strict enough for files this app writes, tolerant
/// enough for hand-edited ones; numbers with a fraction or exponent
/// become `Num`, others `Int`.
pub fn parse(text: &str) -> Result<Json, String> {
    let mut p = Parser {
        s: text.as_bytes(),
        i: 0,
    };
    let v = p.value()?;
    p.ws();
    if p.i != p.s.len() {
        return Err(format!("trailing data at byte {}", p.i));
    }
    Ok(v)
}

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
}

impl Parser<'_> {
    fn ws(&mut self) {
        while self.i < self.s.len() && matches!(self.s[self.i], b' ' | b'\n' | b'\r' | b'\t') {
            self.i += 1;
        }
    }

    fn eat(&mut self, c: u8) -> Result<(), String> {
        self.ws();
        if self.s.get(self.i) == Some(&c) {
            self.i += 1;
            Ok(())
        } else {
            Err(format!("expected '{}' at byte {}", c as char, self.i))
        }
    }

    fn lit(&mut self, word: &str, v: Json) -> Result<Json, String> {
        if self.s[self.i..].starts_with(word.as_bytes()) {
            self.i += word.len();
            Ok(v)
        } else {
            Err(format!("bad literal at byte {}", self.i))
        }
    }

    fn value(&mut self) -> Result<Json, String> {
        self.ws();
        match self.s.get(self.i).copied() {
            Some(b'{') => {
                self.i += 1;
                let mut fields = Vec::new();
                self.ws();
                if self.s.get(self.i) == Some(&b'}') {
                    self.i += 1;
                    return Ok(Json::Obj(fields));
                }
                loop {
                    self.ws();
                    let k = self.string()?;
                    self.eat(b':')?;
                    fields.push((k, self.value()?));
                    self.ws();
                    match self.s.get(self.i) {
                        Some(b',') => self.i += 1,
                        Some(b'}') => {
                            self.i += 1;
                            return Ok(Json::Obj(fields));
                        }
                        _ => return Err(format!("expected ',' or '}}' at byte {}", self.i)),
                    }
                }
            }
            Some(b'[') => {
                self.i += 1;
                let mut items = Vec::new();
                self.ws();
                if self.s.get(self.i) == Some(&b']') {
                    self.i += 1;
                    return Ok(Json::Arr(items));
                }
                loop {
                    items.push(self.value()?);
                    self.ws();
                    match self.s.get(self.i) {
                        Some(b',') => self.i += 1,
                        Some(b']') => {
                            self.i += 1;
                            return Ok(Json::Arr(items));
                        }
                        _ => return Err(format!("expected ',' or ']' at byte {}", self.i)),
                    }
                }
            }
            Some(b'"') => Ok(Json::Str(self.string()?)),
            Some(b't') => self.lit("true", Json::Bool(true)),
            Some(b'f') => self.lit("false", Json::Bool(false)),
            Some(b'n') => self.lit("null", Json::Null),
            Some(c) if c == b'-' || c.is_ascii_digit() => self.number(),
            _ => Err(format!("unexpected input at byte {}", self.i)),
        }
    }

    fn number(&mut self) -> Result<Json, String> {
        let start = self.i;
        let mut float = false;
        while let Some(&c) = self.s.get(self.i) {
            match c {
                b'0'..=b'9' | b'-' | b'+' => {}
                b'.' | b'e' | b'E' => float = true,
                _ => break,
            }
            self.i += 1;
        }
        let t = std::str::from_utf8(&self.s[start..self.i]).map_err(|e| e.to_string())?;
        if float {
            t.parse().map(Json::Num).map_err(|_| format!("bad number '{t}'"))
        } else {
            t.parse::<i64>()
                .map(Json::Int)
                .or_else(|_| t.parse().map(Json::Num))
                .map_err(|_| format!("bad number '{t}'"))
        }
    }

    fn string(&mut self) -> Result<String, String> {
        self.ws();
        if self.s.get(self.i) != Some(&b'"') {
            return Err(format!("expected string at byte {}", self.i));
        }
        self.i += 1;
        let mut out = Vec::new();
        loop {
            let c = *self.s.get(self.i).ok_or("unterminated string")?;
            self.i += 1;
            match c {
                b'"' => break,
                b'\\' => {
                    let e = *self.s.get(self.i).ok_or("bad escape")?;
                    self.i += 1;
                    match e {
                        b'n' => out.push(b'\n'),
                        b't' => out.push(b'\t'),
                        b'r' => out.push(b'\r'),
                        b'b' => out.push(8),
                        b'f' => out.push(12),
                        b'u' => {
                            let hex = self.s.get(self.i..self.i + 4).ok_or("bad \\u escape")?;
                            let cp =
                                u32::from_str_radix(std::str::from_utf8(hex).map_err(|e| e.to_string())?, 16)
                                    .map_err(|e| e.to_string())?;
                            self.i += 4;
                            let ch = char::from_u32(cp).unwrap_or('\u{fffd}');
                            let mut buf = [0u8; 4];
                            out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                        }
                        other => out.push(other),
                    }
                }
                _ => out.push(c),
            }
        }
        String::from_utf8(out).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_what_it_writes() {
        let j = Json::obj()
            .with("a", 1u32)
            .with("b", "x\"y\\n")
            .with("c", vec![Json::Bool(true), Json::Null, Json::Num(2.5)])
            .with("d", Json::obj().with("e", -3i64));
        for text in [j.to_string_compact(), j.to_string_pretty()] {
            let back = parse(&text).unwrap();
            assert_eq!(back.to_string_compact(), j.to_string_compact());
        }
        assert_eq!(
            parse("{\"x\": {\"y\": 7}}")
                .unwrap()
                .at(&["x", "y"])
                .unwrap()
                .as_f64(),
            Some(7.0)
        );
        assert!(parse("{\"x\": }").is_err());
    }

    #[test]
    fn writes_json() {
        let j = Json::obj()
            .with("a", 1u32)
            .with("b", "x\"y")
            .with("c", vec![Json::Bool(true), Json::Null]);
        assert_eq!(j.to_string_compact(), r#"{"a":1,"b":"x\"y","c":[true,null]}"#);
    }
}
