//! Avro 1.12.2's decoder and datum tests, ported where they reach `JsonDecoder`, `BinaryDecoder` and
//! `GenericDatumReader`/`GenericDatumWriter` with one schema (`TestJsonDecoder`, `TestEncoders`,
//! `TestGenericDatumReader`, `TestGenericDatumWriter`). Upstream reads into `GenericRecord`s and compares fields;
//! the ports compare the binary encoding and the JSON read back. `docs/test-coverage.md` lists each test.

use avro::Schema;

fn parse(json: &str) -> Schema {
    avro::parse_schema(json).unwrap_or_else(|e| panic!("{json}: {}", e.java_to_string()))
}

/// JSON → binary → JSON.
fn round_trip(schema: &Schema, json: &str) -> String {
    let bytes = avro::json_to_avro(schema, json).unwrap_or_else(|e| panic!("{json}: {}", e.java_to_string()));
    avro::avro_to_json(schema, &bytes).unwrap_or_else(|e| panic!("{json}: {}", e.java_to_string()))
}

fn json_error(schema: &Schema, json: &str) -> avro::Error {
    avro::json_to_avro(schema, json).expect_err(json)
}

fn binary_error(schema: &Schema, data: &[u8]) -> avro::Error {
    avro::avro_to_json(schema, data).expect_err("an exception")
}

/// `encodeVarints(long...)`.
fn varints(values: &[i64]) -> Vec<u8> {
    let mut out = Vec::new();
    for &n in values {
        let mut v = ((n << 1) ^ (n >> 63)) as u64;
        while v >= 0x80 {
            out.push((v as u8 & 0x7f) | 0x80);
            v >>= 7;
        }
        out.push(v as u8);
    }
    out
}

const AVRO_TYPE: &str = "org.apache.avro.AvroTypeException";
const EOF: &str = "java.io.EOFException";
const SYSTEM_LIMIT: &str = "org.apache.avro.SystemLimitException";

// ---- TestJsonDecoder ----

fn check_numeric(type_name: &str, want: &str) {
    let schema = parse(&format!(r#"{{"type":"record","name":"X","fields":[{{"type":"{type_name}","name":"n"}}]}}"#));
    for record in [r#"{"n":1}"#, r#"{"n":1.0}"#] {
        assert_eq!(round_trip(&schema, record), format!(r#"{{"n":{want}}}"#), "{type_name} {record}");
    }
}

#[test]
fn test_int() {
    check_numeric("int", "1");
}

#[test]
fn test_long() {
    check_numeric("long", "1");
}

#[test]
fn test_float() {
    check_numeric("float", "1.0");
}

#[test]
fn test_double() {
    check_numeric("double", "1.0");
}

#[test]
fn reorder_fields() {
    let schema = parse(r#"{"type":"record","name":"R","fields":[{"type":"long","name":"l"},{"type":{"type":"array","items":"int"},"name":"a"}]}"#);
    // One datum at a time here: the first document, then the second.
    assert_eq!(round_trip(&schema, r#"{"a":[1,2],"l":100}"#), r#"{"l":100,"a":[1,2]}"#);
    assert_eq!(round_trip(&schema, r#"{"l": 200, "a":[1,2]}"#), r#"{"l":200,"a":[1,2]}"#);
}

#[test]
fn test_int_with_error() {
    let schema = parse(r#"{"type":"record","name":"example","namespace":"test","fields":[{"name":"id","type":"int"}]}"#);
    let e = json_error(&schema, r#"{ "id": -1.2 }"#);
    assert_eq!(e.class, AVRO_TYPE);
    assert_eq!(e.to_string(), "Expected int. Got VALUE_NUMBER_FLOAT");
}

#[test]
fn test_ieee754_special_cases() {
    let schema = parse(
        r#"{"type":"record","name":"X","fields": [{"type":"float","name":"nanFloat"},{"type":"float","name":"infinityFloat"},{"type":"float","name":"negativeInfinityFloat"},{"type":"double","name":"nanDouble"},{"type":"double","name":"infinityDouble"},{"type":"double","name":"negativeInfinityDouble"}]}"#,
    );
    let record = r#"{"nanFloat":"NaN", "infinityFloat":"Infinity", "negativeInfinityFloat":"-Infinity", "nanDouble":"NaN", "infinityDouble":"Infinity", "negativeInfinityDouble":"-Infinity"}"#;
    assert_eq!(
        round_trip(&schema, record),
        r#"{"nanFloat":"NaN","infinityFloat":"Infinity","negativeInfinityFloat":"-Infinity","nanDouble":"NaN","infinityDouble":"Infinity","negativeInfinityDouble":"-Infinity"}"#
    );
}

// ---- TestEncoders (JSON) ----

const AB: &str = r#"{"type": "record", "name": "ab", "fields": [{"name": "a", "type": "int"}, {"name": "b", "type": ["null", "string"]}]}"#;

#[test]
fn json_encoder_when_include_namespace_option_is_true() {
    assert_eq!(round_trip(&parse(AB), r#"{"b": {"string":"myVal"}, "a": 1}"#), r#"{"a":1,"b":{"string":"myVal"}}"#);
}

#[test]
fn json_record_ordering() {
    let schema = parse(r#"{"type": "record", "name": "ab", "fields": [{"name": "a", "type": "int"}, {"name": "b", "type": "int"}]}"#);
    assert_eq!(round_trip(&schema, r#"{"b": 2, "a": 1}"#), r#"{"a":1,"b":2}"#);
}

const AB_NESTED: &str = r#"{"type": "record", "name": "ab", "fields": [
{"name": "a", "type": {"type":"record","name":"A","fields":
[{"name":"a1", "type":"null"}, {"name":"a2", "type":"boolean"}]}},
{"name": "b", "type": {"type":"record","name":"B","fields":
[{"name":"b1", "type":"string"}, {"name":"b2", "type":"float"}, {"name":"b3", "type":"double"}]}}
]}"#;

#[test]
fn json_excess_fields() {
    let e = json_error(&parse(AB_NESTED), r#"{"b": { "b3": 1.4, "b2": 3.14, "b1": "h"}, "a": {"a0": 45, "a2":true, "a1": null}}"#);
    assert_eq!(e.class, AVRO_TYPE);
    assert_eq!(e.to_string(), "Expected Unknown fields: [a0]. Got END_OBJECT");
}

#[test]
fn json_record_ordering2() {
    let got = round_trip(&parse(AB_NESTED), r#"{"b": { "b3": 1.4, "b2": 3.14, "b1": "h"}, "a": {"a2":true, "a1": null}}"#);
    assert_eq!(got, r#"{"a":{"a1":null,"a2":true},"b":{"b1":"h","b2":3.14,"b3":1.4}}"#);
}

#[test]
fn array_backed_byte_buffer() {
    // 17 bytes 0..16 as a `bytes` value: the length, then the bytes.
    let schema = parse(r#""bytes""#);
    let json = format!("\"{}\"", (0u8..17).map(|b| format!("\\u{:04x}", b)).collect::<String>());
    let bytes = avro::json_to_avro(&schema, &json).unwrap();
    let mut want = vec![34u8];
    want.extend(0u8..17);
    assert_eq!(bytes, want);
}

// ---- TestGenericDatumReader ----

/// `minBytesPerElement`, as the byte check a collection's block count meets: `N` elements of at least `min`
/// bytes each against none available.
fn claimed_min_bytes(item: &str) -> Option<i64> {
    let schema = parse(&format!(r#"{{"type":"array","items":{item}}}"#));
    // Zero-byte elements pass the check and are read; the block's end is then missing.
    let e = avro::avro_to_json(&schema, &varints(&[3])).expect_err("truncated");
    match e.to_string().strip_prefix("Collection claims 3 elements with at least ") {
        Some(rest) => rest.split(' ').next()?.parse().ok(),
        None => (e.class == EOF).then_some(0),
    }
}

#[test]
fn test_min_bytes_per_element_primitives() {
    assert_eq!(claimed_min_bytes(r#""null""#), Some(0));
    for (t, n) in [("boolean", 1), ("int", 1), ("long", 1), ("float", 4), ("double", 8), ("string", 1), ("bytes", 1)] {
        assert_eq!(claimed_min_bytes(&format!("\"{t}\"")), Some(n), "{t}");
    }
}

#[test]
fn test_min_bytes_per_element_fixed() {
    assert_eq!(claimed_min_bytes(r#"{"type":"fixed","name":"ZeroFixed","namespace":"test","size":0}"#), Some(0));
    assert_eq!(claimed_min_bytes(r#"{"type":"fixed","name":"FiveFixed","namespace":"test","size":5}"#), Some(5));
    assert_eq!(claimed_min_bytes(r#"{"type":"fixed","name":"SixteenFixed","namespace":"test","size":16}"#), Some(16));
}

#[test]
fn test_min_bytes_per_element_union() {
    assert_eq!(claimed_min_bytes(r#"["null","int"]"#), Some(1));
}

#[test]
fn test_min_bytes_per_element_record() {
    assert_eq!(claimed_min_bytes(r#"{"type":"record","name":"Empty","namespace":"test","fields":[]}"#), Some(0));
    assert_eq!(claimed_min_bytes(r#"{"type":"record","name":"WithInt","namespace":"test","fields":[{"name":"x","type":"int"}]}"#), Some(1));
    assert_eq!(claimed_min_bytes(r#"{"type":"record","name":"WithNull","namespace":"test","fields":[{"name":"n","type":"null"}]}"#), Some(0));
    assert_eq!(
        claimed_min_bytes(r#"{"type":"record","name":"WithMultipleFields","namespace":"test","fields":[{"name":"f","type":"float"},{"name":"d","type":"double"}]}"#),
        Some(12)
    );
    // A self-reference counts nothing the second time round.
    assert_eq!(claimed_min_bytes(r#"{"type":"record","name":"L","fields":[{"name":"d","type":"double"},{"name":"next","type":{"type":"array","items":"L"}}]}"#), Some(9));
}

#[test]
fn test_min_bytes_per_element_nested_collections() {
    assert_eq!(claimed_min_bytes(r#"{"type":"array","items":"int"}"#), Some(1));
    assert_eq!(claimed_min_bytes(r#"{"type":"map","values":"int"}"#), Some(1));
}

#[test]
fn array_of_ints_rejects_huge_count() {
    let e = binary_error(&parse(r#"{"type":"array","items":"int"}"#), &varints(&[10_000_000, 0]));
    assert_eq!(e.class, EOF);
    assert_eq!(e.to_string(), "Collection claims 10000000 elements with at least 1 bytes each, but only 1 bytes are available");
}

#[test]
fn array_of_nulls_accepts_large_count() {
    let json = avro::avro_to_json(&parse(r#"{"type":"array","items":"null"}"#), &varints(&[1000, 0])).unwrap();
    assert_eq!(json.matches("null").count(), 1000);
}

#[test]
fn map_of_string_to_int_rejects_huge_count() {
    // The fast reader's maps are not checked against the bytes left: the keys run out.
    let e = binary_error(&parse(r#"{"type":"map","values":"int"}"#), &varints(&[10_000_000, 0]));
    assert_eq!(e.class, EOF);
}

#[test]
fn map_of_string_to_null_rejects_huge_count() {
    let e = binary_error(&parse(r#"{"type":"map","values":"null"}"#), &varints(&[10_000_000, 0]));
    assert_eq!(e.class, EOF);
}

#[test]
fn array_of_zero_length_fixed_accepts_large_count() {
    let json = avro::avro_to_json(&parse(r#"{"type":"array","items":{"type":"fixed","name":"Empty","namespace":"test","size":0}}"#), &varints(&[500, 0])).unwrap();
    assert_eq!(json.matches("\"\"").count(), 500);
}

#[test]
fn array_of_records_rejects_huge_count_using_full_record_size() {
    let e = binary_error(&parse(r#"{"type":"array","items":{"type":"record","name":"Element","namespace":"test","fields":[{"name":"f","type":"float"},{"name":"d","type":"double"}]}}"#), &varints(&[2, 0]));
    assert_eq!(e.class, EOF);
    assert_eq!(e.to_string(), "Collection claims 2 elements with at least 12 bytes each, but only 1 bytes are available");
}

#[test]
fn map_of_records_rejects_huge_count_using_full_record_size() {
    let e = binary_error(&parse(r#"{"type":"map","values":{"type":"record","name":"MapValue","namespace":"test","fields":[{"name":"f","type":"float"},{"name":"d","type":"double"}]}}"#), &varints(&[1, 0]));
    assert_eq!(e.class, EOF);
}

/// The zero-byte allocation cap is [`avro::MAX_ZERO_BYTE_COLLECTION_ALLOCATION`] here, upstream's tests set it to
/// 1000 with a system property.
const CAP: i64 = avro::MAX_ZERO_BYTE_COLLECTION_ALLOCATION;

#[test]
fn array_of_nulls_rejects_count_above_allocation_limit() {
    let schema = parse(r#"{"type":"array","items":"null"}"#);
    assert_eq!(binary_error(&schema, &varints(&[CAP + 1, 0])).class, SYSTEM_LIMIT);
    assert_eq!(binary_error(&schema, &varints(&[CAP / 2 + 1, CAP / 2, 0])).class, SYSTEM_LIMIT);
}

#[test]
fn array_of_nulls_within_allocation_limit_still_decodes() {
    let json = avro::avro_to_json(&parse(r#"{"type":"array","items":"null"}"#), &varints(&[1000, 0])).unwrap();
    assert_eq!(json.matches("null").count(), 1000);
}

#[test]
fn array_of_all_null_records_rejects_count_above_allocation_limit() {
    let schema = parse(r#"{"type":"array","items":{"type":"record","name":"AllNull","namespace":"test","fields":[{"name":"n","type":"null"}]}}"#);
    assert_eq!(binary_error(&schema, &varints(&[CAP + 1, 0])).class, SYSTEM_LIMIT);
}

const TWO_NULL_ARRAY_FIELDS_SCHEMA: &str = r#"{"type":"record","name":"R","fields":[{"name":"a","type":{"type":"array","items":"null"}},{"name":"b","type":{"type":"array","items":"null"}}]}"#;

#[test]
fn record_of_null_array_fields_rejected_cumulatively_across_datum() {
    let e = binary_error(&parse(TWO_NULL_ARRAY_FIELDS_SCHEMA), &varints(&[CAP / 2 + 1, 0, CAP / 2 + 1, 0]));
    assert_eq!(e.class, SYSTEM_LIMIT);
}

#[test]
fn record_of_null_array_fields_within_cumulative_limit_still_decodes() {
    let schema = parse(TWO_NULL_ARRAY_FIELDS_SCHEMA);
    for _ in 0..2 {
        let json = avro::avro_to_json(&schema, &varints(&[400, 0, 400, 0])).unwrap();
        assert_eq!(json.matches("null").count(), 800);
    }
}

#[test]
fn huge_collections_rejected_on_both_reader_paths() {
    for (schema, class) in [
        (r#"{"type":"array","items":"null"}"#, SYSTEM_LIMIT),
        (r#"{"type":"array","items":"long"}"#, EOF),
        (r#"{"type":"array","items":"int"}"#, EOF),
        (r#"{"type":"map","values":"null"}"#, EOF),
        (r#"{"type":"map","values":"long"}"#, EOF),
    ] {
        assert_eq!(binary_error(&parse(schema), &varints(&[200_000_000, 0])).class, class, "{schema}");
    }
}

#[test]
fn fast_and_classic_reader_reject_cumulative_null_blocks() {
    let e = binary_error(&parse(r#"{"type":"array","items":"null"}"#), &varints(&[CAP / 2 + 1, CAP / 2 + 1, 0]));
    assert_eq!(e.class, SYSTEM_LIMIT);
}

#[test]
fn fast_and_classic_reader_reject_negative_null_block_count() {
    let e = binary_error(&parse(r#"{"type":"array","items":"null"}"#), &varints(&[-(CAP + 1), 0, 0]));
    assert_eq!(e.class, SYSTEM_LIMIT);
}

#[test]
fn fast_and_classic_reader_reject_min_value_block_count() {
    let e = binary_error(&parse(r#"{"type":"array","items":"null"}"#), &varints(&[i64::MIN, 0, 0]));
    assert_eq!(e.class, "org.apache.avro.AvroRuntimeException");
    assert_eq!(e.to_string(), "Malformed data. Block count is invalid: -9223372036854775808");
}

#[test]
fn fast_and_classic_reader_reject_out_of_range_union_index() {
    let schema = parse(r#"["null","long"]"#);
    for index in [5, -1] {
        let e = binary_error(&schema, &varints(&[index]));
        assert_eq!(e.class, AVRO_TYPE);
        assert_eq!(e.to_string(), format!("Union branch index out of range: must be in [0, 2), but received {index}"));
    }
}

#[test]
fn fast_and_classic_reader_reject_out_of_range_enum_index() {
    let schema = parse(r#"{"type":"enum","name":"E","symbols":["A","B"]}"#);
    for index in [9, -1] {
        let e = binary_error(&schema, &varints(&[index]));
        assert_eq!(e.class, AVRO_TYPE);
        assert_eq!(e.to_string(), format!("Enumeration out of range: must be in [0, 2), but received {index}"));
    }
}

#[test]
fn fast_and_classic_reader_decode_small_null_array() {
    let json = avro::avro_to_json(&parse(r#"{"type":"array","items":"null"}"#), &varints(&[1000, 0])).unwrap();
    assert_eq!(json.matches("null").count(), 1000);
}

// ---- TestGenericDatumWriter ----

#[test]
fn write() {
    let schema = parse(r#"{"type": "record", "name": "r", "fields": [{ "name": "f1", "type": "long" }]}"#);
    let bytes = avro::json_to_avro(&schema, r#"{"f1": 100}"#).unwrap();
    assert_eq!(bytes, [0xc8, 0x01]);
    assert_eq!(avro::avro_to_json(&schema, &bytes).unwrap(), r#"{"f1":100}"#);
}

#[test]
fn allow_writing_primitives() {
    for (schema, json, hex) in [(r#""int""#, "3", "06"), (r#""string""#, r#""s""#, "0273"), (r#""boolean""#, "true", "01"), (r#""null""#, "null", "")] {
        let bytes = avro::json_to_avro(&parse(schema), json).unwrap();
        assert_eq!(bytes.iter().map(|b| format!("{b:02x}")).collect::<String>(), hex, "{schema}");
    }
}
