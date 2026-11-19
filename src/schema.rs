//! The schema model (`org.apache.avro.Schema`): what `Schema.toString()`, `equals` and `isValidDefault` see.
//!
//! Named types live in [`Schema::named`] and are referred to by index, so recursive schemas are plain data.

use jackson::JsonNode;
use std::collections::HashSet;

/// The primitive `Schema.Type`s.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Primitive {
    /// `null`.
    Null,
    /// `boolean`.
    Boolean,
    /// `int`: 32-bit signed.
    Int,
    /// `long`: 64-bit signed.
    Long,
    /// `float`: 32-bit IEEE 754.
    Float,
    /// `double`: 64-bit IEEE 754.
    Double,
    /// `bytes`.
    Bytes,
    /// `string`.
    String,
}

impl Primitive {
    /// `Schema.Type.getName()`.
    pub fn name(self) -> &'static str {
        match self {
            Primitive::Null => "null",
            Primitive::Boolean => "boolean",
            Primitive::Int => "int",
            Primitive::Long => "long",
            Primitive::Float => "float",
            Primitive::Double => "double",
            Primitive::Bytes => "bytes",
            Primitive::String => "string",
        }
    }

    /// `PRIMITIVES.get(name)`.
    pub fn of(name: &str) -> Option<Primitive> {
        Some(match name {
            "null" => Primitive::Null,
            "boolean" => Primitive::Boolean,
            "int" => Primitive::Int,
            "long" => Primitive::Long,
            "float" => Primitive::Float,
            "double" => Primitive::Double,
            "bytes" => Primitive::Bytes,
            "string" => Primitive::String,
            _ => return None,
        })
    }
}

/// The names of `Schema.Type`'s constants (`Name.shouldWriteFull` checks a name against them).
const TYPE_NAMES: [&str; 14] = ["record", "enum", "array", "map", "union", "fixed", "string", "bytes", "int", "long", "float", "double", "boolean", "null"];

/// `JsonProperties`: the non-reserved attributes, in insertion order; equal as maps.
#[derive(Clone, Debug, Default)]
pub struct Props(pub Vec<(String, JsonNode)>);

impl Props {
    /// `getObjectProp(name)`: the property's value, if set.
    pub fn get(&self, name: &str) -> Option<&JsonNode> {
        self.0.iter().find(|(k, _)| k == name).map(|(_, v)| v)
    }

    /// `getProp(name)`: the property's text, if it is a string.
    pub fn text(&self, name: &str) -> Option<&str> {
        self.get(name).and_then(JsonNode::text_value)
    }

    /// Whether there is no property.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl PartialEq for Props {
    fn eq(&self, other: &Props) -> bool {
        self.0.len() == other.0.len() && self.0.iter().all(|(k, v)| other.get(k) == Some(v))
    }
}

/// `Schema.Name`: a simple name and an optional namespace (`""` is no namespace).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Name {
    /// The simple name.
    pub name: String,
    /// The namespace, `None` for none.
    pub space: Option<String>,
}

impl Name {
    /// The full name (`space.name`).
    pub fn full(&self) -> String {
        match &self.space {
            Some(s) => format!("{s}.{}", self.name),
            None => self.name.clone(),
        }
    }

    /// `getQualified(defaultSpace)`.
    pub fn qualified(&self, default_space: Option<&str>) -> String {
        if self.space.is_some() && self.space.as_deref() == default_space && !TYPE_NAMES.contains(&self.name.as_str()) {
            self.name.clone()
        } else {
            self.full()
        }
    }
}

/// `Schema.Field.Order`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Order {
    /// `ASCENDING`, the default.
    Ascending,
    /// `DESCENDING`.
    Descending,
    /// `IGNORE`.
    Ignore,
}

/// `Schema.Field`: a record's field.
#[derive(Clone, Debug)]
pub struct Field {
    /// `name()`.
    pub name: String,
    /// `doc()`.
    pub doc: Option<String>,
    /// `schema()`: the field's type.
    pub schema: Type,
    /// `defaultValue()`, as the schema wrote it.
    pub default: Option<JsonNode>,
    /// `order()`.
    pub order: Order,
    /// In declaration order, duplicates dropped (a `LinkedHashSet`).
    pub aliases: Vec<String>,
    /// The field's other properties.
    pub props: Props,
}

/// A schema node: primitives, arrays, maps and unions inline, named types by index into [`Schema::named`].
#[derive(Clone, Debug)]
pub enum Type {
    /// A primitive type and its properties.
    Primitive(Primitive, Props),
    /// A named type: its index in [`Schema::named`].
    Named(usize),
    /// `array` of the items' type, and its properties.
    Array(Box<Type>, Props),
    /// `map` with string keys and the values' type, and its properties.
    Map(Box<Type>, Props),
    /// A union of its branches, in order.
    Union(Vec<Type>),
}

impl Type {
    /// The primitive type `p` without properties.
    pub fn primitive(p: Primitive) -> Type {
        Type::Primitive(p, Props::default())
    }
}

/// A `record` (or an `error`).
#[derive(Clone, Debug)]
pub struct Record {
    /// `getName()` and `getNamespace()`.
    pub name: Name,
    /// `getDoc()`.
    pub doc: Option<String>,
    /// `getFields()`, in order.
    pub fields: Vec<Field>,
    /// `isError()`: declared as `error`.
    pub is_error: bool,
    /// The type's other properties.
    pub props: Props,
    /// `getAliases()`.
    pub aliases: Vec<Name>,
}

/// An `enum`.
#[derive(Clone, Debug)]
pub struct Enum {
    /// `getName()` and `getNamespace()`.
    pub name: Name,
    /// `getDoc()`.
    pub doc: Option<String>,
    /// `getEnumSymbols()`, in order.
    pub symbols: Vec<String>,
    /// `getEnumDefault()`.
    pub default: Option<String>,
    /// The type's other properties.
    pub props: Props,
    /// `getAliases()`.
    pub aliases: Vec<Name>,
}

/// A `fixed`.
#[derive(Clone, Debug)]
pub struct Fixed {
    /// `getName()` and `getNamespace()`.
    pub name: Name,
    /// `getDoc()`.
    pub doc: Option<String>,
    /// `getFixedSize()`, in bytes.
    pub size: usize,
    /// The type's other properties.
    pub props: Props,
    /// `getAliases()`.
    pub aliases: Vec<Name>,
}

/// A named type.
#[derive(Clone, Debug)]
pub enum Named {
    /// A `record` or `error`.
    Record(Record),
    /// An `enum`.
    Enum(Enum),
    /// A `fixed`.
    Fixed(Fixed),
    /// `SchemaResolver.unresolvedSchema(name)`: a forward reference while parsing, never in a parsed [`Schema`].
    Unresolved {
        /// The placeholder's number, `UnresolvedSchema_<number>`.
        number: usize,
        /// The full name it stands for.
        target: String,
        /// [`ParseOptions::relocated_prefix`](crate::ParseOptions::relocated_prefix) of the parse.
        relocated_prefix: String,
    },
}

impl Named {
    /// `getFullName()`.
    pub fn full_name(&self) -> String {
        match self {
            Named::Record(r) => r.name.full(),
            Named::Enum(e) => e.name.full(),
            Named::Fixed(f) => f.name.full(),
            Named::Unresolved { number, relocated_prefix, .. } => format!("{relocated_prefix}{UNRESOLVED_NS}.{UNRESOLVED_NAME}_{number}"),
        }
    }

    fn name(&self) -> Name {
        match self {
            Named::Record(r) => r.name.clone(),
            Named::Enum(e) => e.name.clone(),
            Named::Fixed(f) => f.name.clone(),
            Named::Unresolved { number, relocated_prefix, .. } => Name { name: format!("{UNRESOLVED_NAME}_{number}"), space: Some(format!("{relocated_prefix}{UNRESOLVED_NS}")) },
        }
    }
}

pub(crate) const UNRESOLVED_NAME: &str = "UnresolvedSchema";
pub(crate) const UNRESOLVED_NS: &str = "org.apache.avro.compiler";
pub(crate) const UNRESOLVED_ATTR: &str = "org.apache.avro.idl.unresolved.name";

/// A parsed schema (`new Schema.Parser().parse(json)`).
#[derive(Clone, Debug)]
pub struct Schema {
    /// The schema's own type.
    pub root: Type,
    /// Every named type it defines, referred to by index from [`Type::Named`].
    pub named: Vec<Named>,
}

/// `Schema.Type` as its constant name (`INT`, `RECORD`, …), as `Field.toString()` prints it.
pub(crate) fn type_label(named: &[Named], t: &Type) -> &'static str {
    match t {
        Type::Primitive(p, _) => match p {
            Primitive::Null => "NULL",
            Primitive::Boolean => "BOOLEAN",
            Primitive::Int => "INT",
            Primitive::Long => "LONG",
            Primitive::Float => "FLOAT",
            Primitive::Double => "DOUBLE",
            Primitive::Bytes => "BYTES",
            Primitive::String => "STRING",
        },
        Type::Named(i) => match &named[*i] {
            Named::Record(_) | Named::Unresolved { .. } => "RECORD",
            Named::Enum(_) => "ENUM",
            Named::Fixed(_) => "FIXED",
        },
        Type::Array(..) => "ARRAY",
        Type::Map(..) => "MAP",
        Type::Union(_) => "UNION",
    }
}

/// `Schema.getFullName()`: the name for named types, the type name otherwise (a union's is `union`).
pub(crate) fn full_name(named: &[Named], t: &Type) -> String {
    match t {
        Type::Primitive(p, _) => p.name().into(),
        Type::Named(i) => named[*i].full_name(),
        Type::Array(..) => "array".into(),
        Type::Map(..) => "map".into(),
        Type::Union(_) => "union".into(),
    }
}

impl Schema {
    /// The named type `t` refers to, if it is one.
    pub fn named_of(&self, t: &Type) -> Option<&Named> {
        match t {
            Type::Named(i) => self.named.get(*i),
            _ => None,
        }
    }

    /// The record `t` refers to, if it is one.
    pub fn record_of(&self, t: &Type) -> Option<&Record> {
        match self.named_of(t) {
            Some(Named::Record(r)) => Some(r),
            _ => None,
        }
    }

    /// `getFullName()` of `t`: the name of a named type, the type's name otherwise (`int`, `array`, `union`).
    pub fn full_name(&self, t: &Type) -> String {
        full_name(&self.named, t)
    }

    /// `getName()` of `t`: the simple name of a named type, the type's name otherwise.
    pub fn name(&self, t: &Type) -> String {
        match self.named_of(t) {
            Some(n) => n.name().name,
            None => full_name(&self.named, t),
        }
    }

    /// `Schema.Type` of `t` as its constant name (`RECORD`, `INT`, …).
    pub fn type_label(&self, t: &Type) -> &'static str {
        type_label(&self.named, t)
    }

    /// `toString()` of the type `t`.
    pub fn type_to_string(&self, t: &Type) -> String {
        to_json(&self.named, t)
    }

    /// `equals` of two types of this schema.
    pub fn types_equal(&self, a: &Type, b: &Type) -> bool {
        types_equal(&self.named, a, &self.named, b, &mut HashSet::new())
    }

    /// `Schema.isValidDefault(value)` for the type `t`.
    pub fn is_valid_default(&self, t: &Type, value: &JsonNode) -> bool {
        is_valid_default(&self.named, t, value)
    }
}

impl Record {
    /// `getField(name)`.
    pub fn field(&self, name: &str) -> Option<&Field> {
        self.fields.iter().find(|f| f.name == name)
    }
}

/// `Schema.equals`.
impl PartialEq for Schema {
    fn eq(&self, other: &Schema) -> bool {
        types_equal(&self.named, &self.root, &other.named, &other.root, &mut HashSet::new())
    }
}

/// `Schema.toString()`: compact JSON with each named type written in full the first time it appears.
impl std::fmt::Display for Schema {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&to_json(&self.named, &self.root))
    }
}

/// `Schema.toString()` of one type.
pub(crate) fn to_json(named: &[Named], t: &Type) -> String {
    let mut out = String::new();
    write_json(named, t, &mut HashSet::new(), None, &mut out);
    out
}

fn quote(out: &mut String, s: &str) {
    jackson::json::write::quote_into(out, s);
}

/// `"name":value` after an opening brace or a previous member.
fn member(out: &mut String, name: &str) {
    if !out.ends_with('{') && !out.ends_with('[') {
        out.push(',');
    }
    quote(out, name);
    out.push(':');
}

fn write_props(out: &mut String, props: &Props) {
    for (k, v) in &props.0 {
        member(out, k);
        out.push_str(&v.to_java_string());
    }
}

/// `Name.writeName(currentNamespace, gen)`.
fn write_name(out: &mut String, name: &Name, namespace: Option<&str>) {
    member(out, "name");
    quote(out, &name.name);
    match &name.space {
        Some(space) if Some(space.as_str()) != namespace => {
            member(out, "namespace");
            quote(out, space);
        }
        None if namespace.is_some() => {
            member(out, "namespace");
            quote(out, "");
        }
        _ => {}
    }
}

fn write_aliases(out: &mut String, aliases: &[Name], space: Option<&str>) {
    if aliases.is_empty() {
        return;
    }
    member(out, "aliases");
    out.push('[');
    for (i, a) in aliases.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        quote(out, &a.qualified(space));
    }
    out.push(']');
}

/// `toJson(knownNames, namespace, gen)`.
pub(crate) fn write_json(named: &[Named], t: &Type, known: &mut HashSet<String>, namespace: Option<&str>, out: &mut String) {
    match t {
        Type::Primitive(p, props) => {
            if props.is_empty() {
                quote(out, p.name());
            } else {
                out.push('{');
                member(out, "type");
                quote(out, p.name());
                write_props(out, props);
                out.push('}');
            }
        }
        Type::Array(items, props) => {
            out.push('{');
            member(out, "type");
            quote(out, "array");
            member(out, "items");
            write_json(named, items, known, namespace, out);
            write_props(out, props);
            out.push('}');
        }
        Type::Map(values, props) => {
            out.push('{');
            member(out, "type");
            quote(out, "map");
            member(out, "values");
            write_json(named, values, known, namespace, out);
            write_props(out, props);
            out.push('}');
        }
        Type::Union(types) => {
            out.push('[');
            for (i, b) in types.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_json(named, b, known, namespace, out);
            }
            out.push(']');
        }
        Type::Named(i) => {
            let n = &named[*i];
            let name = n.name();
            // `writeNameRef`: a name already written is a reference.
            if !known.insert(name.full()) {
                quote(out, &name.qualified(namespace));
                return;
            }
            out.push('{');
            match n {
                Named::Record(r) => {
                    member(out, "type");
                    quote(out, if r.is_error { "error" } else { "record" });
                    write_name(out, &r.name, namespace);
                    if let Some(doc) = &r.doc {
                        member(out, "doc");
                        quote(out, doc);
                    }
                    member(out, "fields");
                    write_fields(named, &r.fields, known, r.name.space.as_deref(), out);
                    write_props(out, &r.props);
                    write_aliases(out, &r.aliases, r.name.space.as_deref());
                }
                Named::Unresolved { target, relocated_prefix, .. } => {
                    member(out, "type");
                    quote(out, "record");
                    write_name(out, &name, namespace);
                    member(out, "doc");
                    quote(out, "unresolved schema");
                    member(out, "fields");
                    out.push_str("[]");
                    member(out, &format!("{relocated_prefix}{UNRESOLVED_ATTR}"));
                    quote(out, target);
                }
                Named::Enum(e) => {
                    member(out, "type");
                    quote(out, "enum");
                    write_name(out, &e.name, namespace);
                    if let Some(doc) = &e.doc {
                        member(out, "doc");
                        quote(out, doc);
                    }
                    member(out, "symbols");
                    out.push('[');
                    for (i, s) in e.symbols.iter().enumerate() {
                        if i > 0 {
                            out.push(',');
                        }
                        quote(out, s);
                    }
                    out.push(']');
                    if let Some(d) = &e.default {
                        member(out, "default");
                        quote(out, d);
                    }
                    write_props(out, &e.props);
                    write_aliases(out, &e.aliases, e.name.space.as_deref());
                }
                Named::Fixed(f) => {
                    member(out, "type");
                    quote(out, "fixed");
                    write_name(out, &f.name, namespace);
                    if let Some(doc) = &f.doc {
                        member(out, "doc");
                        quote(out, doc);
                    }
                    member(out, "size");
                    out.push_str(&f.size.to_string());
                    write_props(out, &f.props);
                    write_aliases(out, &f.aliases, f.name.space.as_deref());
                }
            }
            out.push('}');
        }
    }
}

fn write_fields(named: &[Named], fields: &[Field], known: &mut HashSet<String>, namespace: Option<&str>, out: &mut String) {
    out.push('[');
    for (i, f) in fields.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push('{');
        member(out, "name");
        quote(out, &f.name);
        member(out, "type");
        write_json(named, &f.schema, known, namespace, out);
        if let Some(doc) = &f.doc {
            member(out, "doc");
            quote(out, doc);
        }
        if let Some(d) = &f.default {
            member(out, "default");
            out.push_str(&d.to_java_string());
        }
        match f.order {
            Order::Ascending => {}
            Order::Descending => {
                member(out, "order");
                quote(out, "descending");
            }
            Order::Ignore => {
                member(out, "order");
                quote(out, "ignore");
            }
        }
        if !f.aliases.is_empty() {
            member(out, "aliases");
            out.push('[');
            for (i, a) in f.aliases.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                quote(out, a);
            }
            out.push(']');
        }
        write_props(out, &f.props);
        out.push('}');
    }
    out.push(']');
}

/// `Schema.equals`: named types by name and content (recursion cut on a pair already being compared), props
/// as maps; docs and aliases do not count. `a` is a type of `na`'s named types, `b` of `nb`'s.
pub(crate) fn types_equal(na: &[Named], a: &Type, nb: &[Named], b: &Type, seen: &mut HashSet<(usize, usize)>) -> bool {
    match (a, b) {
        (Type::Primitive(p, x), Type::Primitive(q, y)) => p == q && x == y,
        (Type::Array(i, x), Type::Array(j, y)) | (Type::Map(i, x), Type::Map(j, y)) => types_equal(na, i, nb, j, seen) && x == y,
        (Type::Union(x), Type::Union(y)) => x.len() == y.len() && x.iter().zip(y).all(|(s, t)| types_equal(na, s, nb, t, seen)),
        (Type::Named(i), Type::Named(j)) => {
            if (std::ptr::eq(na, nb) && i == j) || !seen.insert((*i, *j)) {
                return true;
            }
            named_equal(na, *i, nb, *j, seen)
        }
        _ => false,
    }
}

fn named_equal(na: &[Named], i: usize, nb: &[Named], j: usize, seen: &mut HashSet<(usize, usize)>) -> bool {
    match (&na[i], &nb[j]) {
        (Named::Record(a), Named::Record(b)) => {
            a.name.full() == b.name.full()
                && a.props == b.props
                && a.fields.len() == b.fields.len()
                && a.fields.iter().zip(&b.fields).all(|(f, g)| {
                    f.name == g.name && types_equal(na, &f.schema, nb, &g.schema, seen) && default_equal(&f.default, &g.default) && f.order == g.order && f.props == g.props
                })
        }
        (Named::Enum(a), Named::Enum(b)) => a.name.full() == b.name.full() && a.symbols == b.symbols && a.props == b.props,
        (Named::Fixed(a), Named::Fixed(b)) => a.name.full() == b.name.full() && a.size == b.size && a.props == b.props,
        (Named::Unresolved { number: a, .. }, Named::Unresolved { number: b, .. }) => a == b,
        _ => false,
    }
}

/// `Field.defaultValueEquals`: `NaN` equals `NaN` by `doubleValue()`.
fn default_equal(a: &Option<JsonNode>, b: &Option<JsonNode>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(x), Some(y)) => {
            if x.double_value().is_nan() {
                y.double_value().is_nan()
            } else {
                x == y
            }
        }
        _ => false,
    }
}

/// `Schema.isValidDefault(schema, value)`.
pub(crate) fn is_valid_default(named: &[Named], t: &Type, value: &JsonNode) -> bool {
    match t {
        Type::Primitive(p, _) => match p {
            Primitive::String | Primitive::Bytes => value.is_textual(),
            Primitive::Int => value.is_integral_number() && value.can_convert_to_int(),
            Primitive::Long => value.is_integral_number() && value.can_convert_to_long(),
            Primitive::Float | Primitive::Double => value.is_number(),
            Primitive::Boolean => value.is_boolean(),
            Primitive::Null => value.is_null(),
        },
        Type::Array(items, _) => value.as_array().is_some_and(|a| a.iter().all(|v| is_valid_default(named, items, v))),
        Type::Map(values, _) => value.as_object().is_some_and(|o| o.values().all(|v| is_valid_default(named, values, v))),
        Type::Union(types) => types.iter().any(|b| is_valid_value(named, b, Some(value))),
        Type::Named(i) => match &named[*i] {
            Named::Enum(_) | Named::Fixed(_) => value.is_textual(),
            Named::Record(r) => value.as_object().is_some_and(|o| r.fields.iter().all(|f| is_valid_value(named, &f.schema, o.get(&f.name).or(f.default.as_ref())))),
            Named::Unresolved { .. } => value.is_object(),
        },
    }
}

/// `Schema.isValidValue`.
fn is_valid_value(named: &[Named], t: &Type, value: Option<&JsonNode>) -> bool {
    let Some(value) = value else { return false };
    match t {
        Type::Union(types) => types.iter().any(|b| is_valid_default(named, b, value)),
        _ => is_valid_default(named, t, value),
    }
}

/// `GenericDatumReader.minBytesPerElement`: the fewest bytes a value takes in the binary encoding.
pub(crate) fn min_bytes(named: &[Named], t: &Type, visiting: &mut Vec<usize>) -> i64 {
    match t {
        Type::Primitive(Primitive::Null, _) => 0,
        Type::Primitive(Primitive::Float, _) => 4,
        Type::Primitive(Primitive::Double, _) => 8,
        Type::Named(i) => match &named[*i] {
            Named::Fixed(f) => f.size as i64,
            Named::Record(r) => {
                if visiting.contains(i) {
                    return 0;
                }
                visiting.push(*i);
                let mut sum = 0i64;
                for f in &r.fields {
                    sum += min_bytes(named, &f.schema, visiting);
                    if sum >= i32::MAX as i64 {
                        sum = i32::MAX as i64;
                        break;
                    }
                }
                visiting.pop();
                sum
            }
            _ => 1,
        },
        _ => 1,
    }
}
