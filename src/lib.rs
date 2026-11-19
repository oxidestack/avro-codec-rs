//! A port of the parts of Apache Avro 1.12.2 (Java) that turn JSON into Avro binary and back, byte for byte:
//!
//! * [`parse_schema`]: `new Schema.Parser().parse(json)` — Jackson's reader with comments allowed, the parser's
//!   `SchemaParseException`/`AvroTypeException`/`AvroRuntimeException` texts and the order it finds problems in,
//!   forward references resolved as `ParseContext` does, content after the schema refused; [`Schema`]'s `Display`
//!   is `Schema.toString()`.
//! * [`json_to_avro`]: a JSON document in Avro's JSON encoding read with `GenericDatumReader` over `JsonDecoder`
//!   (fields in any order, unknown fields skipped the way upstream skips them, union branches as
//!   `{"type": value}`), written with `GenericDatumWriter` to `BinaryEncoder`.
//! * [`avro_to_json`]: binary read with `GenericDatumReader` over `BinaryDecoder` (with `SystemLimitException`'s
//!   checks), written with `GenericDatumWriter` to `JsonEncoder`.
//!
//! Maps come out in `HashMap` iteration order, as upstream's `GenericData` builds them.
//!
//! Every failure is an [`Error`]: the Java exception's class and message.
//!
//! ```
//! # fn main() -> Result<(), avro::Error> {
//! let schema = avro::parse_schema(
//!     r#"{"type":"record","name":"Pet","fields":[{"name":"name","type":"string"},{"name":"age","type":"int"}]}"#,
//! )?;
//! let bytes = avro::json_to_avro(&schema, r#"{"name":"Rex","age":3}"#)?;
//! assert_eq!(bytes, [6, b'R', b'e', b'x', 6]);
//! assert_eq!(avro::avro_to_json(&schema, &bytes)?, r#"{"name":"Rex","age":3}"#);
//!
//! let error = avro::json_to_avro(&schema, r#"{"name":"Rex"}"#).unwrap_err();
//! println!("{error}"); // Avro's own exception text
//! # Ok(())
//! # }
//! ```

#![warn(missing_docs)]

mod binary;
mod codec;
pub mod error;
mod java;
mod jhashmap;
mod json_decoder;
mod parser;
mod schema;

pub use codec::MAX_ZERO_BYTE_COLLECTION_ALLOCATION;
pub use error::Error;
pub use schema::{Enum, Field, Fixed, Name, Named, Order, Primitive, Props, Record, Schema, Type};

/// How deeply values may nest in a datum read from binary (JSON is held to Jackson's own limit of 1000 by its
/// parser). Upstream has no limit: a deep enough recursive datum ends in a `StackOverflowError`.
pub const MAX_DEPTH: usize = 1000;

/// Options for [`parse_schema_with`].
#[derive(Clone, Copy, Debug, Default)]
pub struct ParseOptions<'a> {
    /// The prefix a shading build puts before relocated packages (`com.fasterxml.jackson`, `org.apache.avro`), as
    /// it shows in messages that print class or package names: a Jackson failure wrapped in
    /// `SchemaParseException`, `UnresolvedSchema_<n>`'s namespace and attribute, `Field.Order`'s constant, a
    /// `NullPointerException`'s method. Empty (the default) for Avro's own text.
    pub relocated_prefix: &'a str,
}

/// `new Schema.Parser().parse(json)`.
pub fn parse_schema(json: &str) -> Result<Schema, Error> {
    parse_schema_with(json, &ParseOptions::default())
}

/// `new Schema.Parser().parse(json)`, with [`ParseOptions`].
pub fn parse_schema_with(json: &str, options: &ParseOptions) -> Result<Schema, Error> {
    parser::parse(json, options)
}

/// A JSON document in Avro's JSON encoding as Avro binary: `GenericDatumReader` over
/// `DecoderFactory.jsonDecoder(schema, json)`, then `GenericDatumWriter` over a `BinaryEncoder`.
pub fn json_to_avro(schema: &Schema, json: &str) -> Result<Vec<u8>, Error> {
    let mut decoder = json_decoder::JsonDecoder::new(schema, json)?;
    let datum = codec::Reader::new(schema).read(&mut decoder)?;
    let mut out = binary::BinaryEncoder::default();
    codec::write_binary(&schema.named, &schema.root, &datum, &mut out);
    Ok(out.out)
}

/// Avro binary as JSON text in Avro's JSON encoding: `GenericDatumReader` over
/// `DecoderFactory.binaryDecoder(data, null)`, then `GenericDatumWriter` over a `JsonEncoder`. Bytes after the
/// datum are ignored.
pub fn avro_to_json(schema: &Schema, data: &[u8]) -> Result<String, Error> {
    let mut decoder = binary::BinaryDecoder::new(data);
    let datum = codec::Reader::new(schema).read(&mut decoder)?;
    let mut out = String::new();
    codec::write_json_datum(&schema.named, &schema.root, &datum, &mut out);
    Ok(out)
}
