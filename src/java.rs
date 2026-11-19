//! The few `java.lang` behaviours Avro's output depends on: `Character.isLetter`/`isLetterOrDigit` on UTF-16
//! units, `String.trim`, `Double.parseDouble`, `String.hashCode`, `Math.round`, and Java's lossy string
//! encoders.

use unicode_general_category::{GeneralCategory as G, get_general_category};

/// `Character.isLetter(char)`: general category L* (a surrogate unit is not a letter).
pub(crate) fn is_letter(unit: u16) -> bool {
    char::from_u32(unit as u32).is_some_and(|c| matches!(get_general_category(c), G::UppercaseLetter | G::LowercaseLetter | G::TitlecaseLetter | G::ModifierLetter | G::OtherLetter))
}

/// `Character.isLetterOrDigit(char)`: L* or Nd.
pub(crate) fn is_letter_or_digit(unit: u16) -> bool {
    is_letter(unit) || char::from_u32(unit as u32).is_some_and(|c| get_general_category(c) == G::DecimalNumber)
}

/// `NameValidator.UTF_VALIDATOR.validate(name)`: `None` when valid, else the `Result`'s errors.
pub(crate) fn name_errors(name: Option<&str>) -> Option<String> {
    let Some(name) = name else { return Some("Null name".into()) };
    let units: Vec<u16> = name.encode_utf16().collect();
    let Some(&first) = units.first() else { return Some("Empty name".into()) };
    if !(is_letter(first) || first == b'_' as u16) {
        return Some(format!("Illegal initial character: {name}"));
    }
    if units[1..].iter().any(|&u| !(is_letter_or_digit(u) || u == b'_' as u16)) {
        return Some(format!("Illegal character in: {name}"));
    }
    None
}

/// `String.trim()`: strips characters up to U+0020 from both ends.
pub(crate) fn trim(s: &str) -> &str {
    s.trim_matches(|c: char| c <= ' ')
}

/// `String.split("\\.")`: trailing empty strings removed.
pub(crate) fn split_dots(s: &str) -> Vec<&str> {
    let mut parts: Vec<&str> = s.split('.').collect();
    while parts.len() > 1 && parts.last() == Some(&"") {
        parts.pop();
    }
    if parts.len() == 1 && parts[0].is_empty() && !s.is_empty() {
        parts.clear();
    }
    parts
}

/// `String.hashCode()` over UTF-16 units.
pub(crate) fn string_hash(units: &[u16]) -> i32 {
    units.iter().fold(0i32, |h, &u| h.wrapping_mul(31).wrapping_add(u as i32))
}

/// `String.getBytes(UTF_8)`: an unpaired surrogate becomes `?`.
pub(crate) fn utf8_bytes(units: &[u16]) -> Vec<u8> {
    let mut out = Vec::with_capacity(units.len());
    for r in char::decode_utf16(units.iter().copied()) {
        match r {
            Ok(c) => {
                let mut buf = [0u8; 4];
                out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
            }
            Err(_) => out.push(b'?'),
        }
    }
    out
}

/// `String.getBytes(ISO_8859_1)`: a code point past U+00FF (a surrogate pair counts once) becomes `?`.
pub(crate) fn latin1_bytes(units: &[u16]) -> Vec<u8> {
    char::decode_utf16(units.iter().copied()).map(|r| r.ok().filter(|c| (*c as u32) < 256).map_or(b'?', |c| c as u8)).collect()
}

/// `new String(bytes, ISO_8859_1)`.
pub(crate) fn latin1_string(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| b as char).collect()
}

/// `new String(bytes, UTF_8)`: the JDK's decoder, which replaces malformed input with U+FFFD in its own units (an
/// encoded surrogate is one replacement, where Rust's lossy decoding gives three).
pub(crate) fn utf8_string(bytes: &[u8]) -> String {
    if let Ok(s) = std::str::from_utf8(bytes) {
        return s.to_string();
    }
    const REPL: char = '\u{FFFD}';
    let cont = |b: u8| b & 0xc0 == 0x80;
    let mut out = String::with_capacity(bytes.len());
    let (mut sp, sl) = (0usize, bytes.len());
    while sp < sl {
        let b1 = bytes[sp];
        sp += 1;
        if b1 < 0x80 {
            out.push(b1 as char);
        } else if b1 >> 5 == 0b110 && b1 & 0x1e != 0 {
            if sp < sl {
                let b2 = bytes[sp];
                sp += 1;
                if !cont(b2) {
                    out.push(REPL);
                    sp -= 1;
                } else {
                    out.push(char::from_u32((((b1 & 0x1f) as u32) << 6) | (b2 & 0x3f) as u32).unwrap_or(REPL));
                }
                continue;
            }
            out.push(REPL);
            break;
        } else if b1 >> 4 == 0b1110 {
            let malformed_2 = |b2: u8| (b1 == 0xe0 && b2 & 0xe0 == 0x80) || !cont(b2);
            if sp + 1 < sl {
                let (b2, b3) = (bytes[sp], bytes[sp + 1]);
                sp += 2;
                if malformed_2(b2) || !cont(b3) {
                    out.push(REPL);
                    // `malformed3`: one byte when the second is already wrong, else two.
                    sp -= 2;
                    if !malformed_2(b2) {
                        sp += 1;
                    }
                } else {
                    let c = (((b1 & 0x0f) as u32) << 12) | (((b2 & 0x3f) as u32) << 6) | (b3 & 0x3f) as u32;
                    out.push(char::from_u32(c).unwrap_or(REPL));
                }
                continue;
            }
            if sp < sl && malformed_2(bytes[sp]) {
                out.push(REPL);
                continue;
            }
            out.push(REPL);
            break;
        } else if b1 >> 3 == 0b11110 {
            let malformed_2 = |b2: u8| (b1 == 0xf0 && !(0x90..=0xbf).contains(&b2)) || (b1 == 0xf4 && b2 & 0xf0 != 0x80) || !cont(b2);
            if sp + 2 < sl {
                let (b2, b3, b4) = (bytes[sp], bytes[sp + 1], bytes[sp + 2]);
                sp += 3;
                let uc = (((b1 & 0x07) as u32) << 18) | (((b2 & 0x3f) as u32) << 12) | (((b3 & 0x3f) as u32) << 6) | (b4 & 0x3f) as u32;
                if !cont(b2) || !cont(b3) || !cont(b4) || !(0x10000..=0x10ffff).contains(&uc) {
                    out.push(REPL);
                    // `malformed4`.
                    sp -= 3;
                    if !(b1 > 0xf4 || malformed_2(b2)) {
                        sp += if cont(b3) { 2 } else { 1 };
                    }
                } else {
                    out.push(char::from_u32(uc).unwrap_or(REPL));
                }
                continue;
            }
            if b1 > 0xf4 || (sp < sl && malformed_2(bytes[sp])) {
                out.push(REPL);
                continue;
            }
            sp += 1;
            if sp < sl && !cont(bytes[sp]) {
                out.push(REPL);
                continue;
            }
            out.push(REPL);
            break;
        } else {
            out.push(REPL);
        }
    }
    out
}

/// `Math.round(float)`: the closest `int`, ties up, saturated; `NaN` is 0.
pub(crate) fn round_f32(a: f32) -> i32 {
    // `a + 0.5` is exact in a double for every float.
    ((a as f64) + 0.5).floor() as i32
}

/// `Math.round(double)`: the closest `long`, ties up, saturated; `NaN` is 0.
pub(crate) fn round_f64(a: f64) -> i64 {
    if a.is_nan() {
        return 0;
    }
    let floor = a.floor();
    // `a - floor(a)` is exact.
    let up = a - floor >= 0.5;
    let r = if up { floor + 1.0 } else { floor };
    r as i64
}

/// `Double.parseDouble(s)`, with `NumberFormatException`'s message on failure.
pub(crate) fn parse_double(s: &str) -> Result<f64, String> {
    let t = trim(s);
    if t.is_empty() {
        return Err("empty String".into());
    }
    let fail = || format!("For input string: \"{t}\"");
    let (negative, rest) = match t.as_bytes()[0] {
        b'-' => (true, &t[1..]),
        b'+' => (false, &t[1..]),
        _ => (false, t),
    };
    let sign = |v: f64| if negative { -v } else { v };
    if rest == "NaN" {
        return Ok(f64::NAN);
    }
    if rest == "Infinity" {
        return Ok(sign(f64::INFINITY));
    }
    if rest.len() >= 2 && (rest.starts_with("0x") || rest.starts_with("0X")) {
        return parse_hex(&rest[2..]).map(sign).ok_or_else(fail);
    }
    // [digits][.digits][(e|E)[sign]digits][f|F|d|D], at least one digit before the exponent.
    let body = rest.strip_suffix(['f', 'F', 'd', 'D']).unwrap_or(rest);
    let bytes = body.as_bytes();
    let mut i = 0;
    let mut digits = 0;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
        digits += 1;
    }
    if i < bytes.len() && bytes[i] == b'.' {
        i += 1;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
            digits += 1;
        }
    }
    if digits == 0 {
        return Err(fail());
    }
    if i < bytes.len() && (bytes[i] == b'e' || bytes[i] == b'E') {
        i += 1;
        if i < bytes.len() && (bytes[i] == b'+' || bytes[i] == b'-') {
            i += 1;
        }
        let start = i;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        if i == start {
            return Err(fail());
        }
    }
    if i != bytes.len() {
        return Err(fail());
    }
    body.parse::<f64>().map(sign).map_err(|_| fail())
}

/// The hex significand and mandatory binary exponent of a Java hex float literal (`1.8p1`).
fn parse_hex(s: &str) -> Option<f64> {
    let s = s.strip_suffix(['f', 'F', 'd', 'D']).unwrap_or(s);
    let p = s.find(['p', 'P'])?;
    let (mantissa, exponent) = (&s[..p], &s[p + 1..]);
    let exponent: i64 = {
        let digits = exponent.strip_prefix(['+', '-']).unwrap_or(exponent);
        if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        exponent.parse::<i64>().unwrap_or(if exponent.starts_with('-') { i64::MIN / 2 } else { i64::MAX / 2 })
    };
    let (int_part, frac_part) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    if int_part.is_empty() && frac_part.is_empty() {
        return None;
    }
    if !int_part.bytes().chain(frac_part.bytes()).all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    // Exact significand in 128 bits (extra digits only set a sticky bit), then one rounding.
    let mut value: u128 = 0;
    let mut scale: i64 = 0;
    let mut sticky = false;
    for (i, b) in int_part.bytes().chain(frac_part.bytes()).enumerate() {
        let d = (b as char).to_digit(16).expect("hex digit") as u128;
        if value >> 120 == 0 {
            value = value * 16 + d;
            if i >= int_part.len() {
                scale -= 4;
            }
        } else {
            sticky |= d != 0;
            if i < int_part.len() {
                scale += 4;
            }
        }
    }
    if value == 0 {
        return Some(0.0);
    }
    Some(scale_to_f64(value, sticky, scale.saturating_add(exponent)))
}

/// `value * 2^exp`, correctly rounded (half-even; `sticky` marks nonzero bits below `value`).
fn scale_to_f64(value: u128, sticky: bool, exp: i64) -> f64 {
    let bits = 128 - value.leading_zeros() as i64;
    // The value is m * 2^(e) with m having `bits` bits; the double's exponent of the leading bit:
    let lead = bits - 1 + exp;
    if lead > 1023 {
        return f64::INFINITY;
    }
    // Number of significand bits kept: 53 for normals, fewer for subnormals.
    let keep = if lead >= -1022 { 53 } else { 53 - (-1022 - lead) };
    if keep <= 0 {
        // Below half the smallest subnormal (or exactly half with nothing else → rounds to even 0).
        let half = keep == 0 && (value.count_ones() > 1 || sticky);
        return if half { f64::from_bits(1) } else { 0.0 };
    }
    let shift = bits - keep;
    let (mut m, round_bit, rest) = if shift > 0 {
        let m = value >> shift;
        let round_bit = (value >> (shift - 1)) & 1 == 1;
        let rest = value & ((1u128 << (shift - 1)) - 1) != 0 || sticky;
        (m, round_bit, rest)
    } else {
        (value << (-shift), false, sticky)
    };
    if round_bit && (rest || m & 1 == 1) {
        m += 1;
    }
    let mut e2 = exp + shift;
    if m >> keep != 0 {
        m >>= 1;
        e2 += 1;
    }
    // m < 2^53 converts exactly; the exponent is applied in two steps to stay in range.
    let half = e2 / 2;
    (m as f64) * 2f64.powi(half as i32) * 2f64.powi((e2 - half) as i32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_double_follows_java() {
        assert_eq!(parse_double(" 1.5d ").unwrap(), 1.5);
        assert_eq!(parse_double("0x1.8p1").unwrap(), 3.0);
        assert_eq!(parse_double("-0x1p-2").unwrap(), -0.25);
        assert_eq!(parse_double(".5").unwrap(), 0.5);
        assert_eq!(parse_double("5.").unwrap(), 5.0);
        assert_eq!(parse_double("1e3F").unwrap(), 1000.0);
        assert!(parse_double("-NaN").unwrap().is_nan());
        assert_eq!(parse_double("-Infinity").unwrap(), f64::NEG_INFINITY);
        assert_eq!(parse_double("").unwrap_err(), "empty String");
        assert_eq!(parse_double("abc").unwrap_err(), "For input string: \"abc\"");
        assert_eq!(parse_double("1e").unwrap_err(), "For input string: \"1e\"");
        assert_eq!(parse_double("inf").unwrap_err(), "For input string: \"inf\"");
    }

    #[test]
    fn decodes_utf8_as_the_jdk() {
        assert_eq!(utf8_string(&[0xed, 0xa0, 0x80]), "\u{FFFD}");
        assert_eq!(utf8_string(&[0xc3, 0x28]), "\u{FFFD}(");
        assert_eq!(utf8_string(&[0xe2]), "\u{FFFD}");
        assert_eq!(utf8_string(&[0xc0, 0x80]), "\u{FFFD}\u{FFFD}");
        assert_eq!(utf8_string(&[0xf0, 0x9f, 0x98]), "\u{FFFD}");
        assert_eq!(utf8_string(&[0xf0, 0x9f, 0x98, 0x80]), "😀");
        assert_eq!(utf8_string(&[0xe0, 0x80, 0x41]), "\u{FFFD}\u{FFFD}A");
    }

    #[test]
    fn rounds_as_java() {
        assert_eq!(round_f32(2.5), 3);
        assert_eq!(round_f32(-2.5), -2);
        assert_eq!(round_f32(3e9), i32::MAX);
        assert_eq!(round_f32(f32::NAN), 0);
        assert_eq!(round_f64(0.49999999999999994), 0);
        assert_eq!(round_f64(-0.5), 0);
        assert_eq!(round_f64(1e19), i64::MAX);
    }

    #[test]
    fn validates_names_as_utf_validator() {
        assert_eq!(name_errors(Some("a_1")), None);
        assert_eq!(name_errors(Some("")).as_deref(), Some("Empty name"));
        assert_eq!(name_errors(None).as_deref(), Some("Null name"));
        assert_eq!(name_errors(Some("1a")).as_deref(), Some("Illegal initial character: 1a"));
        assert_eq!(name_errors(Some("a-b")).as_deref(), Some("Illegal character in: a-b"));
        assert_eq!(name_errors(Some("é")), None);
        // Letter-number and other-number are not letters or digits for Java.
        assert_eq!(name_errors(Some("aⅠ")).as_deref(), Some("Illegal character in: aⅠ"));
        assert_eq!(name_errors(Some("a²")).as_deref(), Some("Illegal character in: a²"));
        // A supplementary letter is two surrogate units, neither a letter.
        assert_eq!(name_errors(Some("𝐀")).as_deref(), Some("Illegal initial character: 𝐀"));
    }
}
