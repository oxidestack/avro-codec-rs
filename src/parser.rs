//! `new Schema.Parser().parse(String)`: Jackson reads the text (comments allowed), `Schema.parse(JsonNode,
//! ParseContext, namespace)` builds the types, the `ParseContext` resolves forward references
//! (`SchemaResolver.ResolvingVisitor`), and content after the schema is refused.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicUsize, Ordering};

use jackson::JsonNode;

use crate::error::{self, Error};
use crate::java;
use crate::schema::{self, Enum, Field, Fixed, Name, Named, Order, Primitive, Props, Record, Schema, Type};
use crate::ParseOptions;

/// `SchemaResolver`'s `COUNTER`: like the JVM-wide static it ports, one sequence for the whole process, so the
/// `UnresolvedSchema_<n>` a message names depends on every schema parsed before.
static UNRESOLVED: AtomicUsize = AtomicUsize::new(0);

const SCHEMA_RESERVED: [&str; 10] = ["doc", "fields", "items", "name", "namespace", "size", "symbols", "values", "type", "aliases"];
const FIELD_RESERVED: [&str; 6] = ["default", "doc", "name", "order", "type", "aliases"];

pub(crate) fn parse(text: &str, opts: &ParseOptions<'_>) -> Result<Schema, Error> {
    let shade = |e: jackson::JacksonError| Error::parse(format!("{}{}: {}", opts.relocated_prefix, e.class, e.message));
    let (clean, open_comment) = strip_comments(text);
    let node = match jackson::json::read::read_tree(&clean, Default::default()) {
        Ok(node) => node,
        Err(e) => {
            // Jackson hits an unterminated comment before anything the blanked text would report.
            return Err(match open_comment {
                Some(at) if !jackson::json::read::read_tree(&clean[..same_units(text, at, &clean)], Default::default()).is_ok_and(|n| !n.is_missing()) => {
                    shade(eof_in_comment(text))
                }
                _ => shade(e),
            });
        }
    };
    if node.is_missing() {
        if open_comment.is_some() {
            return Err(shade(eof_in_comment(text)));
        }
        return Err(Error::parse("Cannot parse <null> schema"));
    }
    let mut p = Parser { opts, named: Vec::new(), names: HashMap::new(), put_order: Vec::new() };
    let root = p.parse(&node, None)?;
    let root = p.resolve(root)?;
    // `allowDanglingContent` is false for a string.
    let end = same_units(&clean, value_end(&clean), text);
    let dangling = java::trim(&text[end..]);
    if !dangling.is_empty() {
        return Err(Error::parse(format!("dangling content after end of schema: {dangling}")));
    }
    Ok(p.finish(root))
}

/// The text with `/* */` and `//` comments (outside strings) blanked, newlines kept so Jackson's locations do not
/// move, and the offset of an unterminated block comment.
fn strip_comments(text: &str) -> (String, Option<usize>) {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    let mut in_string = false;
    let mut open = None;
    let blank = |out: &mut String, s: &str| {
        for c in s.chars() {
            if c == '\n' || c == '\r' {
                out.push(c);
            } else {
                // Keep the UTF-16 length, as Jackson counts columns in chars.
                for _ in 0..c.len_utf16() {
                    out.push(' ');
                }
            }
        }
    };
    while i < bytes.len() {
        let c = bytes[i];
        if in_string {
            let ch = text[i..].chars().next().expect("char");
            out.push(ch);
            if c == b'\\' && i + 1 < bytes.len() {
                let next = text[i + 1..].chars().next().expect("char");
                out.push(next);
                i += 1 + next.len_utf8();
                continue;
            }
            if c == b'"' {
                in_string = false;
            }
            i += ch.len_utf8();
            continue;
        }
        if c == b'"' {
            in_string = true;
            out.push('"');
            i += 1;
            continue;
        }
        if c == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'*' {
            match text[i + 2..].find("*/") {
                Some(end) => {
                    blank(&mut out, &text[i..i + 2 + end + 2]);
                    i += 2 + end + 2;
                }
                None => {
                    open.get_or_insert(i);
                    blank(&mut out, &text[i..]);
                    i = bytes.len();
                }
            }
            continue;
        }
        if c == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
            let end = text[i..].find(['\n', '\r']).map_or(bytes.len(), |e| i + e);
            blank(&mut out, &text[i..end]);
            i = end;
            continue;
        }
        let ch = text[i..].chars().next().expect("char");
        out.push(ch);
        i += ch.len_utf8();
    }
    (out, open)
}

/// The byte offset in `to` with as many UTF-16 units before it as `from` has before `at` (blanking comments keeps
/// UTF-16 lengths, not UTF-8 ones).
fn same_units(from: &str, at: usize, to: &str) -> usize {
    let units: usize = from[..at].chars().map(char::len_utf16).sum();
    let mut seen = 0;
    for (i, c) in to.char_indices() {
        if seen >= units {
            return i;
        }
        seen += c.len_utf16();
    }
    to.len()
}

/// `_skipCComment` at end of input.
fn eof_in_comment(text: &str) -> jackson::JacksonError {
    let units: Vec<u16> = text.encode_utf16().collect();
    let (mut row, mut row_start, mut i) = (1usize, 0usize, 0usize);
    while i < units.len() {
        if units[i] == b'\n' as u16 {
            row += 1;
            row_start = i + 1;
        } else if units[i] == b'\r' as u16 {
            row += 1;
            if units.get(i + 1) == Some(&(b'\n' as u16)) {
                i += 1;
            }
            row_start = i + 1;
        }
        i += 1;
    }
    jackson::JacksonError {
        class: "com.fasterxml.jackson.core.io.JsonEOFException".into(),
        message: format!(
            "Unexpected end-of-input in a comment\n at [Source: REDACTED (`StreamReadFeature.INCLUDE_SOURCE_IN_LOCATION` disabled); line: {row}, column: {}]",
            units.len() - row_start + 1
        ),
    }
}

/// The byte offset just past the first JSON value of `text` (already known to be valid, comments blanked).
fn value_end(text: &str) -> usize {
    let b = text.as_bytes();
    let mut i = 0;
    while i < b.len() && b[i].is_ascii_whitespace() {
        i += 1;
    }
    let mut depth = 0usize;
    let mut in_string = false;
    while i < b.len() {
        let c = b[i];
        if in_string {
            if c == b'\\' {
                i += 2;
                continue;
            }
            if c == b'"' {
                in_string = false;
                if depth == 0 {
                    return i + 1;
                }
            }
            i += 1;
            continue;
        }
        match c {
            b'"' => in_string = true,
            b'{' | b'[' => depth += 1,
            b'}' | b']' => {
                depth -= 1;
                if depth == 0 {
                    return i + 1;
                }
            }
            _ if depth == 0 => {
                // A scalar at the root: up to the next delimiter.
                let start = i;
                while i < b.len() && !matches!(b[i], b' ' | b'\t' | b'\n' | b'\r' | b',' | b':' | b'{' | b'}' | b'[' | b']' | b'"' | b'/') {
                    i += 1;
                }
                return i.max(start + 1);
            }
            _ => {}
        }
        i += 1;
    }
    b.len()
}

struct Parser<'a> {
    opts: &'a ParseOptions<'a>,
    named: Vec<Named>,
    /// The `ParseContext`'s new schemas by full name (placeholders are not in it).
    names: HashMap<String, usize>,
    /// Named types in the order they were put (inner types before the type that holds them).
    put_order: Vec<usize>,
}

fn text<'n>(node: &'n JsonNode, key: &str) -> Option<&'n str> {
    node.get(key).and_then(JsonNode::text_value)
}

/// `getRequiredText(container, key, error)`.
fn required_text<'n>(node: &'n JsonNode, key: &str, error: &str) -> Result<&'n str, Error> {
    text(node, key).ok_or_else(|| Error::parse(format!("{error}: {node}")))
}

/// `Schema.validateName` (the parser's `UTF_VALIDATOR`).
fn validate_name(name: Option<&str>) -> Result<(), Error> {
    match java::name_errors(name) {
        Some(e) => Err(Error::parse(e)),
        None => Ok(()),
    }
}

/// `new Name(name, space)`.
fn make_name(name: &str, space: Option<&str>) -> Result<Name, Error> {
    let (space, simple) = match name.rfind('.') {
        Some(dot) => (Some(&name[..dot]), &name[dot + 1..]),
        None => (space, name),
    };
    validate_name(Some(simple))?;
    Ok(Name { name: simple.to_string(), space: space.filter(|s| !s.is_empty()).map(str::to_string) })
}

/// `parseAliases(node)`.
fn parse_aliases(node: &JsonNode) -> Result<Option<Vec<String>>, Error> {
    let Some(aliases) = node.get("aliases") else { return Ok(None) };
    let Some(items) = aliases.as_array() else { return Err(Error::parse(format!("aliases not an array: {node}"))) };
    let mut out: Vec<String> = Vec::new();
    for a in items.iter() {
        let Some(s) = a.text_value() else { return Err(Error::parse(format!("alias not a string: {a}"))) };
        if !out.iter().any(|x| x == s) {
            out.push(s.to_string());
        }
    }
    Ok(Some(out))
}

fn parse_props(node: &JsonNode, reserved: &[&str]) -> Props {
    let mut props = Props::default();
    if let Some(o) = node.as_object() {
        for (k, v) in o.iter() {
            if !reserved.contains(&k.as_str()) {
                props.0.push((k.clone(), v.clone()));
            }
        }
    }
    props
}

impl Parser<'_> {
    fn to_json(&self, t: &Type) -> String {
        schema::to_json(&self.named, t)
    }

    /// `ParseContext.find(name, namespace)`.
    fn find(&mut self, name: &str, space: Option<&str>) -> Type {
        if let Some(p) = Primitive::of(name) {
            return Type::primitive(p);
        }
        let full = match space {
            Some(s) if !name.contains('.') => format!("{s}.{name}"),
            _ => name.to_string(),
        };
        if let Some(i) = self.names.get(&full).or_else(|| self.names.get(name)) {
            return Type::Named(*i);
        }
        let number = UNRESOLVED.fetch_add(1, Ordering::SeqCst);
        self.named.push(Named::Unresolved { number, target: full, relocated_prefix: self.opts.relocated_prefix.to_string() });
        Type::Named(self.named.len() - 1)
    }

    /// `ParseContext.put(schema)`: full-name validation, then a second definition must equal the first.
    fn put(&mut self, named: Named) -> Result<usize, Error> {
        let full = named.full_name();
        let parts = java::split_dots(&full);
        for (i, part) in parts.iter().enumerate() {
            if let Some(e) = java::name_errors(Some(part)) {
                let kind = if i + 1 < parts.len() { "Namespace part" } else { "Name" };
                return Err(Error::parse(format!("{kind} \"{part}\" is invalid: {e}")));
            }
        }
        self.named.push(named);
        let index = self.named.len() - 1;
        if let Some(&existing) = self.names.get(&full) {
            if !schema::types_equal(&self.named, &Type::Named(existing), &self.named, &Type::Named(index), &mut HashSet::new()) {
                return Err(Error::parse(format!("Can't redefine: {full}")));
            }
            // The first definition stays; the equal second one is used where it stands.
            return Ok(index);
        }
        self.names.insert(full, index);
        self.put_order.push(index);
        Ok(index)
    }

    /// `NamedSchema`'s constructor check.
    fn not_primitive(name: &Name) -> Result<(), Error> {
        let full = name.full();
        if Primitive::of(&full).is_some() {
            return Err(Error::avro_type(format!("Schemas may not be named after primitives: {full}")));
        }
        Ok(())
    }

    fn parse_name(node: &JsonNode, space: Option<&str>) -> Result<Name, Error> {
        let space = text(node, "namespace").or(space);
        make_name(required_text(node, "name", "No name in schema")?, space)
    }

    fn named_aliases(node: &JsonNode, name: &Name) -> Result<Vec<Name>, Error> {
        let mut out: Vec<Name> = Vec::new();
        for alias in parse_aliases(node)?.unwrap_or_default() {
            let n = make_name(&alias, name.space.as_deref())?;
            if !out.contains(&n) {
                out.push(n);
            }
        }
        Ok(out)
    }

    /// `Schema.parse(JsonNode, ParseContext, currentNameSpace)`.
    fn parse(&mut self, node: &JsonNode, space: Option<&str>) -> Result<Type, Error> {
        if let Some(name) = node.text_value() {
            return Ok(self.find(name, space));
        }
        if node.is_object() {
            let kind = required_text(node, "type", "No type")?;
            if let Some(p) = Primitive::of(kind) {
                return Ok(Type::Primitive(p, parse_props(node, &SCHEMA_RESERVED)));
            }
            return match kind {
                "record" | "error" => self.record(node, space, kind == "error"),
                "enum" => self.enumeration(node, space),
                "array" => self.container(node, space, "items"),
                "map" => self.container(node, space, "values"),
                "fixed" => self.fixed(node, space),
                _ => Err(Error::parse("A schema \"type\" MUST be a primitive type or one of \"enum\", \"fixed\", \"record\", \"error\", \"array\" or \"map\".")),
            };
        }
        if let Some(items) = node.as_array() {
            return self.union_of(items, space);
        }
        Err(Error::parse(format!("Schema not yet supported: {node}")))
    }

    /// `parseArray` / `parseMap`.
    #[inline(never)]
    fn container(&mut self, node: &JsonNode, space: Option<&str>, key: &str) -> Result<Type, Error> {
        let Some(child) = node.get(key) else {
            let what = if key == "items" { "Array has no items type" } else { "Map has no values type" };
            return Err(Error::parse(format!("{what}: {node}")));
        };
        let child = Box::new(self.parse(child, space)?);
        let props = parse_props(node, &SCHEMA_RESERVED);
        Ok(if key == "items" { Type::Array(child, props) } else { Type::Map(child, props) })
    }

    #[inline(never)]
    fn union_of(&mut self, items: &[JsonNode], space: Option<&str>) -> Result<Type, Error> {
        let mut types = Vec::with_capacity(items.len());
        for item in items.iter() {
            types.push(self.parse(item, space)?);
        }
        self.union(types)
    }

    /// `new UnionSchema(types)`.
    #[inline(never)]
    fn union(&self, types: Vec<Type>) -> Result<Type, Error> {
        let mut seen: Vec<String> = Vec::new();
        for t in &types {
            if matches!(t, Type::Union(_)) {
                return Err(Error::runtime(format!("Nested union: {}", self.to_json(&Type::Union(types.clone())))));
            }
            let name = schema::full_name(&self.named, t);
            if seen.contains(&name) {
                return Err(Error::runtime(format!("Duplicate in union:{name}")));
            }
            seen.push(name);
        }
        Ok(Type::Union(types))
    }

    /// `parseRecord`. The recursion through fields keeps only small frames on the stack (the checks and the
    /// messages live in their own functions), so a schema at Jackson's nesting limit fits a debug build's stack.
    #[inline(never)]
    fn record(&mut self, node: &JsonNode, space: Option<&str>, is_error: bool) -> Result<Type, Error> {
        let (name, fields_node) = Self::record_head(node, space)?;
        let mut fields: Vec<Field> = Vec::with_capacity(fields_node.len());
        for f in fields_node.iter() {
            fields.push(self.field(f, name.space.as_deref())?);
        }
        self.record_tail(node, name, fields, is_error)
    }

    #[inline(never)]
    fn record_head<'n>(node: &'n JsonNode, space: Option<&str>) -> Result<(Name, &'n [JsonNode]), Error> {
        let name = Self::parse_name(node, space)?;
        Self::not_primitive(&name)?;
        match node.get("fields").and_then(JsonNode::as_array) {
            Some(fields) => Ok((name, fields.as_slice())),
            None => Err(Error::parse(format!("Record has no fields: {node}"))),
        }
    }

    /// `setFields`, the properties and aliases, `ParseContext.put`.
    #[inline(never)]
    fn record_tail(&mut self, node: &JsonNode, name: Name, fields: Vec<Field>, is_error: bool) -> Result<Type, Error> {
        for (position, f) in fields.iter().enumerate() {
            if let Some(existing) = fields[..position].iter().position(|x| x.name == f.name) {
                return Err(Error::runtime(format!(
                    "Duplicate field {0} in record {1}: {0} type:{2} pos:{position} and {0} type:{3} pos:{existing}.",
                    f.name,
                    name.full(),
                    schema::type_label(&self.named, &f.schema),
                    schema::type_label(&self.named, &fields[existing].schema)
                )));
            }
        }
        let doc = text(node, "doc").map(str::to_string);
        let props = parse_props(node, &SCHEMA_RESERVED);
        let aliases = Self::named_aliases(node, &name)?;
        let index = self.put(Named::Record(Record { name, doc, fields, is_error, props, aliases }))?;
        Ok(Type::Named(index))
    }

    /// `parseField` and the `Field` constructor.
    #[inline(never)]
    fn field(&mut self, node: &JsonNode, space: Option<&str>) -> Result<Field, Error> {
        let type_node = Self::field_type(node)?;
        let schema = self.parse(type_node, space)?;
        self.field_rest(node, schema)
    }

    #[inline(never)]
    fn field_type(node: &JsonNode) -> Result<&JsonNode, Error> {
        required_text(node, "name", "No field name")?;
        node.get("type").ok_or_else(|| Error::parse(format!("No field type: {node}")))
    }

    #[inline(never)]
    fn field_rest(&mut self, node: &JsonNode, schema: Type) -> Result<Field, Error> {
        let name = required_text(node, "name", "No field name")?.to_string();
        let doc = text(node, "doc").map(str::to_string);
        let order = match node.get("order") {
            None => Order::Ascending,
            Some(o) => {
                let Some(o) = o.text_value() else {
                    return Err(Error::new(
                        error::NULL_POINTER,
                        format!(
                            "Cannot invoke \"String.toUpperCase(java.util.Locale)\" because the return value of \"{}com.fasterxml.jackson.databind.JsonNode.textValue()\" is null",
                            self.opts.relocated_prefix
                        ),
                    ));
                };
                match o.to_uppercase().as_str() {
                    "ASCENDING" => Order::Ascending,
                    "DESCENDING" => Order::Descending,
                    "IGNORE" => Order::Ignore,
                    other => {
                        return Err(Error::new(error::ILLEGAL_ARGUMENT, format!("No enum constant {}org.apache.avro.Schema.Field.Order.{other}", self.opts.relocated_prefix)));
                    }
                }
            }
        };
        let mut default = node.get("default").cloned();
        if let (Some(d), Type::Primitive(Primitive::Float | Primitive::Double, _)) = (&default, &schema) {
            if let Some(t) = d.text_value() {
                default = Some(JsonNode::Double(java::parse_double(t).map_err(|m| Error::new(error::NUMBER_FORMAT, m))?));
            }
        }
        validate_name(Some(&name))?;
        if let Some(d) = &default {
            if !schema::is_valid_default(&self.named, &schema, d) {
                return Err(Error::avro_type(format!("Invalid default for field {name}: {d} not a {}", self.to_json(&schema))));
            }
        }
        let props = parse_props(node, &FIELD_RESERVED);
        let aliases = parse_aliases(node)?.unwrap_or_default();
        Ok(Field { name, doc, schema, default, order, aliases, props })
    }

    #[inline(never)]
    fn enumeration(&mut self, node: &JsonNode, space: Option<&str>) -> Result<Type, Error> {
        let name = Self::parse_name(node, space)?;
        let doc = text(node, "doc").map(str::to_string);
        let Some(symbols_node) = node.get("symbols").and_then(JsonNode::as_array) else {
            return Err(Error::parse(format!("Enum has no symbols: {node}")));
        };
        let default = node.get("default").and_then(JsonNode::text_value).map(str::to_string);
        Self::not_primitive(&name)?;
        let mut symbols: Vec<String> = Vec::with_capacity(symbols_node.len());
        for s in symbols_node.iter() {
            let s = s.text_value();
            validate_name(s)?;
            let s = s.expect("validated").to_string();
            if symbols.contains(&s) {
                return Err(Error::parse(format!("Duplicate enum symbol: {s}")));
            }
            symbols.push(s);
        }
        if let Some(d) = &default {
            if !symbols.contains(d) {
                return Err(Error::parse(format!("The Enum Default: {d} is not in the enum symbol set: [{}]", symbols.join(", "))));
            }
        }
        let mut reserved = SCHEMA_RESERVED.to_vec();
        reserved.push("default");
        let props = parse_props(node, &reserved);
        let aliases = Self::named_aliases(node, &name)?;
        let index = self.put(Named::Enum(Enum { name, doc, symbols, default, props, aliases }))?;
        Ok(Type::Named(index))
    }

    #[inline(never)]
    fn fixed(&mut self, node: &JsonNode, space: Option<&str>) -> Result<Type, Error> {
        let name = Self::parse_name(node, space)?;
        let doc = text(node, "doc").map(str::to_string);
        let size = match node.get("size") {
            Some(JsonNode::Int(n)) => *n,
            _ => return Err(Error::parse(format!("Invalid or no size: {node}"))),
        };
        Self::not_primitive(&name)?;
        crate::binary::check_max_bytes_length(size as i64)?;
        let props = parse_props(node, &SCHEMA_RESERVED);
        let aliases = Self::named_aliases(node, &name)?;
        let index = self.put(Named::Fixed(Fixed { name, doc, size: size as usize, props, aliases }))?;
        Ok(Type::Named(index))
    }

    /// `context.commit(); context.resolve(schema)`: every named type, then the main schema, visited as
    /// `Schemas.visit` walks them; a reference to an undefined name fails, and the resolved copies are checked
    /// again (field defaults, union branches) against what the references now stand for.
    fn resolve(&mut self, root: Type) -> Result<Type, Error> {
        let resolved_named = self.named.iter().map(|n| match n {
            Named::Record(rec) => {
                let mut rec = rec.clone();
                for f in &mut rec.fields {
                    f.schema = self.resolved(&f.schema);
                }
                Named::Record(rec)
            }
            other => other.clone(),
        });
        let mut r = Resolution { replaced: HashSet::new(), fields_done: HashSet::new(), named: resolved_named.collect() };
        for i in self.put_order.clone() {
            self.visit(&mut r, &Type::Named(i))?;
        }
        match &root {
            Type::Named(i) => {
                if let Named::Unresolved { .. } = &self.named[*i] {
                    return Err(Error::new(error::NULL_POINTER, format!("Unknown schema: {}", self.named[*i].full_name())));
                }
            }
            _ => self.visit(&mut r, &root)?,
        }
        Ok(root)
    }

    /// The type a (placeholder) index stands for.
    fn target(&self, i: usize) -> usize {
        match &self.named[i] {
            Named::Unresolved { target, .. } => self.names.get(target).copied().unwrap_or(i),
            _ => i,
        }
    }

    /// A copy of `t` with every placeholder replaced by the type it names (placeholders are all defined here).
    fn resolved(&self, t: &Type) -> Type {
        match t {
            Type::Named(i) => Type::Named(self.target(*i)),
            Type::Array(items, props) => Type::Array(Box::new(self.resolved(items)), props.clone()),
            Type::Map(values, props) => Type::Map(Box::new(self.resolved(values)), props.clone()),
            Type::Union(types) => Type::Union(types.iter().map(|b| self.resolved(b)).collect()),
            p => p.clone(),
        }
    }

    fn visit(&self, r: &mut Resolution, start: &Type) -> Result<(), Error> {
        enum Work<'t> {
            Visit(&'t Type),
            After(&'t Type),
        }
        let mut visited: HashSet<usize> = HashSet::new();
        let mut stack = vec![Work::Visit(start)];
        while let Some(w) = stack.pop() {
            match w {
                Work::After(t) => self.after_visit(r, t)?,
                Work::Visit(t) => match t {
                    Type::Named(i) => {
                        if !visited.insert(*i) {
                            continue;
                        }
                        match &self.named[*i] {
                            Named::Record(rec) => {
                                r.replaced.insert(*i);
                                stack.push(Work::After(t));
                                for f in rec.fields.iter().rev() {
                                    stack.push(Work::Visit(&f.schema));
                                }
                            }
                            Named::Unresolved { target, .. } => {
                                if r.replaced.insert(*i) {
                                    let Some(&def) = self.names.get(target) else {
                                        return Err(Error::avro_type(format!("Undefined schema: {target}")));
                                    };
                                    if !r.replaced.contains(&def) {
                                        let def_type = Type::Named(def);
                                        // `replace.computeIfAbsent(resSchema, …)` visits the definition first.
                                        match &self.named[def] {
                                            Named::Record(_) => self.visit(r, &def_type)?,
                                            _ => {
                                                r.replaced.insert(def);
                                            }
                                        }
                                    }
                                }
                            }
                            _ => {
                                r.replaced.insert(*i);
                            }
                        }
                    }
                    Type::Union(types) => {
                        stack.push(Work::After(t));
                        for b in types {
                            stack.push(Work::Visit(b));
                        }
                    }
                    Type::Array(child, _) | Type::Map(child, _) => {
                        stack.push(Work::After(t));
                        stack.push(Work::Visit(child));
                    }
                    Type::Primitive(..) => {}
                },
            }
        }
        Ok(())
    }

    /// `ResolvingVisitor.afterVisitNonTerminal`: `new Field(field, resolved)` re-validates defaults,
    /// `Schema.createUnion(resolved)` re-checks the branches.
    fn after_visit(&self, r: &mut Resolution, t: &Type) -> Result<(), Error> {
        match t {
            Type::Named(i) => {
                if let Named::Record(rec) = &self.named[*i] {
                    if r.fields_done.insert(*i) {
                        for f in &rec.fields {
                            if let Some(d) = &f.default {
                                let resolved = self.resolved(&f.schema);
                                if !schema::is_valid_default(&r.named, &resolved, d) {
                                    return Err(Error::avro_type(format!("Invalid default for field {}: {d} not a {}", f.name, schema::to_json(&r.named, &resolved))));
                                }
                            }
                        }
                    }
                }
            }
            Type::Union(types) => {
                let mut seen: Vec<String> = Vec::new();
                for b in types {
                    let name = schema::full_name(&self.named, &self.resolved(b));
                    if seen.contains(&name) {
                        return Err(Error::runtime(format!("Duplicate in union:{name}")));
                    }
                    seen.push(name);
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// The parsed schema without placeholders: references point at their definitions, indices renumbered.
    fn finish(self, root: Type) -> Schema {
        let mut remap: Vec<Option<usize>> = vec![None; self.named.len()];
        let mut kept = 0;
        for (i, n) in self.named.iter().enumerate() {
            if !matches!(n, Named::Unresolved { .. }) {
                remap[i] = Some(kept);
                kept += 1;
            }
        }
        let fix = |t: &Type| -> Type {
            fn go(p: &Parser, remap: &[Option<usize>], t: &Type) -> Type {
                match t {
                    Type::Named(i) => Type::Named(remap[p.target(*i)].expect("every reference is defined")),
                    Type::Array(items, props) => Type::Array(Box::new(go(p, remap, items)), props.clone()),
                    Type::Map(values, props) => Type::Map(Box::new(go(p, remap, values)), props.clone()),
                    Type::Union(types) => Type::Union(types.iter().map(|b| go(p, remap, b)).collect()),
                    other => other.clone(),
                }
            }
            go(&self, &remap, t)
        };
        let mut named = Vec::with_capacity(kept);
        for n in &self.named {
            match n {
                Named::Unresolved { .. } => {}
                Named::Record(r) => {
                    let mut r = r.clone();
                    for f in &mut r.fields {
                        f.schema = fix(&f.schema);
                    }
                    named.push(Named::Record(r));
                }
                other => named.push(other.clone()),
            }
        }
        Schema { root: fix(&root), named }
    }
}

struct Resolution {
    replaced: HashSet<usize>,
    fields_done: HashSet<usize>,
    /// The named types with every defined reference resolved, for the checks on resolved copies.
    named: Vec<Named>,
}
