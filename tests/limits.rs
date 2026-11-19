//! Hostile nesting cannot overflow a 2 MiB stack (a tokio worker's), even in a debug build: schema and JSON text
//! are held to Jackson's nesting limit of 1000, binary data to [`avro::MAX_DEPTH`].

fn on_small_stack(f: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new().stack_size(2 << 20).spawn(f).expect("thread").join().expect("no overflow");
}

const LIST: &str = r#"{"type":"record","name":"L","fields":[{"name":"next","type":["null","L"]}]}"#;

fn arrays(depth: usize, leaf: &str) -> String {
    format!("{}{leaf}{}", r#"{"type":"array","items":"#.repeat(depth), "}".repeat(depth))
}

fn records(depth: usize) -> String {
    let mut json = String::from("\"int\"");
    for i in 0..depth {
        json = format!(r#"{{"type":"record","name":"R{i}","fields":[{{"name":"f","type":{json}}}]}}"#);
    }
    json
}

#[test]
fn schemas_at_the_nesting_limit_work_and_past_it_fail() {
    on_small_stack(|| {
        let schema = avro::parse_schema(&arrays(999, "\"int\"")).unwrap();
        let _ = schema.to_string();
        let payload = format!("{}1{}", "[".repeat(999), "]".repeat(999));
        let bytes = avro::json_to_avro(&schema, &payload).unwrap();
        assert_eq!(avro::avro_to_json(&schema, &bytes).unwrap(), payload);
        assert!(avro::parse_schema(&arrays(999, "\"nope\"")).is_err());
        assert!(avro::parse_schema(&arrays(1000, "\"int\"")).is_ok());
        for depth in [1001, 5000] {
            let e = avro::parse_schema(&arrays(depth, "\"int\"")).expect_err("too deep");
            assert!(e.to_string().contains("exceeds the maximum allowed (1000"), "{depth}: {e}");
        }
        let schema = avro::parse_schema(&records(332)).unwrap();
        let _ = schema.to_string();
        let mut payload = String::from("1");
        for _ in 0..332 {
            payload = format!(r#"{{"f":{payload}}}"#);
        }
        let bytes = avro::json_to_avro(&schema, &payload).unwrap();
        assert_eq!(avro::avro_to_json(&schema, &bytes).unwrap(), payload);
        // The message quotes the whole schema.
        let e = avro::parse_schema(&records(332).replacen(",\"type\":{", ",\"default\":{},\"type\":{", 1)).unwrap_err();
        assert!(e.to_string().starts_with("Invalid default for field f: {} not a {\"type\":\"record\""), "{e}");
    });
}

#[test]
fn json_past_the_nesting_limit_fails() {
    on_small_stack(|| {
        let schema = avro::parse_schema(LIST).unwrap();
        // Two JSON levels per list node.
        let json = format!("{}{{\"next\":null}}{}", r#"{"next":{"L":"#.repeat(499), "}}".repeat(499));
        let bytes = avro::json_to_avro(&schema, &json).unwrap();
        assert_eq!(avro::avro_to_json(&schema, &bytes).unwrap(), json);
        let deep = format!("{}{{\"next\":null}}{}", r#"{"next":{"L":"#.repeat(3000), "}}".repeat(3000));
        let e = avro::json_to_avro(&schema, &deep).unwrap_err();
        assert!(e.to_string().contains("exceeds the maximum allowed (1000"), "{e}");
    });
}

#[test]
fn binary_past_max_depth_fails() {
    on_small_stack(|| {
        let schema = avro::parse_schema(LIST).unwrap();
        // Each node is the union index 1 (`02`), the last one null (`00`); a node is two levels (record, union).
        let mut data = vec![0x02; avro::MAX_DEPTH / 2 - 1];
        data.push(0x00);
        assert!(avro::avro_to_json(&schema, &data).is_ok());
        let mut data = vec![0x02; 1_000_000];
        data.push(0x00);
        let e = avro::avro_to_json(&schema, &data).unwrap_err();
        assert_eq!(e.class, "org.apache.avro.AvroRuntimeException");
        assert_eq!(e.to_string(), "Datum nesting depth (1001) exceeds the maximum allowed (1000)");
    });
}

#[test]
fn zero_byte_elements_are_capped() {
    let schema = avro::parse_schema(r#"{"type":"array","items":"null"}"#).unwrap();
    // A block of 2^29 nulls in five bytes.
    let e = avro::avro_to_json(&schema, &[0x80, 0x80, 0x80, 0x80, 0x04, 0x00]).unwrap_err();
    assert_eq!(e.class, "org.apache.avro.SystemLimitException");
    assert!(e.to_string().starts_with("Cannot allocate 536870912 collection elements whose minimum encoded size is zero"), "{e}");
}
