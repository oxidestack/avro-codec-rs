# avro-codec-rs

<!-- languages:begin -->
[English](README.md) · [Русский](README.ru.md) · [中文](README.zh-CN.md) · **Español**
<!-- languages:end -->
<!-- translation of README.md: 66cd71d75dd7 -->

Un port no oficial a Rust de las partes de [Apache Avro](https://avro.apache.org/) 1.12.2 (Java,
`org.apache.avro:avro`) que convierten JSON en binario de Avro y viceversa, fiel a la salida de Avro y a los textos de
sus excepciones. No está afiliado a la Apache Software Foundation ni cuenta con su aval; la implementación propia de
Apache en Rust es el crate `apache-avro`, una biblioteca distinta. El paquete de crates.io es `avro-codec-rs`; la
biblioteca se importa como `avro` (`use avro::...`).

Existe para [mock-server](https://github.com/oxidestack/mock-server), un sustituto directo de MockServer en Rust, cuyo
`AvroPayloadCodec` llama a esta parte de Avro y que debe responder byte a byte como lo hace MockServer 8.0.0. Es útil
allí donde la codificación JSON de Avro tenga que coincidir con la de la biblioteca de Java fuera de la JVM.

Estado: previo a la primera versión. El crate aún no está publicado en crates.io; hasta entonces se usa desde una copia
local de este repositorio.

## Instalación

Aún no está publicado en crates.io; cuando lo esté:

```toml
[dependencies]
avro-codec-rs = "0.1"               # not published yet
```

Hasta entonces, depende de una copia local: `avro-codec-rs = { path = "../avro-codec-rs" }`, con
[java-compat-rs](https://github.com/oxidestack/java-compat-rs) y
[swagger-parser-rs](https://github.com/oxidestack/swagger-parser-rs) (su `crates/jackson`) clonados a su lado.

## Inicio rápido

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

La referencia de la API estará en docs.rs cuando el crate se publique; desde una copia local la genera
`cargo doc --open`.

## Guías

[Actualización](docs/i18n/es/guide/upgrading.md): lo que cada versión pide al código que usa el crate. Este README y
las guías son también un sitio, `https://oxidestack.github.io/avro-codec-rs/`, cuando el repositorio sea público.

## Compilar y probar

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

`upstream/` es el código fuente de Avro fijado en la versión portada, para consulta; ninguna prueba lo necesita.

## Alcance

- **El parser de esquemas.** `new Schema.Parser().parse(String)`, portado como `parse_schema`:
  - Jackson lee el texto con los comentarios permitidos, y se rechaza el contenido que sigue al esquema.
  - Los nombres y espacios de nombres se comprueban con `UTF_VALIDATOR`, que aplica `Character.isLetter` de Java a
    unidades UTF-16.
  - Los tipos con nombre se registran y la redefinición se comprueba como lo hace `ParseContext`. Se permite una
    segunda definición igual.
  - Las referencias adelantadas se convierten en marcadores `UnresolvedSchema_<n>` y se resuelven después, como lo
    hace `SchemaResolver.ResolvingVisitor`. Esa pasada también vuelve a comprobar los valores por defecto de los campos
    y las ramas de las uniones frente a lo que representan las referencias.
  - Los valores por defecto se validan como en `isValidDefault`. Un valor por defecto textual para `float`/`double`
    pasa por `Double.parseDouble`.
  - Se tratan las propiedades, los alias y el orden de los campos.
  - Cada texto de `SchemaParseException`, `AvroTypeException`, `AvroRuntimeException`, `NullPointerException` y
    `NumberFormatException` es el de Avro, y también el orden en que se detectan los problemas.
  - `Display` para `Schema` es `Schema.toString()`: JSON compacto, con cada tipo con nombre escrito completo donde
    aparece por primera vez.
- **De JSON a binario.** `GenericDatumReader` (su lector rápido por defecto) sobre `JsonDecoder`, portado como
  `json_to_avro`:
  - Se sigue el parser perezoso en streaming de Jackson, también donde upstream deja de leer.
  - La gramática es la de `JsonGrammarGenerator`, con las acciones implícitas que `Parser` ejecuta de forma perezosa.
  - Los campos pueden llegar en cualquier orden; los campos que el esquema no conoce se almacenan en `TokenBuffer`.
    Los campos desconocidos solo fallan donde `RECORD_END` de upstream llega a ejecutarse, y el salto al siguiente
    `END_OBJECT` es el de upstream.
  - Las ramas de las uniones se escriben como `{"branch": value}`.
  - Los números siguen `getIntValue`/`getLongValue` de Jackson (con `InputCoercionException`) y las reglas de
    redondeo de Avro. `NaN` y los infinitos pueden darse como cadenas.
  - Las cadenas se codifican como las codifica Java: un sustituto (surrogate) sin pareja se convierte en `?`, y los
    valores bytes y fixed usan ISO-8859-1.
  - Después el valor se escribe con `GenericDatumWriter` en `BinaryEncoder`. Los mapas salen en el orden de
    iteración de `HashMap`, con claves `Utf8` o, con `"avro.java.string": "String"`, `String`.
- **De binario a JSON.** `GenericDatumReader` sobre `BinaryDecoder`, portado como `avro_to_json`:
  - Se mantienen el truncamiento de `readInt` y el «solo 1 es verdadero» de `readBoolean`.
  - Se mantienen las comprobaciones de `SystemLimitException` y `ensureAvailableBytes`, junto con las comprobaciones
    de rango del lector rápido.
  - Después el valor se escribe con `GenericDatumWriter` en `JsonEncoder`:
    - El JSON es compacto.
    - Los caracteres más allá de U+FFFF salen como sustitutos escapados, como los escribe `UTF8JsonGenerator`.
    - Los números de coma flotante usan `Float.toString`/`Double.toString` (de `java-compat`).
    - El UTF-8 mal formado se decodifica como lo decodifica el JDK.
    - Un registro vacío no escribe nada, porque la gramática del codificador nunca avanza.

No portado, porque un códec JSON↔binario nunca llega a ello:

- La resolución de esquemas entre esquemas de escritor y de lector distintos.
- Specific y reflect data, la generación de código, IDL y los protocolos.
- Los archivos contenedores, y `SchemaBuilder` y los demás constructores programáticos.
- Las conversiones de tipos lógicos: un `logicalType` se conserva como propiedad, y eso es todo lo que hace
  `GenericData` sin conversiones registradas.

`parse_schema_with(json, &ParseOptions { relocated_prefix })` reproduce una compilación shaded. La imagen de
MockServer, por ejemplo, reubica Jackson y Avro bajo `shaded_package.`. El prefijo va donde los mensajes de Avro
imprimen nombres de clases o de paquetes.

## Diferencias

- **Profundidad de anidamiento.** Los datos binarios pueden anidar como máximo `MAX_DEPTH` (1000) contenedores. Los
  datos más profundos fallan con un `AvroRuntimeException`, donde upstream recursa hasta `StackOverflowError`. El texto
  JSON, tanto de esquemas como de datos, está sujeto al límite propio de Jackson de 1000 que aplica su parser. Todo
  cabe en una pila de 2 MiB, también en compilaciones de depuración (`tests/limits.rs`).
- **Elementos de cero bytes.** El límite de `SystemLimitException` para los elementos de colección que no ocupan
  bytes (`array<null>` y similares) se deriva del heap de la JVM en upstream. Aquí es
  `MAX_ZERO_BYTE_COLLECTION_ALLOCATION` (2^20, lo que daría un heap de 32 MiB).
- **Árboles de `HashMap`.** Un cubo de `HashMap` que se convierte en árbol (nueve claves en colisión en una tabla de 64
  o más) conserva aquí el orden de inserción. Java mueve la raíz del árbol al principio.
- **`RECORD_END` atascado.** `RECORD_END`/`UNION_END` saltan hasta el siguiente `END_OBJECT`. Cuando la entrada se
  acaba antes, upstream entra en un bucle infinito; aquí falla con `EOFException`.
- **Límites y mensajes de Jackson.**
  - No se aplican los límites de `StreamReadConstraints` de Jackson sobre la longitud de números y cadenas; su límite
    de anidamiento sí.
  - Los errores de sintaxis toman los mensajes de Jackson del lector del crate `jackson`. Los tres textos que ese crate
    redacta de otro modo (un número cortado por el final de la entrada) se corrigen para los datos, pero no para el
    texto de los esquemas.
- **Versión de Unicode.** La validación de nombres usa las tablas de Unicode del crate `unicode-general-category`
  (Unicode 16), no las de la JVM en la que se ejecuta upstream.

## Pruebas

Todas en verde (`cargo test`):

- **Vectores diferenciales.** Los archivos de `tests/vectors/` se grabaron con el propio Avro 1.12.2
  (`scripts/record.sh` ejecuta `scripts/Cases.java` contra los jar de `~/.m2`), y `tests/vectors.rs` los reproduce (se
  necesitan un JDK y Maven para volver a grabarlos, no para ejecutar las pruebas). Cubren el parser de esquemas, el
  decodificador JSON y el decodificador binario, cada caso con su resultado completo.
- **Límites de anidamiento.** `tests/limits.rs` cubre los límites de anidamiento.
- **Las pruebas unitarias de upstream.** Donde llegan al código portado, están portadas en `tests/upstream_*.rs`.
  `docs/test-coverage.md` relaciona cada clase y método de prueba de upstream que se consideró con su port, o explica
  por qué queda fuera del alcance.

## Obtener ayuda

Una pregunta sobre cómo funciona el crate puede ir primero a [DeepWiki](https://deepwiki.com/oxidestack/avro-codec-rs):
responde a partir del código y la documentación de este repositorio, y donde no coincide con la documentación, la
documentación tiene razón. Las preguntas que no resuelva van a
[Discussions](https://github.com/oxidestack/avro-codec-rs/discussions), y los errores a
[issues](https://github.com/oxidestack/avro-codec-rs/issues). <!-- these three links need the public repository; it is not published yet -->

## Licencia

El código se distribuye bajo la [Apache License, Version 2.0](LICENSE) (`Apache-2.0`); lo que porta se atribuye en
[`NOTICE`](NOTICE). La documentación —este README y las guías de `docs/`— se distribuye bajo la licencia
[Creative Commons Attribution 4.0 International](https://creativecommons.org/licenses/by/4.0/) (`CC-BY-4.0`); su texto
legal está en [`LICENSE-docs`](LICENSE-docs). Las contribuciones se firman conforme al DCO
([`CONTRIBUTING.md`](CONTRIBUTING.md)).
