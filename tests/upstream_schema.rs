//! Avro 1.12.2's parser tests, ported where they reach `Schema.Parser` (`TestSchema`, `TestSchemaParser`,
//! `TestFixed`, `TestUnionSelfReference`, `SchemaNameValidatorTest`, `ParseContextTest`, `TestSchemaResolver`,
//! `TestNestedRecords`). Upstream builds many of its schemas with `SchemaBuilder` or `Schema.create*`; the ports
//! give the same schemas as JSON. `docs/test-coverage.md` lists each test.

use avro::{Named, Order, Primitive, Schema, Type};
use jackson::JsonNode;

fn parse(json: &str) -> Schema {
    avro::parse_schema(json).unwrap_or_else(|e| panic!("{json}: {}", e.java_to_string()))
}

fn fail(json: &str) -> avro::Error {
    match avro::parse_schema(json) {
        Ok(s) => panic!("{json} parsed as {s}"),
        Err(e) => e,
    }
}

fn node(json: &str) -> JsonNode {
    jackson::json::read::read_tree(json, Default::default()).expect("JSON")
}

const SCHEMA_PARSE: &str = "org.apache.avro.SchemaParseException";
const AVRO_TYPE: &str = "org.apache.avro.AvroTypeException";
const AVRO_RUNTIME: &str = "org.apache.avro.AvroRuntimeException";

// ---- TestSchema ----

/// `splitSchemaBuild`: the schema `SchemaBuilder` gives, read back from its text.
#[test]
fn split_schema_build() {
    let text = r#"{"type":"record","name":"HandshakeRequest","namespace":"org.apache.avro.ipc","fields":[{"name":"clientProtocol","type":["null","string"],"default":null},{"name":"meta","type":["null",{"type":"map","values":"bytes"}],"default":null}]}"#;
    let (a, b) = text.split_at(text.len() / 2);
    assert_eq!(parse(text).to_string(), parse(&format!("{a}{b}")).to_string());
    assert_eq!(parse(text).to_string(), text);
}

#[test]
fn default_record_with_duplicate_field_name() {
    let e = fail(r#"{"type":"record","name":"name","namespace":"namespace","doc":"doc","fields":[{"name":"field_name","type":"null"},{"name":"field_name","type":"int"}]}"#);
    assert_eq!(e.class, AVRO_RUNTIME);
    assert!(e.to_string().contains("Duplicate field field_name in record name"), "{e}");
    assert_eq!(e.to_string(), "Duplicate field field_name in record namespace.name: field_name type:INT pos:1 and field_name type:NULL pos:0.");
}

#[test]
fn record_with_null_doc() {
    assert_eq!(parse(r#"{"type":"record","name":"name","namespace":"namespace","fields":[]}"#).to_string(), r#"{"type":"record","name":"name","namespace":"namespace","fields":[]}"#);
}

#[test]
fn record_with_null_namespace() {
    assert_eq!(parse(r#"{"type":"record","name":"name","doc":"doc","fields":[]}"#).to_string(), r#"{"type":"record","name":"name","doc":"doc","fields":[]}"#);
}

#[test]
fn empty_record_schema() {
    let s = parse(r#"{"type":"record","name":"name","namespace":"namespace","doc":"doc","fields":[]}"#);
    assert_eq!(s.to_string(), r#"{"type":"record","name":"name","namespace":"namespace","doc":"doc","fields":[]}"#);
}

#[test]
fn parse_empty_schema() {
    let e = fail("");
    assert_eq!(e.class, SCHEMA_PARSE);
    assert_eq!(e.to_string(), "Cannot parse <null> schema");
}

#[test]
fn schema_with_fields() {
    let s = parse(r#"{"type":"record","name":"name","namespace":"namespace","doc":"doc","fields":[{"name":"field_name1","type":"null"},{"name":"field_name2","type":"int"}]}"#);
    assert_eq!(s.record_of(&s.root).unwrap().fields.len(), 2);
}

#[test]
fn schema_with_null_fields() {
    // `createRecord(…, null)` has no JSON form; a record without `fields` is the parser's refusal.
    let e = fail(r#"{"type":"record","name":"name","namespace":"namespace","doc":"doc"}"#);
    assert_eq!(e.to_string(), r#"Record has no fields: {"type":"record","name":"name","namespace":"namespace","doc":"doc"}"#);
}

fn is_union(s: &Schema) -> bool {
    matches!(s.root, Type::Union(_))
}

/// `isNullable()`.
fn is_nullable(s: &Schema) -> bool {
    match &s.root {
        Type::Union(types) => types.iter().any(|t| matches!(t, Type::Primitive(Primitive::Null, _))),
        Type::Primitive(Primitive::Null, _) => true,
        _ => false,
    }
}

#[test]
fn is_union_on_union_with_multiple_elements() {
    assert!(is_union(&parse(r#"["null","long"]"#)));
}

#[test]
fn is_union_on_union_with_one_element() {
    assert!(is_union(&parse(r#"["long"]"#)));
}

#[test]
fn is_union_on_record() {
    assert!(!is_union(&parse(r#"{"type":"record","name":"name","fields":[]}"#)));
}

#[test]
fn is_union_on_array() {
    assert!(!is_union(&parse(r#"{"type":"array","items":"long"}"#)));
}

#[test]
fn is_union_on_enum() {
    assert!(!is_union(&parse(r#"{"type":"enum","name":"name","namespace":"namespace","doc":"doc","symbols":["value"]}"#)));
}

#[test]
fn is_union_on_fixed() {
    assert!(!is_union(&parse(r#"{"type":"fixed","name":"name","namespace":"space","doc":"doc","size":10}"#)));
}

#[test]
fn is_union_on_map() {
    assert!(!is_union(&parse(r#"{"type":"map","values":"long"}"#)));
}

#[test]
fn is_nullable_on_union_with_null() {
    assert!(is_nullable(&parse(r#"["null","long"]"#)));
}

#[test]
fn is_nullable_on_union_without_null() {
    assert!(!is_nullable(&parse(r#"["long"]"#)));
}

#[test]
fn is_nullable_on_record() {
    assert!(!is_nullable(&parse(r#"{"type":"record","name":"name","fields":[]}"#)));
}

/// `serialization`: `SchemaBuilder.avsc` read and compared with itself (Java serialization has no port).
#[test]
fn serialization() {
    let text = std::fs::read_to_string(format!("{}/upstream/lang/java/avro/src/test/resources/SchemaBuilder.avsc", env!("CARGO_MANIFEST_DIR"))).expect("the upstream submodule");
    let s = parse(&text);
    assert_eq!(s, parse(&s.to_string()));
}

#[test]
fn reconstruct_schema_string_without_inlined_child_reference() {
    // Two parses with one `SchemaParser` are one document here: the child defined first, referenced by name.
    let parent = r#"{"type":"record","name":"Parent","namespace":"org.apache.avro.nested","fields":[{"name":"child","type":{"type":"record","name":"Child","fields":[{"name":"childField","type":"string"}]}},{"name":"again","type":"Child"}]}"#;
    assert_eq!(parse(parent).to_string(), parent);
}

fn field_default(record: &str, type_json: &str, default: &str) -> Result<JsonNode, avro::Error> {
    let json = format!(r#"{{"type":"record","name":"{record}","fields":[{{"name":"myField","type":{type_json},"doc":"doc","default":{default}}}]}}"#);
    avro::parse_schema(&json).map(|s| s.record_of(&s.root).unwrap().fields[0].default.clone().unwrap())
}

#[test]
fn int_default_value() {
    for v in ["1", "-2147483648", "2147483647"] {
        assert_eq!(field_default("R", r#""int""#, v).unwrap().to_java_string(), v);
    }
}

#[test]
fn valid_long_as_int_default_value() {
    // A `Long` default that fits an `int` is written as an int node.
    for v in ["1", "-2147483648", "2147483647"] {
        assert!(field_default("R", r#""int""#, v).is_ok());
    }
}

#[test]
fn invalid_long_as_int_default_value() {
    let e = field_default("R", r#""int""#, "2147483648").unwrap_err();
    assert_eq!(e.class, AVRO_TYPE);
    assert_eq!(e.to_string(), r#"Invalid default for field myField: 2147483648 not a "int""#);
}

#[test]
fn double_as_int_default_value() {
    assert_eq!(field_default("R", r#""int""#, "1.0").unwrap_err().class, AVRO_TYPE);
}

#[test]
fn long_default_value() {
    for v in ["1", "-9223372036854775808", "9223372036854775807"] {
        assert_eq!(field_default("R", r#""long""#, v).unwrap().to_java_string(), v);
    }
}

#[test]
fn int_as_long_default_value() {
    assert!(field_default("R", r#""long""#, "1").is_ok());
}

#[test]
fn double_as_long_default_value() {
    assert_eq!(field_default("R", r#""long""#, "1.0").unwrap_err().class, AVRO_TYPE);
}

#[test]
fn double_default_value() {
    assert_eq!(field_default("R", r#""double""#, "1.0").unwrap().double_value(), 1.0);
}

#[test]
fn int_as_double_default_value() {
    assert_eq!(field_default("R", r#""double""#, "1").unwrap().double_value(), 1.0);
}

#[test]
fn long_as_double_default_value() {
    assert_eq!(field_default("R", r#""double""#, "4294967296").unwrap().double_value(), 4294967296.0);
}

#[test]
fn float_as_double_default_value() {
    assert_eq!(field_default("R", r#""double""#, "1.0").unwrap().double_value(), 1.0);
}

#[test]
fn float_default_value() {
    assert_eq!(field_default("R", r#""float""#, "1.0").unwrap().double_value(), 1.0);
}

#[test]
fn int_as_float_default_value() {
    assert_eq!(field_default("R", r#""float""#, "1").unwrap().double_value(), 1.0);
}

#[test]
fn long_as_float_default_value() {
    assert!(field_default("R", r#""float""#, "4294967296").is_ok());
}

#[test]
fn double_as_float_default_value() {
    assert!(field_default("R", r#""float""#, "1.0").is_ok());
}

#[test]
fn enum_symbol_as_null() {
    let e = fail(r#"{"type":"enum","name":"myField","namespace":"namespace","doc":"doc","symbols":[null]}"#);
    assert_eq!(e.class, SCHEMA_PARSE);
    assert_eq!(e.to_string(), "Null name");
}

#[test]
fn schema_field_without_schema() {
    let e = fail(r#"{"type":"record","name":"R","fields":[{"name":"f"}]}"#);
    assert_eq!(e.to_string(), r#"No field type: {"name":"f"}"#);
}

#[test]
fn parse_record_with_name_as_type() {
    let text = "{\n  \"type\" : \"record\",\n  \"name\" : \"ns.int\",\n  \"fields\" : [ \n    {\"name\" : \"value\", \"type\" : \"int\"}, \n    {\"name\" : \"next\", \"type\" : [ \"null\", \"ns.int\" ]}\n  ]\n}";
    let s = parse(text);
    // `ns.int` is written in full wherever it is referred to, even in its own namespace.
    assert_eq!(s.to_string(), r#"{"type":"record","name":"int","namespace":"ns","fields":[{"name":"value","type":"int"},{"name":"next","type":["null","ns.int"]}]}"#);
    assert_eq!(s, parse(&s.to_string()));
}

#[test]
fn qualified_name() {
    // `Name.getQualified`: seen through `toString()`, which writes references with it.
    for t in ["record", "enum", "array", "map", "union", "fixed", "string", "bytes", "int", "long", "float", "double", "boolean", "null"] {
        let s = parse(&format!(r#"{{"type":"record","name":"R","namespace":"space","fields":[{{"name":"a","type":{{"type":"fixed","name":"{t}","size":1}}}},{{"name":"b","type":"space.{t}"}}]}}"#));
        assert!(s.to_string().ends_with(&format!(r#"{{"name":"b","type":"space.{t}"}}]}}"#)), "{t}: {s}");
    }
    let s = parse(r#"{"type":"record","name":"R","namespace":"space","fields":[{"name":"a","type":{"type":"fixed","name":"name","size":1}},{"name":"b","type":"name"}]}"#);
    assert!(s.to_string().ends_with(r#"{"name":"b","type":"name"}]}"#));
    let s = parse(r#"{"type":"record","name":"R","namespace":"otherdefault","fields":[{"name":"a","type":{"type":"fixed","name":"space.name","size":1}},{"name":"b","type":"space.name"}]}"#);
    assert!(s.to_string().ends_with(r#"{"name":"b","type":"space.name"}]}"#));
    let s = parse(r#"{"type":"record","name":"R","namespace":"space","fields":[{"name":"a","type":{"type":"fixed","name":"Int","size":1}},{"name":"b","type":"Int"}]}"#);
    assert!(s.to_string().ends_with(r#"{"name":"b","type":"Int"}]}"#));
}

#[test]
fn valid_value() {
    let s = parse(r#""null""#);
    assert!(s.is_valid_default(&s.root, &node("null")));
    let s = parse(r#""int""#);
    assert!(s.is_valid_default(&s.root, &node("12")));
    let s = parse(r#""string""#);
    assert!(s.is_valid_default(&s.root, &node(r#""textNode""#)));
    let s = parse(r#"{"type":"array","items":"string"}"#);
    assert!(s.is_valid_default(&s.root, &node("[]")));
    assert!(s.is_valid_default(&s.root, &node(r#"["Hello","World"]"#)));
    assert!(!s.is_valid_default(&s.root, &node(r#"["Hello","World",5]"#)));
    let s = parse(r#"["string","int","null"]"#);
    assert!(s.is_valid_default(&s.root, &node(r#""Hello""#)));
    assert!(s.is_valid_default(&s.root, &node("23")));
    assert!(s.is_valid_default(&s.root, &node("null")));
    assert!(!s.is_valid_default(&s.root, &node(r#"["Hello","World",5]"#)));
    let s = parse(r#"{"type":"array","items":["string","int","null"]}"#);
    assert!(s.is_valid_default(&s.root, &node(r#"["Hello",null]"#)));
    let s = parse(r#"["string","bytes"]"#);
    assert!(s.is_valid_default(&s.root, &node(r#""Hello""#)));
    assert!(!s.is_valid_default(&s.root, &node("123")));
}

#[test]
fn enum_late_define() {
    let text = r#"{"type":"record","name":"Main","fields":[{"name":"f1","type":"Sub"},{"name":"f2","type":{"type":"enum","name":"Sub","symbols":["OPEN","CLOSE"]}}]}"#;
    let s = parse(text);
    let main = s.record_of(&s.root).unwrap();
    let (f1, f2) = (&main.field("f1").unwrap().schema, &main.field("f2").unwrap().schema);
    assert!(matches!((f1, f2), (Type::Named(a), Type::Named(b)) if a == b), "the same schema");
    assert_eq!(s.type_label(f1), "ENUM");
    let text = s.to_string();
    let definition = text.find(r#""symbols":["OPEN","CLOSE"]"#).unwrap();
    let usage = text.find(r#""type":"Sub""#).unwrap();
    assert!(definition < usage, "usage is before definition: {text}");
}

#[test]
fn record_in_array() {
    let text = r#"{"type":"record","name":"TestRecord","fields":[{"name":"value","type":{"type":"record","name":"Container","fields":[{"name":"Optional","type":{"type":"array","items":[{"type":"record","name":"optional_field_0","namespace":"","doc":"","fields":[{"name":"optional_field_1","type":"long","doc":"","default":0}]}]}}]}}]}"#;
    parse(text);
}

#[test]
fn union_field_type() {
    let s = parse(r#"{"type": "record", "name": "Lisp", "fields": [{"name":"value", "type":["null", "string",{"type": "record", "name": "Cons", "fields": [{"name":"car", "type":"Lisp"},{"name":"cdr", "type":"Lisp"}]}]}]}"#);
    let value = &s.record_of(&s.root).unwrap().field("value").unwrap().schema;
    let Type::Union(types) = value else { panic!("a union") };
    let cons = types.iter().find_map(|t| s.record_of(t)).expect("a record branch");
    assert_eq!(cons.fields.len(), 2);
}

#[test]
fn parse_aliases() {
    let s = parse(r#"{"type":"record","name":"R","fields":[{"name":"f","type":"int","aliases":["a1","b1"]}]}"#);
    assert_eq!(s.record_of(&s.root).unwrap().fields[0].aliases, ["a1", "b1"]);
    let e = fail(r#"{"type":"record","name":"R","fields":[{"name":"f","type":"int","aliases":{"a1":"b1"}}]}"#);
    assert_eq!(e.class, SCHEMA_PARSE);
    assert!(e.to_string().contains("aliases not an array"), "{e}");
    let e = fail(r#"{"type":"record","name":"R","fields":[{"name":"f","type":"int","aliases":[11,"b1"]}]}"#);
    assert_eq!(e.class, SCHEMA_PARSE);
    assert!(e.to_string().contains("alias not a string"), "{e}");
}

#[test]
fn test_content_after_avsc() {
    let e = fail(r#"{"type": "string"}; DROP TABLE STUDENTS"#);
    assert_eq!(e.class, SCHEMA_PARSE);
    assert_eq!(e.to_string(), "dangling content after end of schema: ; DROP TABLE STUDENTS");
}

#[test]
fn test_parser_null_validate() {
    // `new SchemaParser(null)` turns name validation off; `Schema.Parser()` (what is ported) keeps it on.
    let e = fail(r#"{"type":"record","name":"","fields":[]}"#);
    assert_eq!(e.to_string(), "Empty name");
}

#[test]
fn disallow_type_object_for_named_type() {
    let e = fail(r#"{"namespace":"tests","type":"record","name":"Invalid","fields":[{"name":"good","type":{"type":"fixed","name":"Hash","size":16}},{"name":"right","type":{"type":"Hash"}}]}"#);
    assert_eq!(e.class, SCHEMA_PARSE);
}

#[test]
fn test_parse_multiple_file() {
    // The three files, one document: `DocumentInfo` and `MyResponse` defined where `ApplicationEvent` uses them.
    let dir = "upstream/lang/java/avro/src/test/resources/multipleFile";
    let read = |f: &str| std::fs::read_to_string(format!("{}/{dir}/{f}", env!("CARGO_MANIFEST_DIR"))).expect("file");
    for f in ["DocumentInfo.avsc", "MyResponse.avsc"] {
        parse(&read(f));
    }
    let event = read("ApplicationEvent.avsc");
    let e = fail(&event);
    assert_eq!(e.to_string(), "Undefined schema: model.DocumentInfo");
}

#[test]
fn add_types() {
    let s = parse(r#"{"type":"record","name":"record2","fields":[{"name":"f0","type":{"type":"record","name":"record1","doc":"doc","fields":[{"name":"name","type":"string"}]}},{"name":"f1","type":"record1"}]}"#);
    let r = s.record_of(&s.root).unwrap();
    assert!(s.types_equal(&r.field("f0").unwrap().schema, &r.field("f1").unwrap().schema));
}

// ---- TestSchemaParser ----

const SCHEMA_REAL: &str = r#"{"type":"fixed","name":"Real","namespace":"tests","size":42}"#;

#[test]
fn test_static_parse_text() {
    let s = parse(SCHEMA_REAL);
    assert_eq!(s.to_string(), SCHEMA_REAL);
    let Some(Named::Fixed(f)) = s.named_of(&s.root) else { panic!("fixed") };
    assert_eq!((f.name.full(), f.size), ("tests.Real".to_string(), 42));
}

#[test]
fn test_parse_text_with_fallback_json_parser() {
    assert_eq!(parse(SCHEMA_REAL), parse(&format!("{SCHEMA_REAL}\n")));
}

#[test]
fn test_single_parse_error() {
    let e = fail("foo");
    assert_eq!(e.class, SCHEMA_PARSE);
    assert!(e.to_string().starts_with("com.fasterxml.jackson.core.JsonParseException: Unrecognized token 'foo'"), "{e}");
}

// ---- SchemaFormatterTest ----

#[test]
fn validate_support_for_inline_json_format() {
    assert_eq!(parse(r#"{"type":"fixed","name":"ns.Fixed","size":16}"#).to_string(), r#"{"type":"fixed","name":"Fixed","namespace":"ns","size":16}"#);
}

// ---- TestFixed ----

#[test]
fn fixed_default_value_drop() {
    let s = parse(r#"{"type":"record","name":"test","fields":[{"name":"hash","type":{"type":"fixed","name":"MD5","size":16},"default":"\u0000\u0000\u0000\u0000\u0000\u0000\u0000\u0000\u0000\u0000\u0000\u0000\u0000\u0000\u0000\u0000"}]}"#);
    let d = s.record_of(&s.root).unwrap().fields[0].default.as_ref().unwrap();
    assert_eq!(d.text_value().unwrap().len(), 16);
}

#[test]
fn fixed_length_out_of_limit() {
    let e = fail(r#"{"type":"fixed","name":"oversize","namespace":"space","doc":"doc","size":2147483647}"#);
    assert_eq!(e.class, "java.lang.UnsupportedOperationException");
    assert_eq!(e.to_string(), "Cannot read arrays longer than 2147483639 bytes in Java library");
}

#[test]
fn fixed_negative_length() {
    let e = fail(r#"{"type":"fixed","name":"negative","namespace":"space","doc":"doc","size":-1}"#);
    assert_eq!(e.class, AVRO_RUNTIME);
    assert_eq!(e.to_string(), "Malformed data. Length is negative: -1");
}

// ---- TestUnionSelfReference ----

fn branch_names(s: &Schema, t: &Type) -> Vec<String> {
    let Type::Union(types) = t else { panic!("a union") };
    types.iter().map(|b| s.name(b)).collect()
}

#[test]
fn self_reference_in_union() {
    let s = parse(r#"{"namespace":"tree","type":"record","name":"Node","fields":[{"name":"left","type":["null","Node"],"default":null},{"name":"right","type":["null","Node"],"default":null}]}"#);
    let node = s.record_of(&s.root).unwrap();
    for f in ["left", "right"] {
        let f = node.field(f).unwrap();
        assert!(f.default.as_ref().unwrap().is_null());
        assert_eq!(branch_names(&s, &f.schema), ["null", "Node"]);
    }
}

#[test]
fn self_reference_in_three_union() {
    let s = parse(r#"{"namespace":"tree","type":"record","name":"Node","fields":[{"name":"left","type":["null","string","Node"],"default":null},{"name":"right","type":["null","string","Node"],"default":null}]}"#);
    let node = s.record_of(&s.root).unwrap();
    for f in ["left", "right"] {
        let f = node.field(f).unwrap();
        assert!(f.default.as_ref().unwrap().is_null());
        assert_eq!(branch_names(&s, &f.schema), ["null", "string", "Node"]);
    }
}

// ---- SchemaNameValidatorTest (UTF_VALIDATOR, the parser's) ----

#[test]
fn validator() {
    for (name, ok) in [("", false), ("Hello world", false), ("H&", false), ("H=", false), ("H]", false), ("Hello_world", true), ("éàçô", true), ("5éàçô", false), ("_Hello_world", true)] {
        let got = avro::parse_schema(&format!(r#"{{"type":"fixed","name":"{name}","size":1}}"#));
        assert_eq!(got.is_ok(), ok, "{name}: {got:?}");
    }
    let e = fail(r#"{"type":"enum","name":"E","symbols":[null]}"#);
    assert_eq!(e.to_string(), "Null name");
}

// ---- ParseContextTest ----

#[test]
fn check_new_name_context_contains_primitives() {
    for p in ["null", "boolean", "int", "long", "float", "double", "bytes", "string"] {
        assert!(avro::parse_schema(&format!("\"{p}\"")).is_ok(), "{p}");
    }
    for c in ["record", "enum", "fixed", "union", "array", "map"] {
        let e = fail(&format!("\"{c}\""));
        assert_eq!(e.class, "java.lang.NullPointerException", "{c}");
    }
}

#[test]
fn primitives_are_not_cached() {
    let s = parse(r#"{"type":"record","name":"R","fields":[{"name":"a","type":"int"},{"name":"b","type":{"type":"int","logicalType":"brick"}}]}"#);
    let r = s.record_of(&s.root).unwrap();
    assert!(!s.types_equal(&r.fields[0].schema, &r.fields[1].schema));
    assert!(s.types_equal(&r.fields[0].schema, &Type::primitive(Primitive::Int)));
}

#[test]
fn validate_schema_retrieval_failure() {
    let e = fail(r#"{"type":"record","name":"R","fields":[{"name":"a","type":"unknown"}]}"#);
    assert_eq!(e.class, AVRO_TYPE);
    assert_eq!(e.to_string(), "Undefined schema: unknown");
}

#[test]
fn validate_schema_retrieval_by_full_name() {
    let s = parse(r#"{"type":"record","name":"R","fields":[{"name":"a","type":{"type":"record","name":"ns.Foo","fields":[]}},{"name":"b","type":"ns.Foo"}]}"#);
    let r = s.record_of(&s.root).unwrap();
    assert!(matches!((&r.fields[0].schema, &r.fields[1].schema), (Type::Named(a), Type::Named(b)) if a == b));
}

#[test]
fn validate_schema_retrieval_by_simple_name() {
    let s = parse(r#"{"type":"record","name":"R","namespace":"ns","fields":[{"name":"a","type":{"type":"record","name":"ns.Foo","fields":[]}},{"name":"b","type":"Foo"}]}"#);
    let r = s.record_of(&s.root).unwrap();
    assert!(matches!((&r.fields[0].schema, &r.fields[1].schema), (Type::Named(a), Type::Named(b)) if a == b));
}

#[test]
fn verify_put_is_idempotent() {
    parse(r#"["null",{"type":"record","name":"R","fields":[{"name":"a","type":{"type":"record","name":"ns.Foo","fields":[]}},{"name":"b","type":{"type":"record","name":"ns.Foo","fields":[]}}]}]"#);
}

#[test]
fn verify_add_does_not_allow_changing_schemas() {
    let e = fail(r#"{"type":"record","name":"R","fields":[{"name":"a","type":{"type":"record","name":"ns.Foo","fields":[]}},{"name":"b","type":{"type":"enum","name":"ns.Foo","symbols":[]}}]}"#);
    assert_eq!(e.class, SCHEMA_PARSE);
    assert_eq!(e.to_string(), "Can't redefine: ns.Foo");
}

// ---- TestSchemaResolver ----

#[test]
fn test_resolving() {
    // A forward reference resolved: the reference and the definition are one type.
    let s = parse(r#"{"type":"record","name":"R","fields":[{"name":"a","type":"S"},{"name":"b","type":{"type":"record","name":"S","fields":[{"name":"x","type":"int"}]}}]}"#);
    let r = s.record_of(&s.root).unwrap();
    assert!(matches!((&r.fields[0].schema, &r.fields[1].schema), (Type::Named(a), Type::Named(b)) if a == b));
    assert_eq!(s.to_string(), r#"{"type":"record","name":"R","fields":[{"name":"a","type":{"type":"record","name":"S","fields":[{"name":"x","type":"int"}]}},{"name":"b","type":"S"}]}"#);
}

// ---- TestNestedRecords ----

const PARENT: &str = r#"{"type":"record","name":"Parent","namespace":"org.apache.avro.nested","fields":[{"name":"parentField1","type":"string"},{"name":"child1","type":{"type":"record","name":"Child","fields":[{"name":"childField","type":"string"}]}},{"name":"parentField2","type":"string"}]}"#;

#[test]
fn single_sub_record() {
    let s = parse(PARENT);
    let input = "{\n \"parentField1\": \"parentValue1\",\n \"child1\":{\n    \"childField\":\"childValue1\"\n },\n \"parentField2\":\"parentValue2\"\n}";
    let bytes = avro::json_to_avro(&s, input).unwrap();
    assert_eq!(avro::avro_to_json(&s, &bytes).unwrap(), r#"{"parentField1":"parentValue1","child1":{"childField":"childValue1"},"parentField2":"parentValue2"}"#);
}

#[test]
fn single_sub_record_extra_field() {
    let s = parse(PARENT);
    let input = "{\n \"parentField1\": \"parentValue1\",\n \"child1\":{\n    \"childField\":\"childValue1\",\n    \"extraField\":\"extraValue\"\n },\n \"parentField2\":\"parentValue2\"\n}";
    let bytes = avro::json_to_avro(&s, input).unwrap();
    assert_eq!(avro::avro_to_json(&s, &bytes).unwrap(), r#"{"parentField1":"parentValue1","child1":{"childField":"childValue1"},"parentField2":"parentValue2"}"#);
}

#[test]
fn field_order_is_parsed() {
    let s = parse(r#"{"type":"record","name":"R","fields":[{"name":"a","type":"int","order":"descending"},{"name":"b","type":"int","order":"ignore"},{"name":"c","type":"int"}]}"#);
    let orders: Vec<Order> = s.record_of(&s.root).unwrap().fields.iter().map(|f| f.order).collect();
    assert_eq!(orders, [Order::Descending, Order::Ignore, Order::Ascending]);
}
