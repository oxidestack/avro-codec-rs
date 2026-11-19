//! The Java exception a failure corresponds to.

/// `org.apache.avro.SchemaParseException`
pub const SCHEMA_PARSE: &str = "org.apache.avro.SchemaParseException";
/// `org.apache.avro.AvroTypeException`
pub const AVRO_TYPE: &str = "org.apache.avro.AvroTypeException";
/// `org.apache.avro.AvroRuntimeException`
pub const AVRO_RUNTIME: &str = "org.apache.avro.AvroRuntimeException";
/// `org.apache.avro.SystemLimitException`
pub const SYSTEM_LIMIT: &str = "org.apache.avro.SystemLimitException";
/// `org.apache.avro.InvalidNumberEncodingException`
pub const INVALID_NUMBER_ENCODING: &str = "org.apache.avro.InvalidNumberEncodingException";
/// `java.io.EOFException`
pub const EOF: &str = "java.io.EOFException";
/// `java.lang.NullPointerException`
pub const NULL_POINTER: &str = "java.lang.NullPointerException";
/// `java.lang.NumberFormatException`
pub const NUMBER_FORMAT: &str = "java.lang.NumberFormatException";
/// `java.lang.IllegalArgumentException`
pub const ILLEGAL_ARGUMENT: &str = "java.lang.IllegalArgumentException";
/// `java.lang.UnsupportedOperationException`
pub const UNSUPPORTED_OPERATION: &str = "java.lang.UnsupportedOperationException";

/// A failure, as the Java exception Avro (or Jackson underneath it) throws: its fully qualified class and
/// `getMessage()`, `None` where Java's message is `null`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error {
    /// The fully qualified Java class, e.g. `org.apache.avro.SchemaParseException` or Jackson's
    /// `com.fasterxml.jackson.core.JsonParseException`. Never relocated: [`crate::ParseOptions::relocated_prefix`]
    /// applies to class names inside messages only.
    pub class: String,
    /// `getMessage()`.
    pub message: Option<String>,
}

impl Error {
    /// The exception `class` with this message.
    pub fn new(class: &str, message: impl Into<String>) -> Error {
        Error { class: class.to_string(), message: Some(message.into()) }
    }

    /// An exception whose message is `null` (`new EOFException()`).
    pub fn without_message(class: &str) -> Error {
        Error { class: class.to_string(), message: None }
    }

    /// `Throwable.toString()`: `class: message`, or the class alone when the message is `null`.
    pub fn java_to_string(&self) -> String {
        match &self.message {
            Some(m) => format!("{}: {m}", self.class),
            None => self.class.clone(),
        }
    }

    /// The simple class name (`SchemaParseException`).
    pub fn simple_class(&self) -> &str {
        self.class.rsplit('.').next().unwrap_or(&self.class)
    }

    pub(crate) fn parse(message: impl Into<String>) -> Error {
        Error::new(SCHEMA_PARSE, message)
    }

    pub(crate) fn avro_type(message: impl Into<String>) -> Error {
        Error::new(AVRO_TYPE, message)
    }

    pub(crate) fn runtime(message: impl Into<String>) -> Error {
        Error::new(AVRO_RUNTIME, message)
    }

    pub(crate) fn eof() -> Error {
        Error::without_message(EOF)
    }
}

/// The message, as string concatenation prints it (`"null"` for a `null` message).
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.message.as_deref().unwrap_or("null"))
    }
}

impl std::error::Error for Error {}

impl From<jackson::JacksonError> for Error {
    fn from(e: jackson::JacksonError) -> Error {
        Error { class: e.class, message: Some(e.message) }
    }
}
