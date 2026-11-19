//! `JsonDecoder`: Avro's JSON encoding read through Jackson's streaming parser, driven by the grammar
//! `JsonGrammarGenerator` builds (`Parser` with its implicit actions: record start and end, field reordering through
//! `TokenBuffer`s, union ends).
//!
//! Jackson's parser is lazy, and so is this one: a token is read when the decoder asks for it, so malformed JSON past
//! what the datum needs is never seen, as with upstream. Syntax errors carry Jackson's exact message, taken from the
//! `jackson` crate's reader.

use std::collections::HashMap;

use crate::error::Error;
use crate::java;
use crate::jhashmap::JavaHashMap;
use crate::schema::{self, Named, Primitive, Schema, Type};

/// `StreamReadConstraints.getMaxNestingDepth()`.
const MAX_NESTING_DEPTH: usize = 1000;

// ---- tokens (Jackson's `JsonToken`) ----

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    StartObject,
    EndObject,
    StartArray,
    EndArray,
    FieldName,
    Str,
    Int,
    Float,
    True,
    False,
    Null,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Kind::StartObject => "START_OBJECT",
            Kind::EndObject => "END_OBJECT",
            Kind::StartArray => "START_ARRAY",
            Kind::EndArray => "END_ARRAY",
            Kind::FieldName => "FIELD_NAME",
            Kind::Str => "VALUE_STRING",
            Kind::Int => "VALUE_NUMBER_INT",
            Kind::Float => "VALUE_NUMBER_FLOAT",
            Kind::True => "VALUE_TRUE",
            Kind::False => "VALUE_FALSE",
            Kind::Null => "VALUE_NULL",
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Tok {
    kind: Kind,
    /// A field name's or string's UTF-16 text; a number's digits (ASCII).
    text: Vec<u16>,
    /// A string whose content is malformed (Jackson reports it when the string is read or skipped).
    bad: bool,
    /// Line and column just past a number, for `InputCoercionException` (`None` when replayed from a buffer).
    loc: Option<(usize, usize)>,
}

impl Tok {
    fn new(kind: Kind) -> Tok {
        Tok { kind, text: Vec::new(), bad: false, loc: None }
    }

    fn number_text(&self) -> String {
        String::from_utf16_lossy(&self.text)
    }
}

/// A source of tokens: the streaming parser or a `TokenBuffer` being replayed.
trait Source {
    fn current(&self) -> Option<&Tok>;
    fn next_token(&mut self) -> Result<(), Error>;
    /// `getText()` of the current field name or string.
    fn text(&self) -> Result<Vec<u16>, Error>;

    fn kind(&self) -> Option<Kind> {
        self.current().map(|t| t.kind)
    }

    /// `TokenBuffer.copyCurrentStructure(parser)`: the current value, the parser left on its last token.
    fn copy_current_structure(&mut self) -> Result<Vec<Tok>, Error> {
        let mut out = Vec::new();
        let mut depth = 0i32;
        while let Some(t) = self.current() {
            let mut t = t.clone();
            if matches!(t.kind, Kind::Str | Kind::FieldName) {
                t.text = self.text()?;
            }
            t.loc = None;
            match t.kind {
                Kind::StartArray | Kind::StartObject => depth += 1,
                Kind::EndArray | Kind::EndObject => depth -= 1,
                _ => {}
            }
            let is_name = t.kind == Kind::FieldName;
            out.push(t);
            if depth <= 0 && !is_name {
                break;
            }
            self.next_token()?;
        }
        Ok(out)
    }
}

/// `TokenBuffer.asParser()`.
struct Replay {
    tokens: std::vec::IntoIter<Tok>,
    current: Option<Tok>,
}

impl Source for Replay {
    fn current(&self) -> Option<&Tok> {
        self.current.as_ref()
    }

    fn next_token(&mut self) -> Result<(), Error> {
        self.current = self.tokens.next();
        Ok(())
    }

    fn text(&self) -> Result<Vec<u16>, Error> {
        Ok(self.current.as_ref().map(|t| t.text.clone()).unwrap_or_default())
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CtxKind {
    Root,
    Array,
    Object,
}

struct Ctx {
    kind: CtxKind,
    entries: usize,
}

/// `ReaderBasedJsonParser` with `JsonFactory`'s defaults: token by token over the UTF-16 text.
struct Scanner {
    text: String,
    buf: Vec<u16>,
    ptr: usize,
    row: usize,
    row_start: usize,
    ctx: Vec<Ctx>,
    /// The value token read with a field name (`_nextToken`).
    pending: Option<Tok>,
    current: Option<Tok>,
    /// Where the current root-level value starts.
    root_start: usize,
}

const WS: [u16; 4] = [b' ' as u16, b'\t' as u16, b'\n' as u16, b'\r' as u16];

fn is_digit(u: u16) -> bool {
    (b'0' as u16..=b'9' as u16).contains(&u)
}

impl Scanner {
    fn new(text: &str) -> Scanner {
        Scanner {
            text: text.to_string(),
            buf: text.encode_utf16().collect(),
            ptr: 0,
            row: 1,
            row_start: 0,
            ctx: vec![Ctx { kind: CtxKind::Root, entries: 0 }],
            pending: None,
            current: None,
            root_start: 0,
        }
    }

    /// The syntax error Jackson reports here: the `jackson` crate's reader over the current root value (earlier
    /// root values blanked, so locations stay put).
    fn syntax_error(&self) -> Error {
        let masked: String = if self.root_start == 0 {
            self.text.clone()
        } else {
            let mut units: Vec<u16> = self.buf[..self.root_start].iter().map(|&u| if u == b'\n' as u16 || u == b'\r' as u16 { u } else { b' ' as u16 }).collect();
            units.extend_from_slice(&self.buf[self.root_start..]);
            String::from_utf16_lossy(&units)
        };
        match jackson::json::read::read_tree(&masked, Default::default()) {
            Err(e) => e.into(),
            Ok(_) => Error::new("com.fasterxml.jackson.core.JsonParseException", "Unexpected token"),
        }
    }

    fn peek(&self) -> Option<u16> {
        self.buf.get(self.ptr).copied()
    }

    /// `_skipWSOrEnd`: the next significant unit, consumed.
    fn skip_ws(&mut self) -> Result<Option<u16>, Error> {
        while let Some(u) = self.peek() {
            self.ptr += 1;
            if u > 32 {
                return Ok(Some(u));
            }
            match u {
                0x20 | 0x09 => {}
                0x0a => {
                    self.row += 1;
                    self.row_start = self.ptr;
                }
                0x0d => {
                    if self.peek() == Some(0x0a) {
                        self.ptr += 1;
                    }
                    self.row += 1;
                    self.row_start = self.ptr;
                }
                _ => return Err(self.syntax_error()),
            }
        }
        Ok(None)
    }

    /// A string's content after its opening quote; `None` when malformed (unterminated, a bad escape, an
    /// unescaped control character).
    fn string(&mut self) -> Option<Vec<u16>> {
        let mut out = Vec::new();
        loop {
            let u = self.peek()?;
            self.ptr += 1;
            match u {
                0x22 => return Some(out),
                0x5c => {
                    let e = self.peek()?;
                    self.ptr += 1;
                    let v = match e {
                        0x22 | 0x5c | 0x2f => e,
                        0x62 => 0x08,
                        0x66 => 0x0c,
                        0x6e => 0x0a,
                        0x72 => 0x0d,
                        0x74 => 0x09,
                        0x75 => {
                            let hex = self.buf.get(self.ptr..self.ptr + 4)?;
                            let s = String::from_utf16(hex).ok()?;
                            if !s.bytes().all(|b| b.is_ascii_hexdigit()) {
                                return None;
                            }
                            self.ptr += 4;
                            u16::from_str_radix(&s, 16).ok()?
                        }
                        _ => return None,
                    };
                    out.push(v);
                }
                u if u < 0x20 => return None,
                u => out.push(u),
            }
        }
    }

    /// A value token starting with `u` (already consumed).
    fn value(&mut self, u: u16) -> Result<Tok, Error> {
        match u {
            0x7b => Ok(Tok::new(Kind::StartObject)),
            0x5b => Ok(Tok::new(Kind::StartArray)),
            0x22 => {
                let mut t = Tok::new(Kind::Str);
                match self.string() {
                    Some(text) => t.text = text,
                    None => t.bad = true,
                }
                Ok(t)
            }
            0x2d | 0x30..=0x39 => self.number(u),
            0x74 => self.literal("true", Kind::True),
            0x66 => self.literal("false", Kind::False),
            0x6e => self.literal("null", Kind::Null),
            _ => Err(self.syntax_error()),
        }
    }

    fn literal(&mut self, word: &str, kind: Kind) -> Result<Tok, Error> {
        for b in word.bytes().skip(1) {
            if self.peek() != Some(b as u16) {
                return Err(self.syntax_error());
            }
            self.ptr += 1;
        }
        // `_checkMatchEnd`: a following identifier character makes it an unrecognized token.
        if let Some(u) = self.peek() {
            if char::from_u32(u as u32).is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '$') {
                return Err(self.syntax_error());
            }
        }
        Ok(Tok::new(kind))
    }

    /// A number cut short by the end of input, as Jackson words it (the `jackson` crate's reader words these
    /// three differently).
    fn number_eof(&self, class: &str, message: &str, back: usize) -> Error {
        let column = self.ptr - self.row_start + 1 - back;
        Error::new(
            class,
            format!("{message}\n at [Source: REDACTED (`StreamReadFeature.INCLUDE_SOURCE_IN_LOCATION` disabled); line: {}, column: {column}]", self.row),
        )
    }

    fn number(&mut self, first: u16) -> Result<Tok, Error> {
        const EOF: &str = "com.fasterxml.jackson.core.io.JsonEOFException";
        let start = self.ptr - 1;
        let mut u = first;
        if u == b'-' as u16 {
            match self.peek() {
                Some(d) if is_digit(d) => {
                    u = d;
                    self.ptr += 1;
                }
                None => return Err(self.number_eof(EOF, "Unexpected end-of-inputNo digit following sign", 0)),
                _ => return Err(self.syntax_error()),
            }
        }
        if u == b'0' as u16 && self.peek().is_some_and(is_digit) {
            return Err(self.syntax_error());
        }
        while self.peek().is_some_and(is_digit) {
            self.ptr += 1;
        }
        let mut float = false;
        if self.peek() == Some(b'.' as u16) {
            self.ptr += 1;
            if self.peek().is_none() {
                return Err(self.number_eof(
                    "com.fasterxml.jackson.core.JsonParseException",
                    "Unexpected character ('.' (code 46)) in numeric value: Decimal point not followed by a digit",
                    1,
                ));
            }
            if !self.peek().is_some_and(is_digit) {
                return Err(self.syntax_error());
            }
            while self.peek().is_some_and(is_digit) {
                self.ptr += 1;
            }
            float = true;
        }
        if matches!(self.peek(), Some(0x65 | 0x45)) {
            self.ptr += 1;
            if matches!(self.peek(), Some(0x2b | 0x2d)) {
                self.ptr += 1;
            }
            if self.peek().is_none() {
                return Err(self.number_eof(EOF, "Unexpected end-of-inputexpected a digit for number exponent", 0));
            }
            if !self.peek().is_some_and(is_digit) {
                return Err(self.syntax_error());
            }
            while self.peek().is_some_and(is_digit) {
                self.ptr += 1;
            }
            float = true;
        }
        // `_verifyRootSpace`: a root-level number must be followed by white space.
        if self.ctx.last().is_some_and(|c| c.kind == CtxKind::Root) {
            if let Some(next) = self.peek() {
                if !WS.contains(&next) {
                    return Err(self.syntax_error());
                }
            }
        }
        let mut t = Tok::new(if float { Kind::Float } else { Kind::Int });
        t.text = self.buf[start..self.ptr].to_vec();
        t.loc = Some((self.row, self.ptr - self.row_start + 1));
        Ok(t)
    }

    /// Makes `t` current, opening a context for a start token.
    fn set_current(&mut self, t: Tok) -> Result<(), Error> {
        match t.kind {
            Kind::StartObject | Kind::StartArray => {
                if self.ctx.len() > MAX_NESTING_DEPTH {
                    return Err(self.syntax_error());
                }
                self.ctx.push(Ctx { kind: if t.kind == Kind::StartObject { CtxKind::Object } else { CtxKind::Array }, entries: 0 });
            }
            _ => {}
        }
        self.current = Some(t);
        Ok(())
    }

    fn close(&mut self, u: u16) -> Result<(), Error> {
        let want = if u == b'}' as u16 { CtxKind::Object } else { CtxKind::Array };
        if self.ctx.last().map(|c| c.kind) != Some(want) {
            return Err(self.syntax_error());
        }
        self.ctx.pop();
        self.current = Some(Tok::new(if want == CtxKind::Object { Kind::EndObject } else { Kind::EndArray }));
        Ok(())
    }
}

impl Source for Scanner {
    fn current(&self) -> Option<&Tok> {
        self.current.as_ref()
    }

    fn text(&self) -> Result<Vec<u16>, Error> {
        match &self.current {
            Some(t) if t.bad => Err(self.syntax_error()),
            Some(t) => Ok(t.text.clone()),
            None => Ok(Vec::new()),
        }
    }

    fn next_token(&mut self) -> Result<(), Error> {
        if self.current.as_ref().is_some_and(|t| t.bad) {
            return Err(self.syntax_error());
        }
        if let Some(t) = self.pending.take() {
            return self.set_current(t);
        }
        let u = self.skip_ws()?;
        let top = self.ctx.last().expect("root context").kind;
        match top {
            CtxKind::Root => {
                let Some(u) = u else {
                    self.current = None;
                    return Ok(());
                };
                self.root_start = self.ptr - 1;
                self.ctx[0].entries += 1;
                let t = self.value(u)?;
                self.set_current(t)
            }
            CtxKind::Array => {
                let Some(mut u) = u else { return Err(self.syntax_error()) };
                if u == b']' as u16 {
                    return self.close(u);
                }
                if self.ctx.last().expect("array").entries > 0 {
                    if u != b',' as u16 {
                        return Err(self.syntax_error());
                    }
                    u = self.skip_ws()?.ok_or_else(|| self.syntax_error())?;
                }
                self.ctx.last_mut().expect("array").entries += 1;
                let t = self.value(u)?;
                self.set_current(t)
            }
            CtxKind::Object => {
                let Some(mut u) = u else { return Err(self.syntax_error()) };
                if u == b'}' as u16 {
                    return self.close(u);
                }
                if self.ctx.last().expect("object").entries > 0 {
                    if u != b',' as u16 {
                        return Err(self.syntax_error());
                    }
                    u = self.skip_ws()?.ok_or_else(|| self.syntax_error())?;
                }
                if u != b'"' as u16 {
                    return Err(self.syntax_error());
                }
                let Some(name) = self.string() else { return Err(self.syntax_error()) };
                if self.skip_ws()? != Some(b':' as u16) {
                    return Err(self.syntax_error());
                }
                let Some(v) = self.skip_ws()? else { return Err(self.syntax_error()) };
                let value = self.value(v)?;
                self.pending = Some(value);
                self.ctx.last_mut().expect("object").entries += 1;
                let mut t = Tok::new(Kind::FieldName);
                t.text = name;
                self.current = Some(t);
                Ok(())
            }
        }
    }
}

// ---- the grammar (`Symbol`) ----

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Terminal {
    Null,
    Boolean,
    Int,
    Long,
    Float,
    Double,
    String,
    Bytes,
    Fixed,
    Enum,
    Union,
    ArrayStart,
    ArrayEnd,
    MapStart,
    MapEnd,
    ItemEnd,
    MapKeyMarker,
}

impl Terminal {
    fn name(self) -> &'static str {
        match self {
            Terminal::Null => "null",
            Terminal::Boolean => "boolean",
            Terminal::Int => "int",
            Terminal::Long => "long",
            Terminal::Float => "float",
            Terminal::Double => "double",
            Terminal::String => "string",
            Terminal::Bytes => "bytes",
            Terminal::Fixed => "fixed",
            Terminal::Enum => "enum",
            Terminal::Union => "union",
            Terminal::ArrayStart => "array-start",
            Terminal::ArrayEnd => "array-end",
            Terminal::MapStart => "map-start",
            Terminal::MapEnd => "map-end",
            Terminal::ItemEnd => "item-end",
            Terminal::MapKeyMarker => "map-key-marker",
        }
    }
}

type SymId = usize;

enum Sym {
    Terminal(Terminal),
    /// `Root` and `Sequence`: the production, the last element processed first.
    Seq(Vec<SymId>),
    Root(Vec<SymId>),
    /// `Repeater`: pushes itself back under its body until its end symbol is asked for.
    Repeater { end: Terminal, body: Vec<SymId> },
    Alternative { symbols: Vec<SymId>, labels: Vec<String> },
    IntCheck(usize),
    EnumLabels(Vec<String>),
    FieldAdjust { name: Vec<u16>, aliases: Vec<Vec<u16>> },
    RecordStart,
    RecordEnd,
    UnionEnd,
    FieldEnd,
}

impl Sym {
    fn is_implicit(&self) -> bool {
        matches!(self, Sym::FieldAdjust { .. } | Sym::RecordStart | Sym::RecordEnd | Sym::UnionEnd | Sym::FieldEnd)
    }

    fn is_trailing(&self) -> bool {
        matches!(self, Sym::RecordEnd | Sym::UnionEnd | Sym::FieldEnd)
    }
}

struct Grammar {
    syms: Vec<Sym>,
    terminals: HashMap<Terminal, SymId>,
    record_start: SymId,
    record_end: SymId,
    union_end: SymId,
    field_end: SymId,
}

impl Grammar {
    fn add(&mut self, s: Sym) -> SymId {
        self.syms.push(s);
        self.syms.len() - 1
    }

    fn terminal(&mut self, t: Terminal) -> SymId {
        if let Some(id) = self.terminals.get(&t) {
            return *id;
        }
        let id = self.add(Sym::Terminal(t));
        self.terminals.insert(t, id);
        id
    }

    /// `JsonGrammarGenerator.generate(schema)`.
    fn new(schema: &Schema) -> (Grammar, SymId) {
        let mut g = Grammar { syms: Vec::new(), terminals: HashMap::new(), record_start: 0, record_end: 0, union_end: 0, field_end: 0 };
        g.record_start = g.add(Sym::RecordStart);
        g.record_end = g.add(Sym::RecordEnd);
        g.union_end = g.add(Sym::UnionEnd);
        g.field_end = g.add(Sym::FieldEnd);
        let mut seen = HashMap::new();
        let body = g.generate(schema, &schema.root, &mut seen);
        let root = g.add(Sym::Root(vec![body]));
        (g, root)
    }

    fn generate(&mut self, schema: &Schema, t: &Type, seen: &mut HashMap<usize, SymId>) -> SymId {
        match t {
            Type::Primitive(p, _) => self.terminal(match p {
                Primitive::Null => Terminal::Null,
                Primitive::Boolean => Terminal::Boolean,
                Primitive::Int => Terminal::Int,
                Primitive::Long => Terminal::Long,
                Primitive::Float => Terminal::Float,
                Primitive::Double => Terminal::Double,
                Primitive::Bytes => Terminal::Bytes,
                Primitive::String => Terminal::String,
            }),
            Type::Array(items, _) => self.repeated(schema, items, None, seen),
            Type::Map(values, _) => self.repeated(schema, values, Some(()), seen),
            Type::Union(types) => self.alternative(schema, types, seen),
            Type::Named(i) => self.named_type(schema, *i, seen),
        }
    }

    /// An array (`key` `None`) or a map.
    #[inline(never)]
    fn repeated(&mut self, schema: &Schema, element: &Type, key: Option<()>, seen: &mut HashMap<usize, SymId>) -> SymId {
        let end = self.terminal(Terminal::ItemEnd);
        let value = self.generate(schema, element, seen);
        let (body, end_symbol, start) = match key {
            None => (vec![end, value], Terminal::ArrayEnd, Terminal::ArrayStart),
            Some(()) => {
                let marker = self.terminal(Terminal::MapKeyMarker);
                let key = self.terminal(Terminal::String);
                (vec![end, value, marker, key], Terminal::MapEnd, Terminal::MapStart)
            }
        };
        let rep = self.add(Sym::Repeater { end: end_symbol, body });
        let start = self.terminal(start);
        self.add(Sym::Seq(vec![rep, start]))
    }

    #[inline(never)]
    fn alternative(&mut self, schema: &Schema, types: &[Type], seen: &mut HashMap<usize, SymId>) -> SymId {
        let symbols = types.iter().map(|b| self.generate(schema, b, seen)).collect();
        let labels = types.iter().map(|b| schema::full_name(&schema.named, b)).collect();
        let alt = self.add(Sym::Alternative { symbols, labels });
        let u = self.terminal(Terminal::Union);
        self.add(Sym::Seq(vec![alt, u]))
    }

    #[inline(never)]
    fn named_type(&mut self, schema: &Schema, i: usize, seen: &mut HashMap<usize, SymId>) -> SymId {
        match &schema.named[i] {
            Named::Fixed(f) => {
                let check = self.add(Sym::IntCheck(f.size));
                let fixed = self.terminal(Terminal::Fixed);
                self.add(Sym::Seq(vec![check, fixed]))
            }
            Named::Enum(e) => {
                let labels = self.add(Sym::EnumLabels(e.symbols.clone()));
                let en = self.terminal(Terminal::Enum);
                self.add(Sym::Seq(vec![labels, en]))
            }
            Named::Record(r) => {
                if let Some(id) = seen.get(&i) {
                    return *id;
                }
                let id = self.add(Sym::Seq(Vec::new()));
                seen.insert(i, id);
                // `production[--i]` from the end: RECORD_START, then per field its adjust action, value and FIELD_END;
                // RECORD_END first.
                let mut per_field = Vec::with_capacity(r.fields.len());
                for f in &r.fields {
                    let adjust = self.add(Sym::FieldAdjust { name: f.name.encode_utf16().collect(), aliases: f.aliases.iter().map(|a| a.encode_utf16().collect()).collect() });
                    let value = self.generate(schema, &f.schema, seen);
                    per_field.push((adjust, value));
                }
                let mut production = vec![self.record_end];
                for (adjust, value) in per_field.into_iter().rev() {
                    production.push(self.field_end);
                    production.push(value);
                    production.push(adjust);
                }
                production.push(self.record_start);
                self.syms[id] = Sym::Seq(production);
                id
            }
            Named::Unresolved { .. } => unreachable!("a parsed schema has no placeholders"),
        }
    }
}

// ---- the decoder ----

struct ReorderBuffer {
    saved: JavaHashMap<Vec<u16>, Vec<Tok>>,
    orig: Option<Box<dyn Source>>,
}

pub(crate) struct JsonDecoder {
    g: Grammar,
    stack: Vec<SymId>,
    input: Box<dyn Source>,
    reorder_buffers: Vec<Option<ReorderBuffer>>,
    current_buffer: Option<ReorderBuffer>,
}

/// A string as read: UTF-16 units (JSON) or UTF-8 bytes (binary).
pub(crate) enum Text {
    Units(Vec<u16>),
    Bytes(Vec<u8>),
}

fn units_display(u: &[u16]) -> String {
    String::from_utf16_lossy(u)
}

impl JsonDecoder {
    /// `DecoderFactory.jsonDecoder(schema, json)`: the parser is moved to the first token.
    pub(crate) fn new(schema: &Schema, json: &str) -> Result<JsonDecoder, Error> {
        let (g, root) = Grammar::new(schema);
        let mut input: Box<dyn Source> = Box::new(Scanner::new(json));
        input.next_token()?;
        Ok(JsonDecoder { g, stack: vec![root], input, reorder_buffers: Vec::new(), current_buffer: None })
    }

    fn error(&self, what: &str) -> Error {
        Error::avro_type(format!("Expected {what}. Got {}", self.input.kind().map_or("null", Kind::name)))
    }

    fn push_production(&mut self, id: SymId) {
        match &self.g.syms[id] {
            Sym::Seq(p) => self.stack.extend_from_slice(p),
            Sym::Root(p) => {
                self.stack.push(id);
                self.stack.extend_from_slice(p);
            }
            Sym::Repeater { body, .. } => {
                self.stack.push(id);
                self.stack.extend_from_slice(body);
            }
            _ => {}
        }
    }

    /// `Parser.advance(input)`.
    fn parser_advance(&mut self, input: Terminal) -> Result<(), Error> {
        loop {
            let Some(top) = self.stack.pop() else { return Err(Error::runtime("empty parser stack")) };
            match &self.g.syms[top] {
                Sym::Terminal(t) if *t == input => return Ok(()),
                Sym::Terminal(t) => return Err(Error::avro_type(format!("Attempt to process a {} when a {} was expected.", input.name(), t.name()))),
                s if s.is_implicit() => self.do_action(top)?,
                Sym::Repeater { end, .. } if *end == input => return Ok(()),
                _ => self.push_production(top),
            }
        }
    }

    fn process_trailing_implicit_actions(&mut self) -> Result<(), Error> {
        while let Some(&top) = self.stack.last() {
            if !self.g.syms[top].is_trailing() {
                break;
            }
            self.stack.pop();
            self.do_action(top)?;
        }
        Ok(())
    }

    /// `JsonDecoder.advance(symbol)`.
    fn advance(&mut self, input: Terminal) -> Result<(), Error> {
        self.process_trailing_implicit_actions()?;
        if self.input.current().is_none() && self.stack.len() == 1 {
            return Err(Error::eof());
        }
        self.parser_advance(input)
    }

    fn do_action(&mut self, top: SymId) -> Result<(), Error> {
        match &self.g.syms[top] {
            Sym::FieldAdjust { name, aliases } => {
                let (name, aliases) = (name.clone(), aliases.clone());
                if let Some(buf) = &mut self.current_buffer {
                    if let Some(tokens) = buf.saved.remove(java::string_hash(&name), &name) {
                        let replay: Box<dyn Source> = Box::new(Replay { tokens: tokens.into_iter(), current: None });
                        buf.orig = Some(std::mem::replace(&mut self.input, replay));
                        self.input.next_token()?;
                        return Ok(());
                    }
                }
                if self.input.kind() == Some(Kind::FieldName) {
                    loop {
                        let fname = self.input.text()?;
                        self.input.next_token()?;
                        if fname == name || aliases.contains(&fname) {
                            return Ok(());
                        }
                        let tokens = self.input.copy_current_structure()?;
                        let buf = self.current_buffer.get_or_insert_with(|| ReorderBuffer { saved: JavaHashMap::new(), orig: None });
                        buf.saved.put(java::string_hash(&fname), fname, tokens);
                        self.input.next_token()?;
                        if self.input.kind() != Some(Kind::FieldName) {
                            break;
                        }
                    }
                    return Err(Error::avro_type(format!("Expected field name not found: {}", units_display(&name))));
                }
                Ok(())
            }
            Sym::FieldEnd => {
                if let Some(buf) = &mut self.current_buffer {
                    if let Some(orig) = buf.orig.take() {
                        self.input = orig;
                    }
                }
                Ok(())
            }
            Sym::RecordStart => {
                if self.input.kind() != Some(Kind::StartObject) {
                    return Err(self.error("record-start"));
                }
                self.input.next_token()?;
                self.reorder_buffers.push(self.current_buffer.take());
                Ok(())
            }
            Sym::RecordEnd | Sym::UnionEnd => {
                let record = matches!(self.g.syms[top], Sym::RecordEnd);
                // AVRO-2034: on to the end of this object (the next END_OBJECT, whatever it closes).
                while self.input.kind() != Some(Kind::EndObject) {
                    if self.input.current().is_none() {
                        // Upstream loops here for ever.
                        return Err(Error::eof());
                    }
                    self.input.next_token()?;
                }
                if record {
                    if let Some(buf) = &self.current_buffer {
                        if !buf.saved.is_empty() {
                            let names: Vec<String> = buf.saved.iter().map(|(k, _)| units_display(k)).collect();
                            return Err(self.error(&format!("Unknown fields: [{}]", names.join(", "))));
                        }
                    }
                    self.current_buffer = self.reorder_buffers.pop().flatten();
                }
                self.input.next_token()
            }
            _ => Err(Error::avro_type("Unknown action symbol")),
        }
    }

    fn coercion(&self, kind: &str) -> Error {
        let t = self.input.current().expect("a number");
        let range = if kind == "int" { "(-2147483648 - 2147483647)" } else { "(-9223372036854775808 - 9223372036854775807)" };
        let at = match t.loc {
            Some((line, column)) => format!("[Source: REDACTED (`StreamReadFeature.INCLUDE_SOURCE_IN_LOCATION` disabled); line: {line}, column: {column}]"),
            None => "[Source: UNKNOWN; byte offset: #UNKNOWN]".into(),
        };
        Error::new("com.fasterxml.jackson.core.exc.InputCoercionException", format!("Numeric value ({}) out of range of {kind} {range}\n at {at}", t.number_text()))
    }

    fn number(&self) -> String {
        self.input.current().map(Tok::number_text).unwrap_or_default()
    }

    pub(crate) fn read_null(&mut self) -> Result<(), Error> {
        self.advance(Terminal::Null)?;
        if self.input.kind() != Some(Kind::Null) {
            return Err(self.error("null"));
        }
        self.input.next_token()
    }

    pub(crate) fn read_boolean(&mut self) -> Result<bool, Error> {
        self.advance(Terminal::Boolean)?;
        let value = match self.input.kind() {
            Some(Kind::True) => true,
            Some(Kind::False) => false,
            _ => return Err(self.error("boolean")),
        };
        self.input.next_token()?;
        Ok(value)
    }

    pub(crate) fn read_int(&mut self) -> Result<i32, Error> {
        self.advance(Terminal::Int)?;
        match self.input.kind() {
            Some(Kind::Int) => {
                let n = self.number().parse::<i32>().map_err(|_| self.coercion("int"))?;
                self.input.next_token()?;
                Ok(n)
            }
            Some(Kind::Float) => {
                let value = self.number().parse::<f32>().unwrap_or(f32::NAN);
                let rounded = java::round_f32(value);
                if (value - rounded as f32).abs() <= f32::from_bits(1) {
                    self.input.next_token()?;
                    return Ok(rounded);
                }
                Err(self.error("int"))
            }
            _ => Err(self.error("int")),
        }
    }

    pub(crate) fn read_long(&mut self) -> Result<i64, Error> {
        self.advance(Terminal::Long)?;
        match self.input.kind() {
            Some(Kind::Int) => {
                let n = self.number().parse::<i64>().map_err(|_| self.coercion("long"))?;
                self.input.next_token()?;
                Ok(n)
            }
            Some(Kind::Float) => {
                let value = self.number().parse::<f64>().unwrap_or(f64::NAN);
                let rounded = java::round_f64(value);
                if (value - rounded as f64).abs() <= f64::from_bits(1) {
                    self.input.next_token()?;
                    return Ok(rounded);
                }
                Err(self.error("long"))
            }
            _ => Err(self.error("long")),
        }
    }

    /// `NaN`, `Infinity`/`INF` and `-Infinity`/`-INF` as strings.
    fn special(&mut self, what: &str) -> Result<f64, Error> {
        let s = units_display(&self.input.text()?);
        self.input.next_token()?;
        match s.as_str() {
            "NaN" => Ok(f64::NAN),
            "-Infinity" | "-INF" => Ok(f64::NEG_INFINITY),
            "Infinity" | "INF" => Ok(f64::INFINITY),
            _ => Err(self.error(what)),
        }
    }

    pub(crate) fn read_float(&mut self) -> Result<f32, Error> {
        self.advance(Terminal::Float)?;
        match self.input.kind() {
            Some(Kind::Int | Kind::Float) => {
                let f = self.number().parse::<f32>().unwrap_or(0.0);
                self.input.next_token()?;
                Ok(f)
            }
            Some(Kind::Str) => self.special("float").map(|d| d as f32),
            _ => Err(self.error("float")),
        }
    }

    pub(crate) fn read_double(&mut self) -> Result<f64, Error> {
        self.advance(Terminal::Double)?;
        match self.input.kind() {
            Some(Kind::Int | Kind::Float) => {
                let d = self.number().parse::<f64>().unwrap_or(0.0);
                self.input.next_token()?;
                Ok(d)
            }
            Some(Kind::Str) => self.special("double"),
            _ => Err(self.error("double")),
        }
    }

    pub(crate) fn read_string(&mut self) -> Result<Text, Error> {
        self.advance(Terminal::String)?;
        let map_key = self.stack.last().is_some_and(|&t| matches!(self.g.syms[t], Sym::Terminal(Terminal::MapKeyMarker)));
        if map_key {
            self.parser_advance(Terminal::MapKeyMarker)?;
            if self.input.kind() != Some(Kind::FieldName) {
                return Err(self.error("map-key"));
            }
        } else if self.input.kind() != Some(Kind::Str) {
            return Err(self.error("string"));
        }
        let text = self.input.text()?;
        self.input.next_token()?;
        Ok(Text::Units(text))
    }

    pub(crate) fn read_bytes(&mut self) -> Result<Vec<u8>, Error> {
        self.advance(Terminal::Bytes)?;
        if self.input.kind() != Some(Kind::Str) {
            return Err(self.error("bytes"));
        }
        let bytes = java::latin1_bytes(&self.input.text()?);
        self.input.next_token()?;
        Ok(bytes)
    }

    fn pop_symbol(&mut self) -> SymId {
        self.stack.pop().expect("an action symbol")
    }

    pub(crate) fn read_fixed(&mut self, len: usize) -> Result<Vec<u8>, Error> {
        self.advance(Terminal::Fixed)?;
        let check = self.pop_symbol();
        if let Sym::IntCheck(size) = self.g.syms[check] {
            if size != len {
                return Err(Error::avro_type(format!("Incorrect length for fixed binary: expected {size} but received {len} bytes.")));
            }
        }
        if self.input.kind() != Some(Kind::Str) {
            return Err(self.error("fixed"));
        }
        let bytes = java::latin1_bytes(&self.input.text()?);
        self.input.next_token()?;
        if bytes.len() != len {
            return Err(Error::avro_type(format!("Expected fixed length {len}, but got{}", bytes.len())));
        }
        Ok(bytes)
    }

    pub(crate) fn read_enum(&mut self) -> Result<i32, Error> {
        self.advance(Terminal::Enum)?;
        let labels = self.pop_symbol();
        if self.input.kind() != Some(Kind::Str) {
            return Err(self.error("fixed"));
        }
        let text = units_display(&self.input.text()?);
        let Sym::EnumLabels(symbols) = &self.g.syms[labels] else { return Err(self.error("enum")) };
        match symbols.iter().position(|s| *s == text) {
            Some(n) => {
                self.input.next_token()?;
                Ok(n as i32)
            }
            None => Err(Error::avro_type(format!("Unknown symbol in enum {text}"))),
        }
    }

    pub(crate) fn read_array_start(&mut self) -> Result<i64, Error> {
        self.advance(Terminal::ArrayStart)?;
        if self.input.kind() != Some(Kind::StartArray) {
            return Err(self.error("array-start"));
        }
        self.input.next_token()?;
        self.array_next_item()
    }

    pub(crate) fn array_next(&mut self) -> Result<i64, Error> {
        self.advance(Terminal::ItemEnd)?;
        self.array_next_item()
    }

    fn array_next_item(&mut self) -> Result<i64, Error> {
        if self.input.kind() == Some(Kind::EndArray) {
            self.parser_advance(Terminal::ArrayEnd)?;
            self.input.next_token()?;
            return Ok(0);
        }
        Ok(1)
    }

    pub(crate) fn read_map_start(&mut self) -> Result<i64, Error> {
        self.advance(Terminal::MapStart)?;
        if self.input.kind() != Some(Kind::StartObject) {
            return Err(self.error("map-start"));
        }
        self.input.next_token()?;
        self.map_next_item()
    }

    pub(crate) fn map_next(&mut self) -> Result<i64, Error> {
        self.advance(Terminal::ItemEnd)?;
        self.map_next_item()
    }

    fn map_next_item(&mut self) -> Result<i64, Error> {
        if self.input.kind() == Some(Kind::EndObject) {
            self.input.next_token()?;
            self.advance(Terminal::MapEnd)?;
            return Ok(0);
        }
        Ok(1)
    }

    pub(crate) fn read_index(&mut self) -> Result<i32, Error> {
        self.advance(Terminal::Union)?;
        let alt = self.pop_symbol();
        let label: Vec<u16> = match self.input.kind() {
            Some(Kind::Null) => "null".encode_utf16().collect(),
            Some(Kind::StartObject) => {
                self.input.next_token()?;
                if self.input.kind() != Some(Kind::FieldName) {
                    return Err(self.error("start-union"));
                }
                let label = self.input.text()?;
                self.input.next_token()?;
                self.stack.push(self.g.union_end);
                label
            }
            _ => return Err(self.error("start-union")),
        };
        let Sym::Alternative { symbols, labels } = &self.g.syms[alt] else { return Err(self.error("start-union")) };
        let label = units_display(&label);
        let Some(n) = labels.iter().position(|l| *l == label) else { return Err(Error::avro_type(format!("Unknown union branch {label}"))) };
        let branch = symbols[n];
        self.stack.push(branch);
        Ok(n as i32)
    }
}
