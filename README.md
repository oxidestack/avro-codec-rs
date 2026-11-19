# avro-codec-rs

<!-- languages:begin -->
**English** · [Русский](README.ru.md) · [中文](README.zh-CN.md) · [Español](README.es.md)
<!-- languages:end -->

An unofficial Rust port of the parts of [Apache Avro](https://avro.apache.org/) 1.12.2 (Java, `org.apache.avro:avro`)
that turn JSON into Avro binary and back, faithful to Avro's output and exception texts. It is not affiliated with or
endorsed by the Apache Software Foundation; Apache's own Rust implementation is the `apache-avro` crate, a different
library. The crates.io package is `avro-codec-rs`; the library is imported as `avro` (`use avro::...`).

It exists for [mock-server](https://github.com/oxidestack/mock-server), a Rust drop-in for MockServer, whose
`AvroPayloadCodec` calls this part of Avro and which must answer byte for byte as MockServer 8.0.0 does. It is useful
wherever Avro's JSON encoding has to match the Java library's outside the JVM.

Status: pre-release. The crate is not published on crates.io yet; until it is, it is used from a checkout of this
repository.

## Install

It is not published on crates.io yet; once it is:

```toml
[dependencies]
avro-codec-rs = "0.1"               # not published yet
```

Until then, depend on a checkout: `avro-codec-rs = { path = "../avro-codec-rs" }`, with
[java-compat-rs](https://github.com/oxidestack/java-compat-rs) and
[swagger-parser-rs](https://github.com/oxidestack/swagger-parser-rs) (its `crates/jackson`) checked out beside it.

## Quick start

```rust
fn main() -> Result<(), avro::Error> {
    let schema = avro::parse_schema(
        r#"{"type":"record","name":"Pet","fields":[{"name":"name","type":"string"},{"name":"age","type":"int"}]}"#,
    )?;
    let bytes = avro::json_to_avro(&schema, r#"{"name":"Rex","age":3}"#)?;
    assert_eq!(bytes, [6, b'R', b'e', b'x', 6]);
    assert_eq!(avro::avro_to_json(&schema, &bytes)?, r#"{"name":"Rex","age":3}"#);
    Ok(())
}
```

The API reference will be on docs.rs once the crate is published; `cargo doc --open` builds it from a checkout.

## Guides

[Upgrading](docs/guide/upgrading.md): what each release asks of code that uses the crate. This README and the guides
are also a site, `https://oxidestack.github.io/avro-codec-rs/`, once the repository is public.

## Build and test

```sh
git clone https://github.com/oxidestack/java-compat-rs ../java-compat-rs                # JDK semantics this crate builds on
git clone https://github.com/oxidestack/swagger-parser-rs ../swagger-parser-rs          # provides crates/jackson, the JSON reader
git submodule update --init upstream   # Apache Avro 1.12.2's source, read-only reference
cargo test
make lint        # clippy, warnings are errors
make deny        # licences, advisories, sources (cargo install cargo-deny --locked)
make doc         # the API documentation, rustdoc's warnings are errors
make public-check
make site        # the documentation site (site/README.md), in a pinned node container
```

`upstream/` is Avro's source pinned to the ported release, for reading; no test needs it.

## Scope

- **The schema parser.** `new Schema.Parser().parse(String)`, ported as `parse_schema`:
  - Jackson reads the text with comments allowed, and content after the schema is refused.
  - Names and namespaces are checked with `UTF_VALIDATOR`, which applies Java's `Character.isLetter` to UTF-16 units.
  - Named types are registered and redefinition is checked as `ParseContext` does. A second, equal definition is
    allowed.
  - Forward references become `UnresolvedSchema_<n>` placeholders and are resolved afterwards, as
    `SchemaResolver.ResolvingVisitor` does. That pass also re-checks field defaults and union branches against what
    the references stand for.
  - Defaults are validated as `isValidDefault`. A textual default for `float`/`double` goes through
    `Double.parseDouble`.
  - Properties, aliases and field order are handled.
  - Every `SchemaParseException`, `AvroTypeException`, `AvroRuntimeException`, `NullPointerException` and
    `NumberFormatException` text is Avro's, and so is the order in which problems are found.
  - `Display` for `Schema` is `Schema.toString()`: compact JSON, with each named type written in full where it first
    appears.
- **JSON to binary.** `GenericDatumReader` (its default fast reader) runs over `JsonDecoder`, ported as
  `json_to_avro`:
  - Jackson's lazy streaming parser is followed, including when upstream stops reading.
  - The grammar is `JsonGrammarGenerator`'s, with the implicit actions `Parser` runs lazily.
  - Fields may come in any order; fields the schema does not know are buffered through `TokenBuffer`s. Unknown
    fields fail only where upstream's `RECORD_END` actually runs, and the skip to the next `END_OBJECT` is
    upstream's.
  - Union branches are written as `{"branch": value}`.
  - Numbers follow Jackson's `getIntValue`/`getLongValue` (with `InputCoercionException`) and Avro's
    rounding rules. `NaN` and the infinities may be given as strings.
  - Strings are encoded as Java encodes them: an unpaired surrogate becomes `?`, and bytes and fixed values use
    ISO-8859-1.
  - The value is then written with `GenericDatumWriter` to `BinaryEncoder`. Maps come out in `HashMap` iteration
    order, keyed by `Utf8` or, under `"avro.java.string": "String"`, by `String`.
- **Binary to JSON.** `GenericDatumReader` runs over `BinaryDecoder`, ported as `avro_to_json`:
  - `readInt`'s truncation and `readBoolean`'s "only 1 is true" are kept.
  - The checks of `SystemLimitException` and `ensureAvailableBytes` are kept, along with the fast reader's range
    checks.
  - The value is then written with `GenericDatumWriter` to `JsonEncoder`:
    - The JSON is compact.
    - Characters past U+FFFF come out as escaped surrogates, as `UTF8JsonGenerator` writes them.
    - Floats use `Float.toString`/`Double.toString` (from `java-compat`).
    - Malformed UTF-8 is decoded as the JDK decodes it.
    - An empty record writes nothing, because the encoder's grammar never advances.

Not ported, because a JSON↔binary codec never reaches it:

- Schema resolution between different writer and reader schemas.
- Specific and reflect data, code generation, IDL and protocols.
- Container files, and `SchemaBuilder` and the other programmatic constructors.
- Logical-type conversions: a `logicalType` is kept as a property, and that is all `GenericData` does without
  registered conversions.

`parse_schema_with(json, &ParseOptions { relocated_prefix })` reproduces a shaded build. MockServer's image, for
example, relocates Jackson and Avro under `shaded_package.`. The prefix goes where Avro's messages print class or
package names.

## Differences

- **Nesting depth.** Binary data may nest at most `MAX_DEPTH` (1000) containers deep. Deeper data fails with an
  `AvroRuntimeException`, where upstream recurses until `StackOverflowError`. JSON text, both schemas and payloads,
  is held to Jackson's own limit of 1000 by its parser. Everything stays within a 2 MiB stack, in debug builds too
  (`tests/limits.rs`).
- **Zero-byte elements.** `SystemLimitException`'s cap on collection elements that take no bytes (`array<null>`
  and the like) derives from the JVM heap upstream. Here it is `MAX_ZERO_BYTE_COLLECTION_ALLOCATION` (2^20, which
  is what a 32 MiB heap would give).
- **`HashMap` trees.** A `HashMap` bucket that becomes a tree (nine colliding keys in a table of 64 or more) keeps
  insertion order here. Java moves the tree's root to the front.
- **Stuck `RECORD_END`.** `RECORD_END`/`UNION_END` skip to the next `END_OBJECT`. When the input runs out first,
  upstream loops for ever; here it fails with `EOFException`.
- **Jackson limits and messages.**
  - Jackson's `StreamReadConstraints` limits on number and string length are not enforced; its nesting limit is.
  - Syntax errors take Jackson's messages from the `jackson` crate's reader. The three texts that crate words
    differently (a number cut short by the end of input) are corrected for payloads, but not for schema text.
- **Unicode version.** Name validation uses the Unicode tables of the `unicode-general-category` crate (Unicode
  16), not those of the JVM running upstream.

## Tests

All green (`cargo test`):

- **Differential vectors.** The files in `tests/vectors/` were recorded from Avro 1.12.2 itself (`scripts/record.sh`
  runs `scripts/Cases.java` against the jars in `~/.m2`), and `tests/vectors.rs` replays them (a JDK and Maven are needed to re-record, not to run the tests). They cover the
  schema parser, the JSON decoder and the binary decoder, each case with its full outcome.
- **Nesting limits.** `tests/limits.rs` covers the nesting limits.
- **Upstream's unit tests.** Where they reach the ported code, they are ported in `tests/upstream_*.rs`.
  `docs/test-coverage.md` maps every upstream test class and method that was considered to its port, or says why it
  is out of scope.

## Getting help

A question about how the crate works can go to [DeepWiki](https://deepwiki.com/oxidestack/avro-codec-rs) first: it
answers from this repository's code and documentation, and where it disagrees with the documentation, the
documentation is right. Questions it does not settle go to
[Discussions](https://github.com/oxidestack/avro-codec-rs/discussions), bugs to
[issues](https://github.com/oxidestack/avro-codec-rs/issues). <!-- these three links need the public repository; it is not published yet -->

## Licence

The code is licensed under the [Apache License, Version 2.0](LICENSE) (`Apache-2.0`); what it ports is attributed in
[`NOTICE`](NOTICE). The documentation — this README and the guides under `docs/` — is licensed under the
[Creative Commons Attribution 4.0 International](https://creativecommons.org/licenses/by/4.0/) licence
(`CC-BY-4.0`); its legal text is in [`LICENSE-docs`](LICENSE-docs). Contributions are signed off under the DCO
([`CONTRIBUTING.md`](CONTRIBUTING.md)).
