//! JSON parsing and error rendering compatible with Lean 4's Json module.
//! In particular, offsets count UTF-8 bytes, lone surrogates are replaced,
//! and integer decoding tests the decimal exponent rather than rounding.

use num_bigint::{BigInt, BigUint};
use num_traits::{ToPrimitive, Zero};
use serde_json::{Map, Number, Value};

pub(crate) fn parse(text: &str) -> Result<Value, String> {
    let mut p = Parser { text, offset: 0 };
    p.ws();
    let value = p.value()?;
    if p.offset != text.len() {
        return p.error("expected end of input");
    }
    Ok(value)
}

struct Parser<'a> {
    text: &'a str,
    offset: usize,
}
impl Parser<'_> {
    fn error<T>(&self, message: &str) -> Result<T, String> {
        Err(format!("offset {}: {message}", self.offset))
    }
    fn peek(&self) -> Result<char, String> {
        self.text[self.offset..]
            .chars()
            .next()
            .ok_or_else(|| format!("offset {}: unexpected end of input", self.offset))
    }
    fn any(&mut self) -> Result<char, String> {
        let c = self.peek()?;
        self.offset += c.len_utf8();
        Ok(c)
    }
    fn ws(&mut self) {
        while matches!(self.peek(), Ok(' ' | '\t' | '\n' | '\r')) {
            self.offset += 1;
        }
    }
    fn expect(&self, c: char, description: &str) -> Result<(), String> {
        if self.peek()? == c {
            Ok(())
        } else {
            self.error(&format!("expected {description}"))
        }
    }
    fn hex(&mut self) -> Result<u32, String> {
        let c = self.any()?;
        if c.is_ascii_hexdigit() {
            Ok(c.to_digit(16).unwrap())
        } else {
            self.error("invalid hex character")
        }
    }
    fn surrogate(&mut self, high: u32) -> Result<char, String> {
        if self.any()? != '\\' || self.any()? != 'u' || !matches!(self.any()?, 'd' | 'D') {
            return self.error("");
        }
        let low = (self.hex()? << 8) | (self.hex()? << 4) | self.hex()?;
        if low < 0xc00 {
            return self.error("");
        }
        Ok(char::from_u32(0x10000 + ((high & 0x3ff) << 10) + (low & 0x3ff)).unwrap())
    }
    fn escape(&mut self) -> Result<char, String> {
        Ok(match self.any()? {
            '\\' => '\\',
            '"' => '"',
            '/' => '/',
            'b' => '\u{8}',
            'f' => '\u{c}',
            'n' => '\n',
            'r' => '\r',
            't' => '\t',
            'u' => {
                let code =
                    (self.hex()? << 12) | (self.hex()? << 8) | (self.hex()? << 4) | self.hex()?;
                match code {
                    0xd800..=0xdbff => {
                        let saved = self.offset;
                        match self.surrogate(code) {
                            Ok(c) => c,
                            Err(_) => {
                                self.offset = saved;
                                '\u{fffd}'
                            }
                        }
                    }
                    0xdc00..=0xdfff => '\u{fffd}',
                    _ => char::from_u32(code).unwrap(),
                }
            }
            _ => return self.error("illegal \\u escape"),
        })
    }
    fn string(&mut self) -> Result<String, String> {
        let mut out = String::new();
        loop {
            match self.any()? {
                '"' => return Ok(out),
                '\\' => out.push(self.escape()?),
                c if c >= ' ' => out.push(c),
                _ => return self.error("unexpected character in string"),
            }
        }
    }
    fn digits(&mut self, nonzero: bool, description: &str) -> Result<(), String> {
        let c = self.peek()?;
        if !c.is_ascii_digit() || (nonzero && c == '0') {
            return self.error(&format!("expected {description}"));
        }
        while self.peek().is_ok_and(|c| c.is_ascii_digit()) {
            self.offset += 1;
        }
        Ok(())
    }
    fn number(&mut self) -> Result<Value, String> {
        let start = self.offset;
        if self.peek()? == '-' {
            self.offset += 1;
        }
        if self.peek()? == '0' {
            self.offset += 1;
        } else {
            self.digits(true, "1-9")?;
        }
        if self.peek() == Ok('.') {
            self.offset += 1;
            self.digits(false, "digit")?;
        }
        if matches!(self.peek(), Ok('e' | 'E')) {
            self.offset += 1;
            let negative = self.peek()? == '-';
            if matches!(self.peek(), Ok('-' | '+')) {
                self.offset += 1;
            }
            let exp_start = self.offset;
            self.digits(false, "0-9")?;
            if !negative {
                let n: BigUint = self.text[exp_start..self.offset].parse().unwrap();
                if n > (BigUint::from(1u8) << usize::BITS) {
                    return self.error("exp too large");
                }
            }
        }
        // arbitrary_precision retains the mantissa/exponent spelling.
        Ok(Value::Number(
            self.text[start..self.offset].parse::<Number>().unwrap(),
        ))
    }
    fn value(&mut self) -> Result<Value, String> {
        let result = match self.peek()? {
            '[' => {
                self.offset += 1;
                self.ws();
                let mut values = Vec::new();
                if self.peek()? == ']' {
                    self.offset += 1;
                } else {
                    loop {
                        values.push(self.value()?);
                        match self.any()? {
                            ']' => break,
                            ',' => self.ws(),
                            _ => return self.error("unexpected character in array"),
                        }
                    }
                }
                Value::Array(values)
            }
            '{' => {
                self.offset += 1;
                self.ws();
                let mut values = Map::new();
                if self.peek()? == '}' {
                    self.offset += 1;
                } else {
                    loop {
                        self.expect('"', "\"")?;
                        self.offset += 1;
                        let key = self.string()?;
                        self.ws();
                        self.expect(':', ":")?;
                        self.offset += 1;
                        self.ws();
                        let value = self.value()?;
                        match self.any()? {
                            '}' => {
                                values.insert(key, value);
                                break;
                            }
                            ',' => {
                                values.insert(key, value);
                                self.ws();
                            }
                            _ => return self.error("unexpected character in object"),
                        }
                    }
                }
                Value::Object(values)
            }
            '"' => {
                self.offset += 1;
                Value::String(self.string()?)
            }
            c @ ('t' | 'f' | 'n') => {
                let (literal, value) = match c {
                    't' => ("true", Value::Bool(true)),
                    'f' => ("false", Value::Bool(false)),
                    _ => ("null", Value::Null),
                };
                if !self.text[self.offset..].starts_with(literal) {
                    return self.error(&format!("expected: {literal}"));
                }
                self.offset += literal.len();
                value
            }
            '-' | '0'..='9' => self.number()?,
            _ => return self.error("unexpected input"),
        };
        self.ws();
        Ok(result)
    }
}

fn number_parts(n: &Number) -> (BigInt, BigUint) {
    let spelling = n.to_string();
    let (decimal, shift) = spelling
        .split_once(['e', 'E'])
        .map_or((spelling.as_str(), BigInt::zero()), |(a, b)| {
            (a, b.parse::<BigInt>().unwrap())
        });
    let (digits, scale) = decimal
        .split_once('.')
        .map_or((decimal.to_string(), 0), |(a, b)| {
            (format!("{a}{b}"), b.len())
        });
    let mut mantissa: BigInt = digits.parse().unwrap();
    let scale = BigInt::from(scale) - shift;
    if scale < BigInt::zero() {
        // A positive decimal shift materializes the same integer as Lean.
        if !mantissa.is_zero() {
            let places = (-scale)
                .to_usize()
                .expect("JSON number exceeds addressable memory");
            let mut power = BigInt::from(1u8);
            let mut base = BigInt::from(10u8);
            let mut places = places;
            while places != 0 {
                if places & 1 != 0 {
                    power *= &base;
                }
                places >>= 1;
                if places != 0 {
                    base = &base * &base;
                }
            }
            mantissa *= power;
        }
        (mantissa, BigUint::zero())
    } else {
        (mantissa, scale.to_biguint().unwrap())
    }
}

pub(crate) fn integer(v: &Value) -> Option<String> {
    let Value::Number(n) = v else {
        return None;
    };
    let (mantissa, exponent) = number_parts(n);
    exponent.is_zero().then(|| mantissa.to_string())
}

fn number_string(n: &Number) -> String {
    let (m, e) = number_parts(n);
    if e.is_zero() {
        return m.to_string();
    }
    let negative = m < BigInt::zero();
    let digits = m.magnitude().to_string();
    let limit = BigUint::from(9 + digits.len());
    let small_exp = e.clone().min(limit.clone()).to_usize().unwrap();
    let padded = format!(
        "{}{}",
        "0".repeat((small_exp + 1).saturating_sub(digits.len())),
        digits
    );
    let (whole, frac) = padded.split_at(padded.len() - small_exp);
    let frac = frac.trim_end_matches('0');
    let sign = if negative { "-" } else { "" };
    let exponent = if e > limit {
        format!("e-{}", e - limit)
    } else {
        String::new()
    };
    if frac.is_empty() && exponent.is_empty() {
        format!("{sign}{whole}")
    } else {
        format!("{sign}{whole}.{frac}{exponent}")
    }
}

fn quote(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            c if c < ' ' => {
                use std::fmt::Write;
                write!(out, "\\u{:04x}", c as u32).unwrap();
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Lean's compact printer uses ascending object keys, its normalized decimal
/// number spelling, and Unicode escapes for control characters other than LF/CR.
/// An explicit stack also preserves the source printer's behavior on deep JSON.
pub(crate) fn compress(v: &Value) -> String {
    enum Item<'a> {
        Value(&'a Value),
        Key(&'a str),
        Char(char),
    }
    let mut output = String::new();
    let mut work = vec![Item::Value(v)];
    while let Some(item) = work.pop() {
        match item {
            Item::Key(key) => output.push_str(&quote(key)),
            Item::Char(c) => output.push(c),
            Item::Value(Value::Null) => output.push_str("null"),
            Item::Value(Value::Bool(b)) => output.push_str(if *b { "true" } else { "false" }),
            Item::Value(Value::Number(n)) => output.push_str(&number_string(n)),
            Item::Value(Value::String(s)) => output.push_str(&quote(s)),
            Item::Value(Value::Array(items)) => {
                output.push('[');
                work.push(Item::Char(']'));
                for (i, item) in items.iter().enumerate().rev() {
                    work.push(Item::Value(item));
                    if i != 0 {
                        work.push(Item::Char(','));
                    }
                }
            }
            Item::Value(Value::Object(fields)) => {
                output.push('{');
                work.push(Item::Char('}'));
                for (i, (key, value)) in fields.iter().enumerate().rev() {
                    work.push(Item::Value(value));
                    work.push(Item::Char(':'));
                    work.push(Item::Key(key));
                    if i != 0 {
                        work.push(Item::Char(','));
                    }
                }
            }
        }
    }
    output
}

enum Doc {
    Text(String),
    Line,
    Concat(Vec<Doc>),
    Nest(Box<Doc>),
    Group(Box<Doc>),
}
impl Doc {
    fn group(self) -> Self {
        Self::Group(Box::new(self))
    }
    fn bracket(open: &str, items: Vec<Self>, close: &str) -> Self {
        let mut parts = vec![Self::Text(open.into())];
        for (i, item) in items.into_iter().enumerate() {
            if i > 0 {
                parts.push(Self::Text(",".into()));
                parts.push(Self::Line);
            }
            parts.push(item);
        }
        parts.push(Self::Text(close.into()));
        Self::Nest(Box::new(Self::Concat(parts))).group()
    }
    fn json(v: &Value) -> Self {
        match v {
            Value::Array(a) => Self::bracket("[", a.iter().map(Self::json).collect(), "]"),
            Value::Object(m) => Self::bracket(
                "{",
                m.iter()
                    .rev()
                    .map(|(k, v)| {
                        Self::Concat(vec![
                            Self::Text(format!("{}:", quote(k))),
                            Self::Line,
                            Self::json(v),
                        ])
                        .group()
                    })
                    .collect(),
                "}",
            ),
            Value::String(s) => Self::Text(quote(s)),
            Value::Number(n) => Self::Text(number_string(n)),
            _ => Self::Text(v.to_string()),
        }
    }
}

pub(crate) fn pretty(v: &Value) -> String {
    fn fits(mut space: usize, mut work: Vec<(&Doc, usize, bool)>) -> bool {
        while let Some((doc, indent, flat)) = work.pop() {
            let count = match doc {
                Doc::Text(s) => s.chars().count(),
                Doc::Line if !flat => return true,
                Doc::Line => 1,
                Doc::Concat(ds) => {
                    work.extend(ds.iter().rev().map(|d| (d, indent, flat)));
                    continue;
                }
                Doc::Nest(d) => {
                    work.push((d, indent + 1, flat));
                    continue;
                }
                Doc::Group(d) => {
                    work.push((d, indent, true));
                    continue;
                }
            };
            if count > space {
                return false;
            }
            space -= count;
        }
        true
    }
    let doc = Doc::json(v);
    let mut work = vec![(&doc, 0, false)];
    let mut out = String::new();
    let mut column = 0usize;
    while let Some((doc, indent, flat)) = work.pop() {
        match doc {
            Doc::Text(s) => {
                out.push_str(s);
                column += s.chars().count();
            }
            Doc::Line if flat => {
                out.push(' ');
                column += 1;
            }
            Doc::Line => {
                out.push('\n');
                out.push_str(&" ".repeat(indent));
                column = indent;
            }
            Doc::Concat(ds) => work.extend(ds.iter().rev().map(|d| (d, indent, flat))),
            Doc::Nest(d) => work.push((d, indent + 1, flat)),
            Doc::Group(d) => {
                let flatten = flat || {
                    let mut probe = work.clone();
                    probe.push((d, indent, true));
                    fits(80usize.saturating_sub(column), probe)
                };
                work.push((d, indent, flatten));
            }
        }
    }
    out
}
