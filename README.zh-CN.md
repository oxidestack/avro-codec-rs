# avro-codec-rs

<!-- languages:begin -->
[English](README.md) · [Русский](README.ru.md) · **中文** · [Español](README.es.md)
<!-- languages:end -->
<!-- translation of README.md: 66cd71d75dd7 -->

[Apache Avro](https://avro.apache.org/) 1.12.2（Java，`org.apache.avro:avro`）中在 JSON 与 Avro 二进制之间互相转换的
部分的非官方 Rust 移植，忠实于 Avro 的输出和异常文本。本项目与 Apache Software Foundation 无关联，也未获其认可；
Apache 自己的 Rust 实现是 `apache-avro` crate，那是另一个库。crates.io 上的包名为 `avro-codec-rs`；库以 `avro` 导入
（`use avro::...`）。

它是为 [mock-server](https://github.com/oxidestack/mock-server) 而写的。mock-server 是 MockServer 的 Rust 替代品，
其 `AvroPayloadCodec` 调用 Avro 的这一部分，并且必须与 MockServer 8.0.0 逐字节一致地应答。凡是需要在 JVM 之外让 Avro
的 JSON 编码与 Java 库一致的地方，它都能派上用场。

状态：尚未发布。本 crate 还没有发布到 crates.io；在此之前，请从本仓库的本地检出使用它。

## 安装

它尚未发布到 crates.io；发布之后：

```toml
[dependencies]
avro-codec-rs = "0.1"               # not published yet
```

在此之前，请依赖本地检出：`avro-codec-rs = { path = "../avro-codec-rs" }`，并在其旁边检出
[java-compat-rs](https://github.com/oxidestack/java-compat-rs) 和
[swagger-parser-rs](https://github.com/oxidestack/swagger-parser-rs)（用到其 `crates/jackson`）。

## 快速开始

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

crate 发布后，API 参考将位于 docs.rs；在本地检出中可用 `cargo doc --open` 生成。

## 指南

[升级](docs/i18n/zh/guide/upgrading.md)：每个版本对使用本 crate 的代码有哪些要求。仓库公开后，本 README 和这些指南也会以
站点形式提供：`https://oxidestack.github.io/avro-codec-rs/`。

## 构建与测试

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

`upstream/` 是固定在所移植版本上的 Avro 源码，仅供阅读；没有测试需要它。

## 范围

- **schema 解析器。** `new Schema.Parser().parse(String)`，移植为 `parse_schema`：
  - Jackson 以允许注释的方式读取文本，schema 之后的内容会被拒绝。
  - 名称和命名空间由 `UTF_VALIDATOR` 检查，它对 UTF-16 码元应用 Java 的 `Character.isLetter`。
  - 命名类型的注册和重复定义检查与 `ParseContext` 相同。允许第二个相等的定义。
  - 前向引用先成为 `UnresolvedSchema_<n>` 占位符，之后再解析，与 `SchemaResolver.ResolvingVisitor` 相同。这一遍还会
    根据引用所代表的内容重新检查字段默认值和 union 分支。
  - 默认值按 `isValidDefault` 校验。`float`/`double` 的文本默认值经过 `Double.parseDouble`。
  - 处理属性、别名和字段顺序。
  - 每条 `SchemaParseException`、`AvroTypeException`、`AvroRuntimeException`、`NullPointerException` 和
    `NumberFormatException` 的文本都与 Avro 相同，发现问题的顺序也相同。
  - `Schema` 的 `Display` 即 `Schema.toString()`：紧凑的 JSON，每个命名类型在首次出现处完整写出。
- **JSON 转二进制。** `GenericDatumReader`（其默认的快速读取器）运行在 `JsonDecoder` 之上，移植为 `json_to_avro`：
  - 遵循 Jackson 的惰性流式解析器，包括 upstream 停止读取的情形。
  - 语法是 `JsonGrammarGenerator` 生成的，带有 `Parser` 惰性执行的隐式动作。
  - 字段可按任意顺序出现；schema 不认识的字段通过 `TokenBuffer` 缓冲。未知字段只在 upstream 的 `RECORD_END`
    真正执行之处才报错，跳到下一个 `END_OBJECT` 的方式也与 upstream 相同。
  - union 分支写作 `{"branch": value}`。
  - 数字遵循 Jackson 的 `getIntValue`/`getLongValue`（带 `InputCoercionException`）和 Avro 的舍入规则。`NaN`
    和无穷大可以以字符串给出。
  - 字符串按 Java 的方式编码：孤立的代理项变成 `?`，bytes 和 fixed 值使用 ISO-8859-1。
  - 然后由 `GenericDatumWriter` 把值写入 `BinaryEncoder`。map 按 `HashMap` 的迭代顺序输出，键为 `Utf8`，在
    `"avro.java.string": "String"` 下则为 `String`。
- **二进制转 JSON。** `GenericDatumReader` 运行在 `BinaryDecoder` 之上，移植为 `avro_to_json`：
  - 保留 `readInt` 的截断和 `readBoolean` 的"只有 1 为真"。
  - 保留 `SystemLimitException` 和 `ensureAvailableBytes` 的检查，以及快速读取器的范围检查。
  - 然后由 `GenericDatumWriter` 把值写入 `JsonEncoder`：
    - JSON 是紧凑的。
    - U+FFFF 以上的字符以转义的代理对输出，与 `UTF8JsonGenerator` 的写法相同。
    - 浮点数使用 `Float.toString`/`Double.toString`（来自 `java-compat`）。
    - 格式错误的 UTF-8 按 JDK 的方式解码。
    - 空 record 什么也不写，因为编码器的语法从不前进。

未移植的部分，因为 JSON↔二进制编解码器永远不会用到：

- 不同写入方与读取方 schema 之间的 schema 解析（schema resolution）。
- specific 和 reflect data、代码生成、IDL 和协议。
- 容器文件，以及 `SchemaBuilder` 和其他编程式构造器。
- 逻辑类型转换：`logicalType` 作为属性保留，这也是 `GenericData` 在没有注册转换时所做的全部。

`parse_schema_with(json, &ParseOptions { relocated_prefix })` 重现 shaded 构建。例如，MockServer 的镜像把 Jackson 和
Avro 重定位到 `shaded_package.` 之下。该前缀出现在 Avro 消息打印类名或包名的地方。

## 差异

- **嵌套深度。** 二进制数据最多可嵌套 `MAX_DEPTH`（1000）层容器。更深的数据会以 `AvroRuntimeException` 失败，而
  upstream 会一直递归直到 `StackOverflowError`。JSON 文本（schema 和数据）受 Jackson 解析器自身 1000 层的限制。
  一切都在 2 MiB 的栈内完成，调试构建也是如此（`tests/limits.rs`）。
- **零字节元素。** `SystemLimitException` 对不占字节的集合元素（`array<null>` 之类）的上限，在 upstream 中由 JVM
  堆推导而来。这里是 `MAX_ZERO_BYTE_COLLECTION_ALLOCATION`（2^20，即 32 MiB 堆所得的值）。
- **`HashMap` 树。** 变成树的 `HashMap` 桶（大小 64 或以上的表中有九个冲突的键）在这里保持插入顺序。Java 会把树的
  根移到最前面。
- **卡住的 `RECORD_END`。** `RECORD_END`/`UNION_END` 会跳到下一个 `END_OBJECT`。若输入先结束，upstream 会无限循环；
  这里则以 `EOFException` 失败。
- **Jackson 的限制和消息。**
  - 不强制执行 Jackson `StreamReadConstraints` 对数字和字符串长度的限制；其嵌套限制则会执行。
  - 语法错误使用 `jackson` crate 读取器中的 Jackson 消息。该 crate 措辞不同的三条文本（被输入结尾截断的数字）对数据
    做了更正，但对 schema 文本没有。
- **Unicode 版本。** 名称校验使用 `unicode-general-category` crate 的 Unicode 表（Unicode 16），而不是运行 upstream
  的 JVM 的表。

## 测试

全部通过（`cargo test`）：

- **差分向量。** `tests/vectors/` 中的文件录自 Avro 1.12.2 本身（`scripts/record.sh` 用 `~/.m2` 中的 jar 运行
  `scripts/Cases.java`），`tests/vectors.rs` 回放它们（重新录制需要 JDK 和 Maven，运行测试则不需要）。它们覆盖
  schema 解析器、JSON 解码器和二进制解码器，每个用例都有完整的结果。
- **嵌套限制。** `tests/limits.rs` 覆盖嵌套限制。
- **upstream 的单元测试。** 凡是触及所移植代码的，都移植到了 `tests/upstream_*.rs`。`docs/test-coverage.md` 把每个
  考虑过的 upstream 测试类和方法对应到其移植，或说明它为何不在范围内。

## 获取帮助

关于本 crate 如何工作的问题，可以先问 [DeepWiki](https://deepwiki.com/oxidestack/avro-codec-rs)：它根据本仓库的代码和
文档作答；若它与文档不一致，以文档为准。它解决不了的问题请发到
[Discussions](https://github.com/oxidestack/avro-codec-rs/discussions)，缺陷请提交到
[issues](https://github.com/oxidestack/avro-codec-rs/issues)。 <!-- these three links need the public repository; it is not published yet -->

## 许可证

代码采用 [Apache License, Version 2.0](LICENSE)（`Apache-2.0`）许可；所移植的内容在 [`NOTICE`](NOTICE) 中注明出处。
文档（本 README 及 `docs/` 下的指南）采用
[Creative Commons Attribution 4.0 International](https://creativecommons.org/licenses/by/4.0/)（`CC-BY-4.0`）许可；
其法律文本见 [`LICENSE-docs`](LICENSE-docs)。贡献按 DCO 签署（[`CONTRIBUTING.md`](CONTRIBUTING.md)）。
