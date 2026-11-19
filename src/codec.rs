//! The generic datum: what `GenericDatumReader`'s fast reader (`FastReaderBuilder`) builds from a decoder, and what
//! `GenericDatumWriter` writes to `BinaryEncoder` and `JsonEncoder`.

use crate::MAX_DEPTH;
use crate::binary::{BinaryDecoder, BinaryEncoder};
use crate::error::{self, Error};
use crate::java;
use crate::jhashmap::JavaHashMap;
use crate::json_decoder::{JsonDecoder, Text};
use crate::schema::{self, Named, Primitive, Schema, Type};

/// `SystemLimitException`'s cap on collection elements that take no bytes (`array<null>` and the like). Upstream
/// derives it from the JVM heap (`maxMemory() / 4 / 8`); this is the value for a 32 MiB heap.
pub const MAX_ZERO_BYTE_COLLECTION_ALLOCATION: i64 = 1 << 20;

/// A generic datum: `Utf8`/`String` values keep their UTF-8 bytes; maps are in `HashMap` iteration order.
pub(crate) enum Datum {
    Null,
    Boolean(bool),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    Bytes(Vec<u8>),
    String(Vec<u8>),
    Record(Vec<Datum>),
    Enum(usize),
    Fixed(Vec<u8>),
    Array(Vec<Datum>),
    Map(Vec<(Vec<u8>, Datum)>),
    Union(usize, Box<Datum>),
}

/// A map key: `Utf8` (hashed and compared by its bytes) or, under `"avro.java.string": "String"`, a `String`.
#[derive(PartialEq)]
enum Key {
    Utf8(Vec<u8>),
    Str(Vec<u16>),
}

/// `Utf8.hashCode()`: from 1, each byte signed.
fn utf8_hash(bytes: &[u8]) -> i32 {
    bytes.iter().fold(1i32, |h, &b| h.wrapping_mul(31).wrapping_add(b as i8 as i32))
}

/// What the fast reader asks of a decoder.
pub(crate) trait Decoder {
    fn read_null(&mut self) -> Result<(), Error>;
    fn read_boolean(&mut self) -> Result<bool, Error>;
    fn read_int(&mut self) -> Result<i32, Error>;
    fn read_long(&mut self) -> Result<i64, Error>;
    fn read_float(&mut self) -> Result<f32, Error>;
    fn read_double(&mut self) -> Result<f64, Error>;
    fn read_string(&mut self) -> Result<Text, Error>;
    fn read_bytes(&mut self) -> Result<Vec<u8>, Error>;
    fn read_fixed(&mut self, size: usize) -> Result<Vec<u8>, Error>;
    fn read_enum(&mut self) -> Result<i32, Error>;
    fn read_index(&mut self) -> Result<i32, Error>;
    fn read_array_start(&mut self) -> Result<i64, Error>;
    fn array_next(&mut self) -> Result<i64, Error>;
    fn read_map_start(&mut self) -> Result<i64, Error>;
    fn map_next(&mut self) -> Result<i64, Error>;
    /// `remainingBytes()`: `None` for `-1`.
    fn remaining_bytes(&self) -> Option<usize>;
}

macro_rules! delegate {
    ($t:ty, $($body:tt)*) => {
        impl Decoder for $t {
            fn read_null(&mut self) -> Result<(), Error> { Self::read_null(self) }
            fn read_boolean(&mut self) -> Result<bool, Error> { Self::read_boolean(self) }
            fn read_int(&mut self) -> Result<i32, Error> { Self::read_int(self) }
            fn read_long(&mut self) -> Result<i64, Error> { Self::read_long(self) }
            fn read_float(&mut self) -> Result<f32, Error> { Self::read_float(self) }
            fn read_double(&mut self) -> Result<f64, Error> { Self::read_double(self) }
            fn read_bytes(&mut self) -> Result<Vec<u8>, Error> { Self::read_bytes(self) }
            fn read_fixed(&mut self, size: usize) -> Result<Vec<u8>, Error> { Self::read_fixed(self, size) }
            fn read_enum(&mut self) -> Result<i32, Error> { Self::read_enum(self) }
            fn read_index(&mut self) -> Result<i32, Error> { Self::read_index(self) }
            fn read_array_start(&mut self) -> Result<i64, Error> { Self::read_array_start(self) }
            fn array_next(&mut self) -> Result<i64, Error> { Self::array_next(self) }
            fn read_map_start(&mut self) -> Result<i64, Error> { Self::read_map_start(self) }
            fn map_next(&mut self) -> Result<i64, Error> { Self::map_next(self) }
            $($body)*
        }
    };
}

delegate!(JsonDecoder<>,
    fn read_string(&mut self) -> Result<Text, Error> { Self::read_string(self) }
    fn remaining_bytes(&self) -> Option<usize> { None }
);

delegate!(BinaryDecoder<'_>,
    fn read_string(&mut self) -> Result<Text, Error> { Self::read_string(self).map(Text::Bytes) }
    fn remaining_bytes(&self) -> Option<usize> { Some(Self::remaining_bytes(self)) }
);

impl Text {
    fn into_bytes(self) -> Vec<u8> {
        match self {
            Text::Bytes(b) => b,
            Text::Units(u) => java::utf8_bytes(&u),
        }
    }
}

/// `GenericDatumReader.read(null, decoder)` through the fast reader.
pub(crate) struct Reader<'s> {
    schema: &'s Schema,
    /// The collection-allocation scope: zero-byte elements read so far in this datum.
    zero_byte_elements: i64,
}

impl<'s> Reader<'s> {
    pub(crate) fn new(schema: &'s Schema) -> Reader<'s> {
        Reader { schema, zero_byte_elements: 0 }
    }

    /// Reads one datum. Iterative, so hostile nesting costs heap, not stack.
    pub(crate) fn read(&mut self, d: &mut dyn Decoder) -> Result<Datum, Error> {
        let mut stack: Vec<Frame<'s>> = Vec::new();
        let mut next: Option<&'s Type> = Some(&self.schema.root);
        loop {
            let mut done = match next.take() {
                Some(t) => self.start(t, d, &mut stack, &mut next)?,
                None => None,
            };
            while let Some(datum) = done.take() {
                let Some(frame) = stack.last_mut() else { return Ok(datum) };
                done = self.deliver(frame, datum, d, &mut next)?;
                if done.is_some() {
                    done = Some(stack.pop().expect("a frame").finish());
                }
            }
        }
    }

    fn named(&self) -> &'s [Named] {
        &self.schema.named
    }

    fn push(stack: &mut Vec<Frame<'s>>, frame: Frame<'s>) -> Result<(), Error> {
        if stack.len() >= MAX_DEPTH {
            return Err(Error::runtime(format!("Datum nesting depth ({}) exceeds the maximum allowed ({MAX_DEPTH})", stack.len() + 1)));
        }
        stack.push(frame);
        Ok(())
    }

    /// Starts reading a value of `t`: a scalar comes back at once; a container opens a frame and names its first
    /// child in `next` (or comes back empty).
    fn start(&mut self, t: &'s Type, d: &mut dyn Decoder, stack: &mut Vec<Frame<'s>>, next: &mut Option<&'s Type>) -> Result<Option<Datum>, Error> {
        Ok(Some(match t {
            Type::Primitive(p, _) => match p {
                Primitive::Null => {
                    d.read_null()?;
                    Datum::Null
                }
                Primitive::Boolean => Datum::Boolean(d.read_boolean()?),
                Primitive::Int => Datum::Int(d.read_int()?),
                Primitive::Long => Datum::Long(d.read_long()?),
                Primitive::Float => Datum::Float(d.read_float()?),
                Primitive::Double => Datum::Double(d.read_double()?),
                Primitive::Bytes => Datum::Bytes(d.read_bytes()?),
                Primitive::String => Datum::String(d.read_string()?.into_bytes()),
            },
            Type::Named(i) => match &self.named()[*i] {
                Named::Record(r) => {
                    let Some(first) = r.fields.first() else { return Ok(Some(Datum::Record(Vec::new()))) };
                    Self::push(stack, Frame::Record { fields: &r.fields, values: Vec::with_capacity(r.fields.len()) })?;
                    *next = Some(&first.schema);
                    return Ok(None);
                }
                Named::Enum(e) => {
                    let index = d.read_enum()?;
                    if index < 0 || index as usize >= e.symbols.len() {
                        return Err(Error::avro_type(format!("Enumeration out of range: must be in [0, {}), but received {index}", e.symbols.len())));
                    }
                    Datum::Enum(index as usize)
                }
                Named::Fixed(f) => Datum::Fixed(d.read_fixed(f.size)?),
                Named::Unresolved { .. } => unreachable!("a parsed schema has no placeholders"),
            },
            Type::Union(types) => {
                let selection = d.read_index()?;
                if selection < 0 || selection as usize >= types.len() {
                    return Err(Error::avro_type(format!("Union branch index out of range: must be in [0, {}), but received {selection}", types.len())));
                }
                Self::push(stack, Frame::Union(selection as usize))?;
                *next = Some(&types[selection as usize]);
                return Ok(None);
            }
            Type::Array(items, _) => {
                let min_bytes = schema::min_bytes(self.named(), items, &mut Vec::new());
                let l = d.read_array_start()?;
                self.check_array_block(d, min_bytes, l)?;
                if l <= 0 {
                    return Ok(Some(Datum::Array(Vec::new())));
                }
                Self::push(stack, Frame::Array { items, min_bytes, left: l, values: Vec::new() })?;
                *next = Some(items);
                return Ok(None);
            }
            Type::Map(values, props) => {
                let string_keys = props.text("avro.java.string") == Some("String");
                let l = d.read_map_start()?;
                if l <= 0 {
                    return Ok(Some(Datum::Map(Vec::new())));
                }
                let key = Self::key(d.read_string()?, string_keys);
                Self::push(stack, Frame::Map { values, string_keys, left: l, map: JavaHashMap::new(), key: Some(key) })?;
                *next = Some(values);
                return Ok(None);
            }
        }))
    }

    /// A map key as `HashMap` hashes and compares it.
    fn key(text: Text, string_keys: bool) -> (i32, Key) {
        if string_keys {
            let units = match text {
                Text::Units(u) => u,
                Text::Bytes(b) => java::utf8_string(&b).encode_utf16().collect(),
            };
            (java::string_hash(&units), Key::Str(units))
        } else {
            let bytes = text.into_bytes();
            (utf8_hash(&bytes), Key::Utf8(bytes))
        }
    }

    /// Hands a child's value to its container: `Some` when the container is complete (the caller pops it), else
    /// the next child's type in `next`.
    fn deliver(&mut self, frame: &mut Frame<'s>, datum: Datum, d: &mut dyn Decoder, next: &mut Option<&'s Type>) -> Result<Option<Datum>, Error> {
        match frame {
            Frame::Record { fields, values } => {
                values.push(datum);
                match fields.get(values.len()) {
                    Some(f) => *next = Some(&f.schema),
                    None => return Ok(Some(Datum::Null)),
                }
            }
            Frame::Union(index) => {
                *frame = Frame::UnionDone(*index, Some(datum));
                return Ok(Some(Datum::Null));
            }
            Frame::UnionDone(..) => unreachable!("a finished union"),
            Frame::Array { items, min_bytes, left, values } => {
                values.push(datum);
                *left -= 1;
                if *left == 0 {
                    let l = d.array_next()?;
                    self.check_array_block(d, *min_bytes, l)?;
                    if l <= 0 {
                        return Ok(Some(Datum::Null));
                    }
                    *left = l;
                }
                *next = Some(items);
            }
            Frame::Map { values, string_keys, left, map, key } => {
                let (hash, k) = key.take().expect("a key");
                map.put(hash, k, datum);
                *left -= 1;
                if *left == 0 {
                    let l = d.map_next()?;
                    if l <= 0 {
                        return Ok(Some(Datum::Null));
                    }
                    *left = l;
                }
                *key = Some(Self::key(d.read_string()?, *string_keys));
                *next = Some(values);
            }
        }
        Ok(None)
    }

    /// `FastReaderBuilder.checkArrayBlock`.
    fn check_array_block(&mut self, d: &dyn Decoder, min_bytes: i64, count: i64) -> Result<(), Error> {
        if count <= 0 {
            return Ok(());
        }
        if min_bytes == 0 {
            let total = self.zero_byte_elements.checked_add(count);
            match total {
                Some(total) if total <= MAX_ZERO_BYTE_COLLECTION_ALLOCATION => self.zero_byte_elements = total,
                _ => {
                    let shown = total.map_or_else(|| "more than Long.MAX_VALUE".to_string(), |t| t.to_string());
                    return Err(Error::new(
                        error::SYSTEM_LIMIT,
                        format!(
                            "Cannot allocate {shown} collection elements whose minimum encoded size is zero: exceeds the maximum allowed of {MAX_ZERO_BYTE_COLLECTION_ALLOCATION} (configure with the system property org.apache.avro.limits.collectionItems.maxAllocation)"
                        ),
                    ));
                }
            }
        } else if let Some(remaining) = d.remaining_bytes() {
            if count.saturating_mul(min_bytes) > remaining as i64 {
                return Err(Error::new(error::EOF, format!("Collection claims {count} elements with at least {min_bytes} bytes each, but only {remaining} bytes are available")));
            }
        }
        Ok(())
    }
}

/// A container being read.
enum Frame<'s> {
    Record { fields: &'s [crate::schema::Field], values: Vec<Datum> },
    Union(usize),
    UnionDone(usize, Option<Datum>),
    Array { items: &'s Type, min_bytes: i64, left: i64, values: Vec<Datum> },
    Map { values: &'s Type, string_keys: bool, left: i64, map: JavaHashMap<Key, Datum>, key: Option<(i32, Key)> },
}

impl Frame<'_> {
    fn finish(self) -> Datum {
        match self {
            Frame::Record { values, .. } => Datum::Record(values),
            Frame::UnionDone(index, value) => Datum::Union(index, Box::new(value.expect("a branch value"))),
            Frame::Union(_) => unreachable!("an unfinished union"),
            Frame::Array { values, .. } => Datum::Array(values),
            Frame::Map { map, .. } => Datum::Map(
                map.into_vec()
                    .into_iter()
                    .map(|(k, v)| {
                        let bytes = match k {
                            Key::Utf8(b) => b,
                            Key::Str(u) => java::utf8_bytes(&u),
                        };
                        (bytes, v)
                    })
                    .collect(),
            ),
        }
    }
}

/// `GenericDatumWriter.write(datum, binaryEncoder)`.
pub(crate) fn write_binary(named: &[Named], t: &Type, datum: &Datum, out: &mut BinaryEncoder) {
    match (t, datum) {
        (_, Datum::Null) => {}
        (_, Datum::Boolean(b)) => out.write_boolean(*b),
        (_, Datum::Int(n)) => out.write_int(*n),
        (_, Datum::Long(n)) => out.write_long(*n),
        (_, Datum::Float(f)) => out.write_float(*f),
        (_, Datum::Double(d)) => out.write_double(*d),
        (_, Datum::Bytes(b) | Datum::String(b)) => out.write_bytes(b),
        (_, Datum::Fixed(b)) => out.write_fixed(b),
        (_, Datum::Enum(i)) => out.write_int(*i as i32),
        (Type::Named(i), Datum::Record(values)) => {
            if let Named::Record(r) = &named[*i] {
                for (f, v) in r.fields.iter().zip(values) {
                    write_binary(named, &f.schema, v, out);
                }
            }
        }
        (Type::Union(types), Datum::Union(i, v)) => {
            // `resolveUnion` finds the branch the datum was read with.
            out.write_int(*i as i32);
            write_binary(named, &types[*i], v, out);
        }
        (Type::Array(items, _), Datum::Array(values)) => {
            out.write_block_count(values.len());
            for v in values {
                write_binary(named, items, v, out);
            }
            out.write_long(0);
        }
        (Type::Map(values_type, _), Datum::Map(entries)) => {
            out.write_block_count(entries.len());
            for (k, v) in entries {
                out.write_bytes(k);
                write_binary(named, values_type, v, out);
            }
            out.write_long(0);
        }
        _ => unreachable!("a datum read with its schema"),
    }
}

/// A string as `UTF8JsonGenerator` writes it: Jackson's escapes, and a character past U+FFFF as its two surrogates
/// escaped (`\uD83D\uDE00`).
fn quote(out: &mut String, s: &str) {
    out.push('"');
    let mut start = 0;
    for (i, c) in s.char_indices() {
        if (c as u32) > 0xFFFF {
            jackson::json::write::escape_into(out, &s[start..i]);
            let mut units = [0u16; 2];
            for u in c.encode_utf16(&mut units) {
                out.push_str(&format!("\\u{:04X}", u));
            }
            start = i + c.len_utf8();
        }
    }
    jackson::json::write::escape_into(out, &s[start..]);
    out.push('"');
}

/// Whether writing a value of `t` reaches the encoder at all: a record whose fields are all such records does
/// not, and `JsonEncoder` writes nothing for a datum that never advances its grammar.
fn writes_terminal(named: &[Named], t: &Type, visiting: &mut Vec<usize>) -> bool {
    match t {
        Type::Named(i) => match &named[*i] {
            Named::Record(r) => {
                if visiting.contains(i) {
                    return false;
                }
                visiting.push(*i);
                let any = r.fields.iter().any(|f| writes_terminal(named, &f.schema, visiting));
                visiting.pop();
                any
            }
            _ => true,
        },
        _ => true,
    }
}

/// `GenericDatumWriter.write(datum, jsonEncoder)` and `flush()`: compact JSON, unions as `{"branch": value}` except
/// `null`.
pub(crate) fn write_json_datum(named: &[Named], t: &Type, datum: &Datum, out: &mut String) {
    if writes_terminal(named, t, &mut Vec::new()) {
        write_json(named, t, datum, out);
    }
}

fn write_json(named: &[Named], t: &Type, datum: &Datum, out: &mut String) {
    match (t, datum) {
        (_, Datum::Null) => out.push_str("null"),
        (_, Datum::Boolean(b)) => out.push_str(if *b { "true" } else { "false" }),
        (_, Datum::Int(n)) => out.push_str(&n.to_string()),
        (_, Datum::Long(n)) => out.push_str(&n.to_string()),
        (_, Datum::Float(f)) => {
            let text = java_compat::float_to_string(*f);
            if f.is_finite() { out.push_str(&text) } else { quote(out, &text) }
        }
        (_, Datum::Double(d)) => {
            let text = java_compat::double_to_string(*d);
            if d.is_finite() { out.push_str(&text) } else { quote(out, &text) }
        }
        (_, Datum::Bytes(b) | Datum::Fixed(b)) => quote(out, &java::latin1_string(b)),
        (_, Datum::String(b)) => quote(out, &java::utf8_string(b)),
        (Type::Named(i), Datum::Enum(n)) => {
            if let Named::Enum(e) = &named[*i] {
                quote(out, &e.symbols[*n]);
            }
        }
        (Type::Named(i), Datum::Record(values)) => {
            if let Named::Record(r) = &named[*i] {
                out.push('{');
                for (n, (f, v)) in r.fields.iter().zip(values).enumerate() {
                    if n > 0 {
                        out.push(',');
                    }
                    quote(out, &f.name);
                    out.push(':');
                    write_json(named, &f.schema, v, out);
                }
                out.push('}');
            }
        }
        (Type::Union(types), Datum::Union(i, v)) => {
            let branch = &types[*i];
            if matches!(branch, Type::Primitive(Primitive::Null, _)) {
                out.push_str("null");
            } else {
                out.push('{');
                quote(out, &schema::full_name(named, branch));
                out.push(':');
                write_json(named, branch, v, out);
                out.push('}');
            }
        }
        (Type::Array(items, _), Datum::Array(values)) => {
            out.push('[');
            for (n, v) in values.iter().enumerate() {
                if n > 0 {
                    out.push(',');
                }
                write_json(named, items, v, out);
            }
            out.push(']');
        }
        (Type::Map(values_type, _), Datum::Map(entries)) => {
            out.push('{');
            for (n, (k, v)) in entries.iter().enumerate() {
                if n > 0 {
                    out.push(',');
                }
                quote(out, &java::utf8_string(k));
                out.push(':');
                write_json(named, values_type, v, out);
            }
            out.push('}');
        }
        _ => unreachable!("a datum read with its schema"),
    }
}
