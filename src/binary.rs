//! The binary encoding: `BinaryDecoder` over a byte array (with `SystemLimitException`'s checks) and
//! `BinaryEncoder`.

use crate::error::{self, Error};

/// `SystemLimitException.MAX_ARRAY_VM_LIMIT`.
pub const MAX_ARRAY_VM_LIMIT: i64 = i32::MAX as i64 - 8;

/// `SystemLimitException.checkMaxBytesLength` with the default limit.
pub(crate) fn check_max_bytes_length(length: i64) -> Result<usize, Error> {
    if length < 0 {
        return Err(Error::runtime(format!("Malformed data. Length is negative: {length}")));
    }
    if length > MAX_ARRAY_VM_LIMIT {
        return Err(Error::new(error::UNSUPPORTED_OPERATION, format!("Cannot read arrays longer than {MAX_ARRAY_VM_LIMIT} bytes in Java library")));
    }
    Ok(length as usize)
}

/// `SystemLimitException.checkMaxStringLength` with the default limit.
fn check_max_string_length(length: i64) -> Result<usize, Error> {
    if length < 0 {
        return Err(Error::runtime(format!("Malformed data. Length is negative: {length}")));
    }
    if length > MAX_ARRAY_VM_LIMIT {
        return Err(Error::new(error::UNSUPPORTED_OPERATION, format!("Cannot read strings longer than {MAX_ARRAY_VM_LIMIT} bytes")));
    }
    Ok(length as usize)
}

fn collections_too_large() -> Error {
    Error::new(error::UNSUPPORTED_OPERATION, format!("Cannot read collections larger than {MAX_ARRAY_VM_LIMIT} items in Java library"))
}

/// `SystemLimitException.checkMaxCollectionLength(items)`.
fn check_max_collection_length(items: i64) -> Result<i64, Error> {
    if items > MAX_ARRAY_VM_LIMIT {
        return Err(collections_too_large());
    }
    Ok(items)
}

/// `SystemLimitException.checkMaxCollectionLength(existing, items)`.
fn check_max_collection_length_more(existing: i64, items: i64) -> Result<i64, Error> {
    let length = existing.wrapping_add(items);
    if existing < 0 {
        return Err(Error::runtime(format!("Malformed data. Length is negative: {existing}")));
    }
    if items < 0 {
        return Err(Error::runtime(format!("Malformed data. Length is negative: {items}")));
    }
    if length > MAX_ARRAY_VM_LIMIT || length < existing {
        return Err(collections_too_large());
    }
    Ok(length)
}

/// `BinaryDecoder` over a byte array (`DecoderFactory.binaryDecoder(bytes, null)`).
pub struct BinaryDecoder<'a> {
    data: &'a [u8],
    pos: usize,
    /// `collectionCount`: one counter for every collection being read, as upstream's decoder keeps it.
    collection_count: i64,
}

impl<'a> BinaryDecoder<'a> {
    pub fn new(data: &'a [u8]) -> BinaryDecoder<'a> {
        BinaryDecoder { data, pos: 0, collection_count: 0 }
    }

    /// `remainingBytes()`.
    pub fn remaining_bytes(&self) -> usize {
        self.data.len() - self.pos
    }

    /// The byte at `pos + n`, zero past the end (`ensureBounds` pads the buffer; the length check comes after).
    fn byte(&self, n: usize) -> u8 {
        self.data.get(self.pos + n).copied().unwrap_or(0)
    }

    pub fn read_null(&mut self) -> Result<(), Error> {
        Ok(())
    }

    /// `readBoolean()`: only `1` is true.
    pub fn read_boolean(&mut self) -> Result<bool, Error> {
        let Some(&b) = self.data.get(self.pos) else { return Err(Error::eof()) };
        self.pos += 1;
        Ok(b == 1)
    }

    /// `readInt()`: up to five bytes; bits past 32 fall away.
    pub fn read_int(&mut self) -> Result<i32, Error> {
        if self.pos >= self.data.len() {
            return Err(Error::eof());
        }
        let mut len = 1;
        let mut b = self.byte(0) as u32;
        let mut n = b & 0x7f;
        let mut shift = 7;
        while b > 0x7f {
            if len == 5 {
                return Err(Error::new(error::INVALID_NUMBER_ENCODING, "Invalid int encoding"));
            }
            b = self.byte(len) as u32;
            n ^= (b & 0x7f).wrapping_shl(shift);
            shift += 7;
            len += 1;
        }
        self.pos += len;
        if self.pos > self.data.len() {
            return Err(Error::eof());
        }
        Ok(((n >> 1) as i32) ^ -((n & 1) as i32))
    }

    /// `readLong()`: up to ten bytes.
    pub fn read_long(&mut self) -> Result<i64, Error> {
        if self.pos >= self.data.len() {
            return Err(Error::eof());
        }
        let mut len = 1;
        let mut b = self.byte(0) as u64;
        let mut n = b & 0x7f;
        let mut shift = 7u32;
        while b > 0x7f {
            if len == 10 {
                return Err(Error::new(error::INVALID_NUMBER_ENCODING, "Invalid long encoding"));
            }
            b = self.byte(len) as u64;
            n ^= (b & 0x7f).wrapping_shl(shift);
            shift += 7;
            len += 1;
        }
        self.pos += len;
        if self.pos > self.data.len() {
            return Err(Error::eof());
        }
        Ok(((n >> 1) as i64) ^ -((n & 1) as i64))
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], Error> {
        if self.remaining_bytes() < n {
            self.pos = self.data.len();
            return Err(Error::eof());
        }
        let out = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Ok(out)
    }

    pub fn read_float(&mut self) -> Result<f32, Error> {
        if self.pos >= self.data.len() {
            return Err(Error::eof());
        }
        Ok(f32::from_le_bytes(self.take(4)?.try_into().expect("four bytes")))
    }

    pub fn read_double(&mut self) -> Result<f64, Error> {
        if self.pos >= self.data.len() {
            return Err(Error::eof());
        }
        Ok(f64::from_le_bytes(self.take(8)?.try_into().expect("eight bytes")))
    }

    /// `ensureAvailableBytes(length)`.
    fn ensure_available(&self, length: usize) -> Result<(), Error> {
        let remaining = self.remaining_bytes();
        if length > 0 && length > remaining {
            return Err(Error::new(error::EOF, format!("Attempted to read {length} bytes, but only {remaining} bytes are available")));
        }
        Ok(())
    }

    /// `readString(Utf8)`: the UTF-8 bytes as they are.
    pub fn read_string(&mut self) -> Result<Vec<u8>, Error> {
        let length = check_max_string_length(self.read_long()?)?;
        self.ensure_available(length)?;
        Ok(self.take(length)?.to_vec())
    }

    pub fn read_bytes(&mut self) -> Result<Vec<u8>, Error> {
        let length = check_max_bytes_length(self.read_long()?)?;
        self.ensure_available(length)?;
        Ok(self.take(length)?.to_vec())
    }

    pub fn read_fixed(&mut self, length: usize) -> Result<Vec<u8>, Error> {
        Ok(self.take(length)?.to_vec())
    }

    pub fn read_enum(&mut self) -> Result<i32, Error> {
        self.read_int()
    }

    pub fn read_index(&mut self) -> Result<i32, Error> {
        self.read_int()
    }

    /// `doReadItemCount()`: a negative count is followed by the block's byte size.
    fn read_item_count(&mut self) -> Result<i64, Error> {
        let mut result = self.read_long()?;
        if result < 0 {
            if result == i64::MIN {
                return Err(Error::runtime(format!("Malformed data. Block count is invalid: {result}")));
            }
            let bytecount = self.read_long()?;
            if bytecount < 0 {
                return Err(Error::runtime(format!("Malformed data. Block byte-size is negative: {bytecount}")));
            }
            result = -result;
        }
        Ok(result)
    }

    pub fn read_array_start(&mut self) -> Result<i64, Error> {
        self.collection_count = check_max_collection_length(self.read_item_count()?)?;
        Ok(self.collection_count)
    }

    pub fn array_next(&mut self) -> Result<i64, Error> {
        let length = self.read_item_count()?;
        self.collection_count = check_max_collection_length_more(self.collection_count, length)?;
        Ok(length)
    }

    pub fn read_map_start(&mut self) -> Result<i64, Error> {
        self.read_array_start()
    }

    pub fn map_next(&mut self) -> Result<i64, Error> {
        self.array_next()
    }
}

/// `BinaryEncoder` into a byte vector.
#[derive(Default)]
pub struct BinaryEncoder {
    pub out: Vec<u8>,
}

impl BinaryEncoder {
    pub fn write_long(&mut self, n: i64) {
        let mut v = ((n << 1) ^ (n >> 63)) as u64;
        while v >= 0x80 {
            self.out.push((v as u8 & 0x7f) | 0x80);
            v >>= 7;
        }
        self.out.push(v as u8);
    }

    pub fn write_int(&mut self, n: i32) {
        self.write_long(n as i64);
    }

    pub fn write_boolean(&mut self, b: bool) {
        self.out.push(b as u8);
    }

    pub fn write_float(&mut self, f: f32) {
        self.out.extend_from_slice(&f.to_bits().to_le_bytes());
    }

    pub fn write_double(&mut self, d: f64) {
        self.out.extend_from_slice(&d.to_bits().to_le_bytes());
    }

    /// `writeString` / `writeBytes`: the length, then the bytes.
    pub fn write_bytes(&mut self, bytes: &[u8]) {
        self.write_long(bytes.len() as i64);
        self.out.extend_from_slice(bytes);
    }

    pub fn write_fixed(&mut self, bytes: &[u8]) {
        self.out.extend_from_slice(bytes);
    }

    /// `setItemCount(n)` for a non-empty block, then (`writeArrayEnd`/`writeMapEnd`) the terminating zero.
    pub fn write_block_count(&mut self, n: usize) {
        if n > 0 {
            self.write_long(n as i64);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ints_truncate_as_java_does() {
        assert_eq!(BinaryDecoder::new(&[0xff, 0xff, 0xff, 0xff, 0x7f]).read_int().unwrap(), i32::MIN);
        assert_eq!(BinaryDecoder::new(&[0xff, 0xff, 0xff, 0xff, 0x0f]).read_int().unwrap(), i32::MIN);
        assert_eq!(BinaryDecoder::new(&[0xff, 0xff, 0xff, 0xff, 0xff]).read_int().unwrap_err().message.as_deref(), Some("Invalid int encoding"));
        assert_eq!(BinaryDecoder::new(&[0x80]).read_int().unwrap_err(), Error::eof());
        assert_eq!(BinaryDecoder::new(&[0xff; 10]).read_long().unwrap_err().message.as_deref(), Some("Invalid long encoding"));
        let mut max = vec![0xfe];
        max.extend([0xff; 8]);
        max.push(0x01);
        assert_eq!(BinaryDecoder::new(&max).read_long().unwrap(), i64::MAX);
    }
}

/// Avro 1.12.2's `TestBinaryDecoder`, `TestSystemLimitException`, `TestBinaryEncoderFidelity` and `TestBinaryData`,
/// ported where they reach the byte-array decoder and the encoder with the default limits (see
/// `docs/test-coverage.md`).
#[cfg(test)]
mod upstream_tests {
    use super::*;

    const ERROR_NEGATIVE: &str = "Malformed data. Length is negative: -1";

    fn vm_limit(what: &str) -> String {
        match what {
            "bytes" => format!("Cannot read arrays longer than {MAX_ARRAY_VM_LIMIT} bytes in Java library"),
            "string" => format!("Cannot read strings longer than {MAX_ARRAY_VM_LIMIT} bytes"),
            _ => format!("Cannot read collections larger than {MAX_ARRAY_VM_LIMIT} items in Java library"),
        }
    }

    /// `newDecoder(useDirect, long... values)`: the longs written one after the other.
    fn longs(values: &[i64]) -> Vec<u8> {
        let mut e = BinaryEncoder::default();
        for v in values {
            e.write_long(*v);
        }
        e.out
    }

    fn message(r: Result<impl std::fmt::Debug, Error>) -> (String, String) {
        let e = r.expect_err("an exception");
        (e.class.clone(), e.to_string())
    }

    #[test]
    fn eof_boolean_int_long_float_double_bytes_string_fixed_enum() {
        let d = || BinaryDecoder::new(&[]);
        assert_eq!(d().read_boolean().unwrap_err().class, error::EOF);
        assert_eq!(d().read_int().unwrap_err().class, error::EOF);
        assert_eq!(d().read_long().unwrap_err().class, error::EOF);
        assert_eq!(d().read_float().unwrap_err().class, error::EOF);
        assert_eq!(d().read_double().unwrap_err().class, error::EOF);
        assert_eq!(d().read_bytes().unwrap_err().class, error::EOF);
        assert_eq!(d().read_string().unwrap_err().class, error::EOF);
        assert_eq!(d().read_fixed(1).unwrap_err().class, error::EOF);
        assert_eq!(d().read_enum().unwrap_err().class, error::EOF);
    }

    #[test]
    fn bad_int_encoding() {
        assert_eq!(BinaryDecoder::new(&[0xff; 5]).read_int().unwrap_err().to_string(), "Invalid int encoding");
    }

    #[test]
    fn bad_long_encoding() {
        assert_eq!(BinaryDecoder::new(&[0xff; 10]).read_long().unwrap_err().to_string(), "Invalid long encoding");
    }

    #[test]
    fn test_string_negative_length() {
        assert_eq!(message(BinaryDecoder::new(&longs(&[-1])).read_string()), (error::AVRO_RUNTIME.into(), ERROR_NEGATIVE.into()));
    }

    #[test]
    fn test_string_vm_max_size() {
        assert_eq!(message(BinaryDecoder::new(&longs(&[MAX_ARRAY_VM_LIMIT + 1])).read_string()), (error::UNSUPPORTED_OPERATION.into(), vm_limit("string")));
    }

    #[test]
    fn test_string_length_exceeds_available_bytes() {
        let (class, text) = message(BinaryDecoder::new(&longs(&[10_000_000])).read_string());
        assert_eq!((class.as_str(), text.as_str()), (error::EOF, "Attempted to read 10000000 bytes, but only 0 bytes are available"));
    }

    #[test]
    fn test_bytes_length_exceeds_available_bytes() {
        assert_eq!(BinaryDecoder::new(&longs(&[10_000_000])).read_bytes().unwrap_err().class, error::EOF);
    }

    #[test]
    fn test_bytes_negative_length() {
        assert_eq!(message(BinaryDecoder::new(&longs(&[-1])).read_bytes()), (error::AVRO_RUNTIME.into(), ERROR_NEGATIVE.into()));
    }

    #[test]
    fn test_bytes_vm_max_size() {
        assert_eq!(message(BinaryDecoder::new(&longs(&[MAX_ARRAY_VM_LIMIT + 1])).read_bytes()), (error::UNSUPPORTED_OPERATION.into(), vm_limit("bytes")));
    }

    fn collection_limits(start: fn(&mut BinaryDecoder<'_>) -> Result<i64, Error>, next: fn(&mut BinaryDecoder<'_>) -> Result<i64, Error>, arrays: bool) {
        let too_many = (error::UNSUPPORTED_OPERATION.to_string(), vm_limit("collection"));
        assert_eq!(message(start(&mut BinaryDecoder::new(&longs(&[MAX_ARRAY_VM_LIMIT + 1])))), too_many);
        assert_eq!(message(next(&mut BinaryDecoder::new(&longs(&[MAX_ARRAY_VM_LIMIT + 1])))), too_many);
        if arrays {
            let data = longs(&[MAX_ARRAY_VM_LIMIT - 100, i64::MAX]);
            let mut bd = BinaryDecoder::new(&data);
            assert_eq!(start(&mut bd).unwrap(), MAX_ARRAY_VM_LIMIT - 100);
            assert_eq!(message(next(&mut bd)), too_many);
        }
        let data = longs(&[MAX_ARRAY_VM_LIMIT - 100, 100, 1]);
        let mut bd = BinaryDecoder::new(&data);
        assert_eq!(start(&mut bd).unwrap(), MAX_ARRAY_VM_LIMIT - 100);
        assert_eq!(next(&mut bd).unwrap(), 100);
        assert_eq!(message(next(&mut bd)), too_many);
        let data = longs(&[100 - MAX_ARRAY_VM_LIMIT, 999, -100, 999, 1]);
        let mut bd = BinaryDecoder::new(&data);
        assert_eq!(start(&mut bd).unwrap(), MAX_ARRAY_VM_LIMIT - 100);
        assert_eq!(next(&mut bd).unwrap(), 100);
        assert_eq!(message(next(&mut bd)), too_many);
    }

    #[test]
    fn test_array_vm_max_size() {
        collection_limits(|d| d.read_array_start(), |d| d.array_next(), true);
    }

    #[test]
    fn test_map_vm_max_size() {
        collection_limits(|d| d.read_map_start(), |d| d.map_next(), false);
    }

    #[test]
    fn long_length_encoding() {
        let (class, _) = message(BinaryDecoder::new(&[0x80, 0x80, 0x80, 0x80, 16]).read_string());
        assert_eq!(class, error::UNSUPPORTED_OPERATION);
    }

    #[test]
    fn int_too_short() {
        assert_eq!(BinaryDecoder::new(&[0xff; 4]).read_int().unwrap_err().class, error::EOF);
    }

    #[test]
    fn long_too_short() {
        assert_eq!(BinaryDecoder::new(&[0xff; 9]).read_long().unwrap_err().class, error::EOF);
    }

    #[test]
    fn float_too_short() {
        assert_eq!(BinaryDecoder::new(&[0xff; 3]).read_int().unwrap_err().class, error::EOF);
        assert_eq!(BinaryDecoder::new(&[0xff; 3]).read_float().unwrap_err().class, error::EOF);
    }

    #[test]
    fn double_too_short() {
        assert_eq!(BinaryDecoder::new(&[0xff; 7]).read_long().unwrap_err().class, error::EOF);
        assert_eq!(BinaryDecoder::new(&[0xff; 7]).read_double().unwrap_err().class, error::EOF);
    }

    #[test]
    fn eof() {
        let data = longs(&[0x10000000000000]);
        let mut d = BinaryDecoder::new(&data);
        assert_eq!(d.read_long().unwrap(), 0x10000000000000);
        assert_eq!(d.read_int().unwrap_err().class, error::EOF);
    }

    // ---- TestSystemLimitException (default limits) ----

    fn help_check_system_limits(f: fn(i64) -> Result<usize, Error>, error_vm_limit: String) {
        assert_eq!(f(0).unwrap(), 0);
        assert_eq!(f(1024).unwrap(), 1024);
        assert_eq!(f(MAX_ARRAY_VM_LIMIT).unwrap(), MAX_ARRAY_VM_LIMIT as usize);
        assert_eq!(message(f(i64::MAX)), (error::UNSUPPORTED_OPERATION.into(), error_vm_limit.clone()));
        assert_eq!(message(f(MAX_ARRAY_VM_LIMIT + 1)), (error::UNSUPPORTED_OPERATION.into(), error_vm_limit));
        assert_eq!(message(f(-1)), (error::AVRO_RUNTIME.into(), ERROR_NEGATIVE.into()));
    }

    #[test]
    fn test_check_max_bytes_length() {
        help_check_system_limits(check_max_bytes_length, vm_limit("bytes"));
    }

    #[test]
    fn test_check_max_collection_length_from_zero() {
        help_check_system_limits(|l| check_max_collection_length_more(0, l).map(|n| n as usize), vm_limit("collection"));
    }

    #[test]
    fn test_check_max_string_length() {
        help_check_system_limits(check_max_string_length, vm_limit("string"));
    }

    #[test]
    fn test_check_max_collection_length_from_non_zero() {
        let too_many = (error::UNSUPPORTED_OPERATION.to_string(), vm_limit("collection"));
        assert_eq!(check_max_collection_length_more(10, 0).unwrap(), 10);
        assert_eq!(check_max_collection_length_more(10, MAX_ARRAY_VM_LIMIT - 10).unwrap(), MAX_ARRAY_VM_LIMIT);
        assert_eq!(check_max_collection_length_more(MAX_ARRAY_VM_LIMIT - 10, 10).unwrap(), MAX_ARRAY_VM_LIMIT);
        for (a, b) in [(10, MAX_ARRAY_VM_LIMIT - 9), (MAX_ARRAY_VM_LIMIT - 9, 10), (10, i64::MAX - 10), (i64::MAX - 10, 10), (10, i64::MAX), (i64::MAX, 10)] {
            assert_eq!(message(check_max_collection_length_more(a, b)), too_many, "{a} {b}");
        }
        assert_eq!(message(check_max_collection_length_more(10, -1)), (error::AVRO_RUNTIME.into(), ERROR_NEGATIVE.into()));
        assert_eq!(message(check_max_collection_length_more(-1, 10)), (error::AVRO_RUNTIME.into(), ERROR_NEGATIVE.into()));
    }

    // ---- TestBinaryEncoderFidelity (the deterministic values of `generateData`, `generateComplexData`) ----

    /// `LegacyBinaryEncoder.encodeLong`.
    fn legacy_long(mut n: i64, o: &mut Vec<u8>) {
        n = (n << 1) ^ (n >> 63);
        while n & !0x7f != 0 {
            o.push(((n & 0x7f) | 0x80) as u8);
            n = ((n as u64) >> 7) as i64;
        }
        o.push(n as u8);
    }

    #[test]
    fn binary_encoder() {
        let mut e = BinaryEncoder::default();
        let mut legacy = Vec::new();
        e.write_boolean(true);
        e.write_boolean(false);
        legacy.extend([1, 0]);
        let bytes: Vec<u8> = (0..10).collect();
        e.write_bytes(&bytes);
        legacy_long(10, &mut legacy);
        legacy.extend(&bytes);
        e.write_bytes(&[]);
        legacy_long(0, &mut legacy);
        for d in [0.0, -0.0, f64::NAN, f64::NEG_INFINITY] {
            e.write_double(d);
            legacy.extend(d.to_bits().to_le_bytes());
        }
        e.write_int(65);
        legacy_long(65, &mut legacy);
        e.write_fixed(&bytes[7..9]);
        legacy.extend(&bytes[7..9]);
        for f in [1.0f32, f32::INFINITY, f32::MIN_POSITIVE] {
            e.write_float(f);
            legacy.extend(f.to_bits().to_le_bytes());
        }
        for n in [-2, 0, -1, 1, 0x40, -0x41, 0x2000, -0x2001, 0x80000, -0x80001, 0x4000000, -0x4000001, i32::MAX, i32::MIN] {
            e.write_int(n);
            legacy_long(n as i64, &mut legacy);
        }
        for n in [
            0,
            -1,
            1,
            0x40,
            -0x41,
            0x2000,
            -0x2001,
            0x80000,
            -0x80001,
            0x4000000,
            -0x4000001,
            0x200000000,
            -0x200000001,
            0x10000000000,
            -0x10000000001,
            0x800000000000,
            -0x800000000001,
            0x40000000000000,
            -0x40000000000001,
            0x2000000000000000,
            -0x2000000000000001,
            i64::MAX,
            i64::MIN,
        ] {
            e.write_long(n);
            legacy_long(n, &mut legacy);
        }
        for s in ["StringBuilder\u{A2}", "String\u{20AC}", "", "Utf8\u{1D11E}"] {
            e.write_bytes(s.as_bytes());
            legacy_long(s.len() as i64, &mut legacy);
            legacy.extend(s.as_bytes());
        }
        assert_eq!(e.out, legacy);
        // `generateComplexData`: an array of one int, a map of two entries.
        let mut e = BinaryEncoder::default();
        e.write_block_count(1);
        e.write_int(1);
        e.write_long(0);
        e.write_block_count(2);
        e.write_bytes(b"foo");
        e.write_int(-1);
        e.write_double(33.3);
        e.write_bytes(b"bar");
        e.write_int(1);
        e.write_double(-33.3);
        e.write_long(0);
        let mut legacy = Vec::new();
        legacy_long(1, &mut legacy);
        legacy_long(1, &mut legacy);
        legacy_long(0, &mut legacy);
        legacy_long(2, &mut legacy);
        for (k, n, d) in [("foo", -1, 33.3f64), ("bar", 1, -33.3)] {
            legacy_long(3, &mut legacy);
            legacy.extend(k.as_bytes());
            legacy_long(n, &mut legacy);
            legacy.extend(d.to_bits().to_le_bytes());
        }
        legacy_long(0, &mut legacy);
        assert_eq!(e.out, legacy);
    }

    // ---- TestBinaryData ----

    #[test]
    fn test_int_long_vle_equality() {
        for n in [0, 42, -24, i32::MAX, i32::MIN] {
            let (mut a, mut b) = (BinaryEncoder::default(), BinaryEncoder::default());
            a.write_int(n);
            b.write_long(n as i64);
            assert_eq!(a.out, b.out, "{n}");
        }
    }
}

