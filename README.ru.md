# avro-codec-rs

<!-- languages:begin -->
[English](README.md) · **Русский** · [中文](README.zh-CN.md) · [Español](README.es.md)
<!-- languages:end -->
<!-- translation of README.md: 66cd71d75dd7 -->

Неофициальный порт на Rust тех частей [Apache Avro](https://avro.apache.org/) 1.12.2 (Java,
`org.apache.avro:avro`), которые переводят JSON в двоичный формат Avro и обратно, верный выводу Avro и текстам его
исключений. Проект не связан с Apache Software Foundation и не одобрен ею; собственная реализация Apache на Rust — это
крейт `apache-avro`, другая библиотека. Пакет на crates.io называется `avro-codec-rs`; библиотека импортируется как
`avro` (`use avro::...`).

Крейт создан для [mock-server](https://github.com/oxidestack/mock-server) — замены MockServer на Rust, чей
`AvroPayloadCodec` вызывает эту часть Avro и который должен отвечать байт в байт так же, как MockServer 8.0.0. Он
полезен везде, где JSON-кодирование Avro должно совпадать с библиотекой на Java вне JVM.

Статус: до первого выпуска. Крейт ещё не опубликован на crates.io; пока этого не произошло, его используют из
локальной копии этого репозитория.

## Установка

На crates.io он ещё не опубликован; после публикации:

```toml
[dependencies]
avro-codec-rs = "0.1"               # not published yet
```

До тех пор зависьте от локальной копии: `avro-codec-rs = { path = "../avro-codec-rs" }`, а рядом положите копии
[java-compat-rs](https://github.com/oxidestack/java-compat-rs) и
[swagger-parser-rs](https://github.com/oxidestack/swagger-parser-rs) (его `crates/jackson`).

## Быстрый старт

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

Справочник API появится на docs.rs после публикации крейта; из локальной копии его собирает `cargo doc --open`.

## Руководства

[Обновление](docs/i18n/ru/guide/upgrading.md): что каждый выпуск требует от кода, использующего крейт. Этот README и
руководства доступны также в виде сайта, `https://oxidestack.github.io/avro-codec-rs/`, после того как репозиторий
станет публичным.

## Сборка и тесты

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

`upstream/` — исходный код Avro, закреплённый на портированном выпуске, для чтения; ни одному тесту он не нужен.

## Что портировано

- **Парсер схем.** `new Schema.Parser().parse(String)`, портированный как `parse_schema`:
  - Jackson читает текст с разрешёнными комментариями, а содержимое после схемы отвергается.
  - Имена и пространства имён проверяются `UTF_VALIDATOR`, который применяет `Character.isLetter` из Java к
    единицам UTF-16.
  - Именованные типы регистрируются, а повторное определение проверяется так же, как в `ParseContext`. Второе,
    равное определение допускается.
  - Ссылки вперёд становятся заглушками `UnresolvedSchema_<n>` и разрешаются потом, как это делает
    `SchemaResolver.ResolvingVisitor`. Этот проход также заново проверяет значения полей по умолчанию и ветви
    объединений относительно того, что стоит за ссылками.
  - Значения по умолчанию проверяются как в `isValidDefault`. Текстовое значение по умолчанию для `float`/`double`
    проходит через `Double.parseDouble`.
  - Свойства, псевдонимы и порядок полей обрабатываются.
  - Каждый текст `SchemaParseException`, `AvroTypeException`, `AvroRuntimeException`, `NullPointerException` и
    `NumberFormatException` — тот же, что в Avro, как и порядок, в котором обнаруживаются проблемы.
  - `Display` для `Schema` — это `Schema.toString()`: компактный JSON, где каждый именованный тип записан полностью
    там, где он появляется впервые.
- **JSON в двоичный формат.** `GenericDatumReader` (его быстрый читатель по умолчанию) поверх `JsonDecoder`,
  портированный как `json_to_avro`:
  - Повторяется ленивый потоковый парсер Jackson, в том числе там, где upstream прекращает чтение.
  - Грамматика — та, что строит `JsonGrammarGenerator`, с неявными действиями, которые `Parser` выполняет лениво.
  - Поля могут идти в любом порядке; поля, неизвестные схеме, буферизуются через `TokenBuffer`. Неизвестные поля
    приводят к ошибке только там, где в upstream действительно срабатывает `RECORD_END`, и пропуск до следующего
    `END_OBJECT` — такой же, как в upstream.
  - Ветви объединений записываются как `{"branch": value}`.
  - Числа следуют `getIntValue`/`getLongValue` из Jackson (с `InputCoercionException`) и правилам округления Avro.
    `NaN` и бесконечности можно передавать строками.
  - Строки кодируются так, как их кодирует Java: непарный суррогат становится `?`, а bytes и fixed используют
    ISO-8859-1.
  - Затем значение записывается `GenericDatumWriter` в `BinaryEncoder`. Map выводятся в порядке итерации `HashMap`,
    с ключами `Utf8` или, при `"avro.java.string": "String"`, `String`.
- **Двоичный формат в JSON.** `GenericDatumReader` поверх `BinaryDecoder`, портированный как `avro_to_json`:
  - Сохранены усечение в `readInt` и правило `readBoolean` «истинно только 1».
  - Сохранены проверки `SystemLimitException` и `ensureAvailableBytes`, а также проверки диапазонов быстрого
    читателя.
  - Затем значение записывается `GenericDatumWriter` в `JsonEncoder`:
    - JSON компактный.
    - Символы за пределами U+FFFF выводятся как экранированные суррогаты, как их пишет `UTF8JsonGenerator`.
    - Числа с плавающей точкой используют `Float.toString`/`Double.toString` (из `java-compat`).
    - Некорректный UTF-8 декодируется так, как его декодирует JDK.
    - Пустая запись не пишет ничего, потому что грамматика кодировщика никогда не продвигается.

Не портировано, потому что кодек JSON↔двоичный формат туда никогда не доходит:

- Разрешение схем между разными схемами писателя и читателя.
- Specific и reflect data, генерация кода, IDL и протоколы.
- Контейнерные файлы, а также `SchemaBuilder` и другие программные конструкторы.
- Преобразования логических типов: `logicalType` сохраняется как свойство, и это всё, что делает `GenericData` без
  зарегистрированных преобразований.

`parse_schema_with(json, &ParseOptions { relocated_prefix })` воспроизводит shaded-сборку. Образ MockServer,
например, перемещает Jackson и Avro под `shaded_package.`. Префикс подставляется туда, где сообщения Avro печатают
имена классов или пакетов.

## Отличия

- **Глубина вложенности.** Двоичные данные могут вкладывать не более `MAX_DEPTH` (1000) контейнеров. Более глубокие
  данные приводят к `AvroRuntimeException`, тогда как upstream уходит в рекурсию до `StackOverflowError`. Текст JSON —
  и схемы, и данные — ограничен собственным пределом Jackson в 1000 уровней, который соблюдает его парсер. Всё
  укладывается в стек 2 MiB, в том числе в отладочных сборках (`tests/limits.rs`).
- **Элементы нулевого размера.** Ограничение `SystemLimitException` на элементы коллекций, не занимающие байтов
  (`array<null>` и подобные), в upstream выводится из кучи JVM. Здесь это `MAX_ZERO_BYTE_COLLECTION_ALLOCATION`
  (2^20, что дала бы куча в 32 MiB).
- **Деревья `HashMap`.** Корзина `HashMap`, ставшая деревом (девять сталкивающихся ключей в таблице размером 64 и
  больше), здесь сохраняет порядок вставки. Java перемещает корень дерева в начало.
- **Застрявший `RECORD_END`.** `RECORD_END`/`UNION_END` пропускают ввод до следующего `END_OBJECT`. Если ввод
  кончается раньше, upstream зацикливается навсегда; здесь возникает ошибка `EOFException`.
- **Ограничения и сообщения Jackson.**
  - Ограничения `StreamReadConstraints` из Jackson на длину чисел и строк не применяются; ограничение вложенности —
    применяется.
  - Синтаксические ошибки берут сообщения Jackson из читателя крейта `jackson`. Три текста, которые этот крейт
    формулирует иначе (число, обрезанное концом ввода), исправлены для данных, но не для текста схем.
- **Версия Unicode.** Проверка имён использует таблицы Unicode крейта `unicode-general-category` (Unicode 16), а не
  таблицы JVM, на которой работает upstream.

## Тесты

Все зелёные (`cargo test`):

- **Дифференциальные векторы.** Файлы в `tests/vectors/` записаны с самого Avro 1.12.2 (`scripts/record.sh` запускает
  `scripts/Cases.java` с jar-файлами из `~/.m2`), а `tests/vectors.rs` воспроизводит их (JDK и Maven нужны, чтобы
  записать их заново, но не чтобы запускать тесты). Они покрывают парсер схем, декодер JSON и двоичный декодер, у
  каждого случая — полный результат.
- **Пределы вложенности.** `tests/limits.rs` покрывает пределы вложенности.
- **Модульные тесты upstream.** Там, где они доходят до портированного кода, они портированы в `tests/upstream_*.rs`.
  `docs/test-coverage.md` сопоставляет каждый рассмотренный тестовый класс и метод upstream с его портом или
  объясняет, почему он вне рамок.

## Помощь

Вопрос о том, как работает крейт, можно сначала задать [DeepWiki](https://deepwiki.com/oxidestack/avro-codec-rs): она
отвечает по коду и документации этого репозитория, а там, где она расходится с документацией, права документация.
Вопросы, которые она не решает, — в [Discussions](https://github.com/oxidestack/avro-codec-rs/discussions), ошибки —
в [issues](https://github.com/oxidestack/avro-codec-rs/issues). <!-- these three links need the public repository; it is not published yet -->

## Лицензия

Код распространяется по лицензии [Apache License, Version 2.0](LICENSE) (`Apache-2.0`); то, что он портирует,
указано в [`NOTICE`](NOTICE). Документация — этот README и руководства в `docs/` — распространяется по лицензии
[Creative Commons Attribution 4.0 International](https://creativecommons.org/licenses/by/4.0/) (`CC-BY-4.0`); её
юридический текст — в [`LICENSE-docs`](LICENSE-docs). Вклады подписываются по DCO
([`CONTRIBUTING.md`](CONTRIBUTING.md)).
