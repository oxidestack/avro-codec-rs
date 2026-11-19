//! Differential vectors recorded from Avro 1.12.2 (`scripts/Cases.java` over `tests/vectors/*.tsv`): a schema, an
//! optional payload (JSON, or `HEX:` binary), then what Avro did — `OK <Schema.toString()>` for a schema alone,
//! `<binary hex> <JSON read back>` for a JSON payload, `BIN <JSON>` for a binary one, or `ERR <exception simple
//! name>: <message>` (newlines as `\n`) where it threw.

fn err(e: &avro::Error) -> String {
    format!("ERR {}: {}", e.simple_class(), e.to_string().replace('\n', "\\n"))
}

fn run(schema_json: &str, payload: Option<&str>) -> String {
    let schema = match avro::parse_schema(schema_json) {
        Ok(s) => s,
        Err(e) => return err(&e),
    };
    let Some(payload) = payload else { return format!("OK {schema}") };
    if let Some(hex) = payload.strip_prefix("HEX:") {
        let bytes: Vec<u8> = (0..hex.len() / 2).map(|i| u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).expect("hex")).collect();
        return match avro::avro_to_json(&schema, &bytes) {
            Ok(json) => format!("BIN {json}"),
            Err(e) => err(&e),
        };
    }
    match avro::json_to_avro(&schema, payload) {
        Ok(bytes) => {
            let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
            match avro::avro_to_json(&schema, &bytes) {
                Ok(json) => format!("{hex} {json}"),
                Err(e) => format!("{hex} {}", err(&e)),
            }
        }
        Err(e) => err(&e),
    }
}

/// `UnresolvedSchema_<n>` counts across the process, so its number is not compared.
fn normalize(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(i) = rest.find("UnresolvedSchema_") {
        out.push_str(&rest[..i + "UnresolvedSchema_".len()]);
        rest = rest[i + "UnresolvedSchema_".len()..].trim_start_matches(|c: char| c.is_ascii_digit());
        out.push('N');
    }
    out.push_str(rest);
    out
}

fn check(file: &str, table: &str) {
    let mut failures = Vec::new();
    let mut count = 0;
    for line in table.lines().filter(|l| !l.trim().is_empty() && !l.starts_with('#')) {
        let cols: Vec<&str> = line.split('\t').collect();
        let (schema, payload, want) = match cols.len() {
            3 => (cols[0], Some(cols[1]), cols[2]),
            2 => (cols[0], None, cols[1]),
            _ => panic!("{file}: bad row {line}"),
        };
        let schema = if schema == "<EMPTY>" { "" } else { schema };
        let got = run(schema, payload);
        count += 1;
        if normalize(&got) != normalize(want) {
            failures.push(format!("{} | {}\n  got  {got}\n  want {want}", cols[0], payload.unwrap_or("-")));
        }
    }
    eprintln!("{file}: {count} rows");
    assert!(failures.is_empty(), "{file}: {} of {count} differ:\n{}", failures.len(), failures.join("\n"));
}

#[test]
fn mockserver_cases() {
    check("cases.tsv", include_str!("vectors/cases.tsv"));
}

#[test]
fn parser_cases() {
    check("parser.tsv", include_str!("vectors/parser.tsv"));
}

#[test]
fn json_cases() {
    check("json.tsv", include_str!("vectors/json.tsv"));
}

#[test]
fn binary_cases() {
    check("binary.tsv", include_str!("vectors/binary.tsv"));
}

/// `TestSchemaCommons.runFolder`'s parse, and the other schemas under `share/test`.
#[test]
fn shared_schemas() {
    check("shared.tsv", include_str!("vectors/shared.tsv"));
}

#[test]
fn json_more_cases() {
    check("json2.tsv", include_str!("vectors/json2.tsv"));
}
