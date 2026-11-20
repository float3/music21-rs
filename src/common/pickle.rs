//! Python's pickle format, read into a tree of values and written back,
//! as far as music21's cache files use it.
//!
//! music21 keeps its corpus metadata as a pickled `MetadataBundle`. Reading
//! one runs pickle's stack machine over its opcodes; what comes out is a
//! [`Pickle`]: every value the stream built, in one arena, with objects of
//! Python classes kept as the class they name, the arguments they were made
//! with and the state they were given. Nothing is imported or run. Writing
//! is the reverse: a [`Pickle`] built with [`Pickle::push`] is written as
//! protocol 5, which Python's own `pickle.loads` reads.
//!
//! ```
//! use music21::common::pickle::{Pickle, PickleValue};
//!
//! let mut built = Pickle::default();
//! let key = built.push(PickleValue::Text("title".into()));
//! let value = built.push(PickleValue::Int(7));
//! let root = built.push(PickleValue::Dict(vec![(key, value)]));
//! built.set_root(root);
//!
//! let read = Pickle::read(&built.write()).unwrap();
//! let found = read.dict_get(read.root(), "title").unwrap();
//! assert_eq!(read.get(found), &PickleValue::Int(7));
//! ```

use std::collections::HashMap;

use crate::error::{Error, Result};

/// Where a value sits in a [`Pickle`]'s arena.
pub type ValueId = usize;

/// The protocol [`Pickle::write`] writes, as music21's `pickle.dumps(...,
/// protocol=5)` does.
pub const PROTOCOL: u8 = 5;

// The opcodes read and written, as Python's `pickletools` names them.
const MARK: u8 = b'(';
const STOP: u8 = b'.';
const POP: u8 = b'0';
const POP_MARK: u8 = b'1';
const DUP: u8 = b'2';
const BININT: u8 = b'J';
const BININT1: u8 = b'K';
const BININT2: u8 = b'M';
const NONE: u8 = b'N';
const BINUNICODE: u8 = b'X';
const BINBYTES: u8 = b'B';
const SHORT_BINBYTES: u8 = b'C';
const APPEND: u8 = b'a';
const BUILD: u8 = b'b';
const GLOBAL: u8 = b'c';
const DICT: u8 = b'd';
const EMPTY_DICT: u8 = b'}';
const APPENDS: u8 = b'e';
const BINGET: u8 = b'h';
const LONG_BINGET: u8 = b'j';
const LIST: u8 = b'l';
const EMPTY_LIST: u8 = b']';
const BINPUT: u8 = b'q';
const LONG_BINPUT: u8 = b'r';
const SETITEM: u8 = b's';
const TUPLE: u8 = b't';
const EMPTY_TUPLE: u8 = b')';
const SETITEMS: u8 = b'u';
const BINFLOAT: u8 = b'G';
const PROTO: u8 = 0x80;
const NEWOBJ: u8 = 0x81;
const TUPLE1: u8 = 0x85;
const TUPLE2: u8 = 0x86;
const TUPLE3: u8 = 0x87;
const NEWTRUE: u8 = 0x88;
const NEWFALSE: u8 = 0x89;
const LONG1: u8 = 0x8a;
const REDUCE: u8 = b'R';
const SHORT_BINUNICODE: u8 = 0x8c;
const BINUNICODE8: u8 = 0x8d;
const BINBYTES8: u8 = 0x8e;
const EMPTY_SET: u8 = 0x8f;
const ADDITEMS: u8 = 0x90;
const FROZENSET: u8 = 0x91;
const NEWOBJ_EX: u8 = 0x92;
const STACK_GLOBAL: u8 = 0x93;
const MEMOIZE: u8 = 0x94;
const FRAME: u8 = 0x95;

/// How an object of a Python class was made.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Construction {
    /// `cls.__new__(cls, *args)`: pickle's `NEWOBJ`, what a plain class's
    /// instance is written as.
    New,
    /// `callable(*args)`: pickle's `REDUCE`, what `OrderedDict()` is.
    Call,
}

/// One value a pickle builds.
#[derive(Clone, Debug, PartialEq)]
pub enum PickleValue {
    /// `None`.
    None,
    /// `True` or `False`.
    Bool(bool),
    /// An `int` that fits in 64 bits.
    Int(i64),
    /// A `float`.
    Float(f64),
    /// A `str`.
    Text(String),
    /// A `bytes`.
    Bytes(Vec<u8>),
    /// A `tuple`.
    Tuple(Vec<ValueId>),
    /// A `list`.
    List(Vec<ValueId>),
    /// A `dict`, its items in order.
    Dict(Vec<(ValueId, ValueId)>),
    /// A `set` or `frozenset`.
    Set(Vec<ValueId>),
    /// A class or function, by its module and name.
    Global {
        /// Its module's dotted name.
        module: String,
        /// Its name there.
        name: String,
    },
    /// An object made from a class or callable.
    Object {
        /// The [`PickleValue::Global`] it was made from.
        class: ValueId,
        /// How it was made.
        construction: Construction,
        /// The arguments it was made with.
        args: Vec<ValueId>,
        /// What `BUILD` gave it: its `__setstate__` argument or `__dict__`.
        state: Option<ValueId>,
        /// What was appended to it, as to a list.
        items: Vec<ValueId>,
        /// What was set in it, as in a dict.
        entries: Vec<(ValueId, ValueId)>,
    },
}

/// A pickle's values, as read or as built to be written.
#[derive(Clone, Debug, Default)]
pub struct Pickle {
    values: Vec<PickleValue>,
    root: ValueId,
}

impl Pickle {
    /// Runs a pickle's opcodes, building its values.
    pub fn read(bytes: &[u8]) -> Result<Self> {
        Reader::new(bytes).run()
    }

    /// The value the pickle stands for.
    pub fn root(&self) -> ValueId {
        self.root
    }

    /// Makes a value the one the pickle stands for.
    pub fn set_root(&mut self, root: ValueId) {
        self.root = root;
    }

    /// A value, by where it sits.
    pub fn get(&self, id: ValueId) -> &PickleValue {
        &self.values[id]
    }

    /// Adds a value, answering where it sits.
    pub fn push(&mut self, value: PickleValue) -> ValueId {
        self.values.push(value);
        self.values.len() - 1
    }

    /// Adds a `str`.
    pub fn text(&mut self, text: &str) -> ValueId {
        self.push(PickleValue::Text(text.to_string()))
    }

    /// Adds a class or function by name.
    pub fn global(&mut self, module: &str, name: &str) -> ValueId {
        self.push(PickleValue::Global {
            module: module.to_string(),
            name: name.to_string(),
        })
    }

    /// The text of a `str` value.
    pub fn as_text(&self, id: ValueId) -> Option<&str> {
        match self.get(id) {
            PickleValue::Text(text) => Some(text),
            _ => None,
        }
    }

    /// The items of a dict, or of an object's state where that is a dict,
    /// or what was set in an object.
    pub fn dict_items(&self, id: ValueId) -> &[(ValueId, ValueId)] {
        match self.get(id) {
            PickleValue::Dict(items) => items,
            PickleValue::Object {
                state: Some(state),
                entries,
                ..
            } if entries.is_empty() => self.dict_items(*state),
            PickleValue::Object { entries, .. } => entries,
            _ => &[],
        }
    }

    /// The value under a `str` key of a dict, or of an object's state.
    pub fn dict_get(&self, id: ValueId, key: &str) -> Option<ValueId> {
        self.dict_items(id)
            .iter()
            .find(|(found, _)| self.as_text(*found) == Some(key))
            .map(|(_, value)| *value)
    }

    /// The members of a list, tuple or set, or what was appended to an
    /// object.
    pub fn members(&self, id: ValueId) -> &[ValueId] {
        match self.get(id) {
            PickleValue::List(members)
            | PickleValue::Tuple(members)
            | PickleValue::Set(members) => members,
            PickleValue::Object { items, .. } => items,
            _ => &[],
        }
    }

    /// The module and name of the class an object was made from.
    pub fn class_of(&self, id: ValueId) -> Option<(&str, &str)> {
        let PickleValue::Object { class, .. } = self.get(id) else {
            return None;
        };
        match self.get(*class) {
            PickleValue::Global { module, name } => Some((module, name)),
            _ => None,
        }
    }

    /// The arguments an object was made with.
    pub fn args_of(&self, id: ValueId) -> &[ValueId] {
        match self.get(id) {
            PickleValue::Object { args, .. } => args,
            _ => &[],
        }
    }

    /// The pickle as protocol 5 bytes.
    pub fn write(&self) -> Vec<u8> {
        let mut out = vec![PROTO, PROTOCOL];
        self.write_value(self.root, &mut out);
        out.push(STOP);
        out
    }

    fn write_value(&self, id: ValueId, out: &mut Vec<u8>) {
        match self.get(id) {
            PickleValue::None => out.push(NONE),
            PickleValue::Bool(true) => out.push(NEWTRUE),
            PickleValue::Bool(false) => out.push(NEWFALSE),
            PickleValue::Int(value) => write_int(*value, out),
            PickleValue::Float(value) => {
                out.push(BINFLOAT);
                out.extend_from_slice(&value.to_be_bytes());
            }
            PickleValue::Text(text) => write_text(text, out),
            PickleValue::Bytes(bytes) => {
                out.push(BINBYTES);
                out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
                out.extend_from_slice(bytes);
            }
            PickleValue::Tuple(members) => {
                out.push(MARK);
                self.write_all(members, out);
                out.push(TUPLE);
            }
            PickleValue::List(members) => {
                out.push(EMPTY_LIST);
                self.write_appends(members, out);
            }
            PickleValue::Dict(items) => {
                out.push(EMPTY_DICT);
                self.write_setitems(items, out);
            }
            PickleValue::Set(members) => {
                out.push(EMPTY_SET);
                if !members.is_empty() {
                    out.push(MARK);
                    self.write_all(members, out);
                    out.push(ADDITEMS);
                }
            }
            PickleValue::Global { module, name } => {
                write_text(module, out);
                write_text(name, out);
                out.push(STACK_GLOBAL);
            }
            PickleValue::Object {
                class,
                construction,
                args,
                state,
                items,
                entries,
            } => {
                self.write_value(*class, out);
                out.push(MARK);
                self.write_all(args, out);
                out.push(TUPLE);
                out.push(match construction {
                    Construction::New => NEWOBJ,
                    Construction::Call => REDUCE,
                });
                self.write_appends(items, out);
                self.write_setitems(entries, out);
                if let Some(state) = state {
                    self.write_value(*state, out);
                    out.push(BUILD);
                }
            }
        }
    }

    fn write_all(&self, members: &[ValueId], out: &mut Vec<u8>) {
        for member in members {
            self.write_value(*member, out);
        }
    }

    fn write_appends(&self, members: &[ValueId], out: &mut Vec<u8>) {
        if members.is_empty() {
            return;
        }
        out.push(MARK);
        self.write_all(members, out);
        out.push(APPENDS);
    }

    fn write_setitems(&self, items: &[(ValueId, ValueId)], out: &mut Vec<u8>) {
        if items.is_empty() {
            return;
        }
        out.push(MARK);
        for (key, value) in items {
            self.write_value(*key, out);
            self.write_value(*value, out);
        }
        out.push(SETITEMS);
    }
}

/// An int in the shortest opcode that holds it.
fn write_int(value: i64, out: &mut Vec<u8>) {
    if (0..=0xff).contains(&value) {
        out.extend_from_slice(&[BININT1, value as u8]);
        return;
    }
    if (0..=0xffff).contains(&value) {
        out.push(BININT2);
        out.extend_from_slice(&(value as u16).to_le_bytes());
        return;
    }
    if let Ok(small) = i32::try_from(value) {
        out.push(BININT);
        out.extend_from_slice(&small.to_le_bytes());
        return;
    }

    // LONG1: little-endian two's complement, as short as keeps the sign.
    let mut bytes = value.to_le_bytes().to_vec();
    while bytes.len() > 1 {
        let last = bytes[bytes.len() - 1];
        let next = bytes[bytes.len() - 2];
        let redundant = (last == 0 && next & 0x80 == 0) || (last == 0xff && next & 0x80 != 0);
        if !redundant {
            break;
        }
        bytes.pop();
    }
    out.extend_from_slice(&[LONG1, bytes.len() as u8]);
    out.extend_from_slice(&bytes);
}

fn write_text(text: &str, out: &mut Vec<u8>) {
    let bytes = text.as_bytes();
    if bytes.len() < 256 {
        out.extend_from_slice(&[SHORT_BINUNICODE, bytes.len() as u8]);
    } else {
        out.push(BINUNICODE);
        out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    }
    out.extend_from_slice(bytes);
}

fn pickle_error(message: impl Into<String>) -> Error {
    Error::Value(format!("pickle: {}", message.into()))
}

/// What sits on the reading stack: a value, or a mark.
#[derive(Clone, Copy)]
enum Slot {
    Value(ValueId),
    Mark,
}

/// Pickle's stack machine.
struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
    stack: Vec<Slot>,
    memo: HashMap<u64, ValueId>,
    built: Pickle,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            at: 0,
            stack: Vec::new(),
            memo: HashMap::new(),
            built: Pickle::default(),
        }
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8]> {
        let end = self
            .at
            .checked_add(count)
            .filter(|end| *end <= self.bytes.len());
        let Some(end) = end else {
            return Err(pickle_error("data ends early"));
        };
        let taken = &self.bytes[self.at..end];
        self.at = end;
        Ok(taken)
    }

    fn byte(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }

    fn u32(&mut self) -> Result<u32> {
        let bytes = self.take(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn u64(&mut self) -> Result<u64> {
        let mut bytes = [0; 8];
        bytes.copy_from_slice(self.take(8)?);
        Ok(u64::from_le_bytes(bytes))
    }

    fn line(&mut self) -> Result<String> {
        let rest = &self.bytes[self.at..];
        let Some(end) = rest.iter().position(|byte| *byte == b'\n') else {
            return Err(pickle_error("a line without its end"));
        };
        self.at += end + 1;
        Ok(String::from_utf8_lossy(&rest[..end]).into_owned())
    }

    fn utf8(&mut self, count: usize) -> Result<String> {
        let bytes = self.take(count)?;
        String::from_utf8(bytes.to_vec()).map_err(|_| pickle_error("a string that is not UTF-8"))
    }

    fn push(&mut self, value: PickleValue) {
        let id = self.built.push(value);
        self.stack.push(Slot::Value(id));
    }

    fn pop(&mut self) -> Result<ValueId> {
        match self.stack.pop() {
            Some(Slot::Value(id)) => Ok(id),
            _ => Err(pickle_error("a value was wanted on the stack")),
        }
    }

    fn top(&self) -> Result<ValueId> {
        match self.stack.last() {
            Some(Slot::Value(id)) => Ok(*id),
            _ => Err(pickle_error("a value was wanted on the stack")),
        }
    }

    /// Everything above the last mark, the mark taken off.
    fn pop_mark(&mut self) -> Result<Vec<ValueId>> {
        let Some(mark) = self
            .stack
            .iter()
            .rposition(|slot| matches!(slot, Slot::Mark))
        else {
            return Err(pickle_error("no mark on the stack"));
        };
        let above = self.stack.split_off(mark + 1);
        self.stack.pop();
        Ok(above
            .into_iter()
            .filter_map(|slot| match slot {
                Slot::Value(id) => Some(id),
                Slot::Mark => None,
            })
            .collect())
    }

    fn memoize(&mut self, key: u64) -> Result<()> {
        let top = self.top()?;
        self.memo.insert(key, top);
        Ok(())
    }

    fn recall(&mut self, key: u64) -> Result<()> {
        let Some(id) = self.memo.get(&key) else {
            return Err(pickle_error(format!("nothing remembered under {key}")));
        };
        self.stack.push(Slot::Value(*id));
        Ok(())
    }

    fn append(&mut self, target: ValueId, members: Vec<ValueId>) -> Result<()> {
        match &mut self.built.values[target] {
            PickleValue::List(list) => list.extend(members),
            PickleValue::Set(set) => set.extend(members),
            PickleValue::Object { items, .. } => items.extend(members),
            _ => return Err(pickle_error("appending to what is not a list")),
        }
        Ok(())
    }

    fn set_items(&mut self, target: ValueId, flat: Vec<ValueId>) -> Result<()> {
        let pairs = flat.chunks_exact(2).map(|pair| (pair[0], pair[1]));
        match &mut self.built.values[target] {
            PickleValue::Dict(items) => items.extend(pairs),
            PickleValue::Object { entries, .. } => entries.extend(pairs),
            _ => return Err(pickle_error("setting items in what is not a dict")),
        }
        Ok(())
    }

    fn make(&mut self, construction: Construction) -> Result<()> {
        let args = self.pop()?;
        let class = self.pop()?;
        let args = self.built.members(args).to_vec();
        self.push(PickleValue::Object {
            class,
            construction,
            args,
            state: None,
            items: Vec::new(),
            entries: Vec::new(),
        });
        Ok(())
    }

    fn run(mut self) -> Result<Pickle> {
        loop {
            let opcode = self.byte()?;
            if opcode == STOP {
                break;
            }
            self.step(opcode)?;
        }
        let root = self.pop()?;
        self.built.root = root;
        Ok(self.built)
    }

    fn step(&mut self, opcode: u8) -> Result<()> {
        match opcode {
            PROTO => {
                self.byte()?;
            }
            FRAME => {
                self.u64()?;
            }
            MARK => self.stack.push(Slot::Mark),
            POP => {
                self.stack.pop();
            }
            POP_MARK => {
                self.pop_mark()?;
            }
            DUP => {
                let top = self.top()?;
                self.stack.push(Slot::Value(top));
            }
            NONE => self.push(PickleValue::None),
            NEWTRUE => self.push(PickleValue::Bool(true)),
            NEWFALSE => self.push(PickleValue::Bool(false)),
            BININT1 => {
                let value = self.byte()?;
                self.push(PickleValue::Int(i64::from(value)));
            }
            BININT2 => {
                let bytes = self.take(2)?;
                let value = u16::from_le_bytes([bytes[0], bytes[1]]);
                self.push(PickleValue::Int(i64::from(value)));
            }
            BININT => {
                let value = self.u32()? as i32;
                self.push(PickleValue::Int(i64::from(value)));
            }
            LONG1 => {
                let count = usize::from(self.byte()?);
                let value = long_value(self.take(count)?)?;
                self.push(PickleValue::Int(value));
            }
            BINFLOAT => {
                let mut bytes = [0; 8];
                bytes.copy_from_slice(self.take(8)?);
                self.push(PickleValue::Float(f64::from_be_bytes(bytes)));
            }
            SHORT_BINUNICODE => {
                let count = usize::from(self.byte()?);
                let text = self.utf8(count)?;
                self.push(PickleValue::Text(text));
            }
            BINUNICODE => {
                let count = self.u32()? as usize;
                let text = self.utf8(count)?;
                self.push(PickleValue::Text(text));
            }
            BINUNICODE8 => {
                let count = self.u64()? as usize;
                let text = self.utf8(count)?;
                self.push(PickleValue::Text(text));
            }
            SHORT_BINBYTES => {
                let count = usize::from(self.byte()?);
                let bytes = self.take(count)?.to_vec();
                self.push(PickleValue::Bytes(bytes));
            }
            BINBYTES => {
                let count = self.u32()? as usize;
                let bytes = self.take(count)?.to_vec();
                self.push(PickleValue::Bytes(bytes));
            }
            BINBYTES8 => {
                let count = self.u64()? as usize;
                let bytes = self.take(count)?.to_vec();
                self.push(PickleValue::Bytes(bytes));
            }
            EMPTY_TUPLE => self.push(PickleValue::Tuple(Vec::new())),
            TUPLE1 | TUPLE2 | TUPLE3 => {
                let count = usize::from(opcode - TUPLE1 + 1);
                let mut members = Vec::with_capacity(count);
                for _ in 0..count {
                    members.push(self.pop()?);
                }
                members.reverse();
                self.push(PickleValue::Tuple(members));
            }
            TUPLE => {
                let members = self.pop_mark()?;
                self.push(PickleValue::Tuple(members));
            }
            EMPTY_LIST => self.push(PickleValue::List(Vec::new())),
            LIST => {
                let members = self.pop_mark()?;
                self.push(PickleValue::List(members));
            }
            EMPTY_DICT => self.push(PickleValue::Dict(Vec::new())),
            DICT => {
                let flat = self.pop_mark()?;
                let items = flat
                    .chunks_exact(2)
                    .map(|pair| (pair[0], pair[1]))
                    .collect();
                self.push(PickleValue::Dict(items));
            }
            EMPTY_SET => self.push(PickleValue::Set(Vec::new())),
            FROZENSET => {
                let members = self.pop_mark()?;
                self.push(PickleValue::Set(members));
            }
            APPEND => {
                let member = self.pop()?;
                let target = self.top()?;
                self.append(target, vec![member])?;
            }
            APPENDS | ADDITEMS => {
                let members = self.pop_mark()?;
                let target = self.top()?;
                self.append(target, members)?;
            }
            SETITEM => {
                let value = self.pop()?;
                let key = self.pop()?;
                let target = self.top()?;
                self.set_items(target, vec![key, value])?;
            }
            SETITEMS => {
                let flat = self.pop_mark()?;
                let target = self.top()?;
                self.set_items(target, flat)?;
            }
            GLOBAL => {
                let module = self.line()?;
                let name = self.line()?;
                self.push(PickleValue::Global { module, name });
            }
            STACK_GLOBAL => {
                let name = self.pop()?;
                let module = self.pop()?;
                let (Some(module), Some(name)) =
                    (self.built.as_text(module), self.built.as_text(name))
                else {
                    return Err(pickle_error("STACK_GLOBAL wants two strings"));
                };
                let global = PickleValue::Global {
                    module: module.to_string(),
                    name: name.to_string(),
                };
                self.push(global);
            }
            NEWOBJ => self.make(Construction::New)?,
            NEWOBJ_EX => {
                // The keywords are dropped: music21's classes take none.
                self.pop()?;
                self.make(Construction::New)?;
            }
            REDUCE => self.make(Construction::Call)?,
            BUILD => {
                let state = self.pop()?;
                let target = self.top()?;
                let PickleValue::Object { state: slot, .. } = &mut self.built.values[target] else {
                    return Err(pickle_error("BUILD on what is not an object"));
                };
                *slot = Some(state);
            }
            MEMOIZE => {
                let key = self.memo.len() as u64;
                self.memoize(key)?;
            }
            BINPUT => {
                let key = u64::from(self.byte()?);
                self.memoize(key)?;
            }
            LONG_BINPUT => {
                let key = u64::from(self.u32()?);
                self.memoize(key)?;
            }
            BINGET => {
                let key = u64::from(self.byte()?);
                self.recall(key)?;
            }
            LONG_BINGET => {
                let key = u64::from(self.u32()?);
                self.recall(key)?;
            }
            other => return Err(pickle_error(format!("opcode {other:#04x} is not read"))),
        }
        Ok(())
    }
}

/// A `LONG1` value: little-endian two's complement.
fn long_value(bytes: &[u8]) -> Result<i64> {
    if bytes.is_empty() {
        return Ok(0);
    }
    if bytes.len() > 8 {
        return Err(pickle_error("an int wider than 64 bits"));
    }
    let fill = if bytes[bytes.len() - 1] & 0x80 != 0 {
        0xff
    } else {
        0
    };
    let mut full = [fill; 8];
    full[..bytes.len()].copy_from_slice(bytes);
    Ok(i64::from_le_bytes(full))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `pickle.dumps(OrderedDict(a=[1, -2, 70000, 2**40]), protocol=5)` as
    /// Python 3.13 writes it.
    const ORDERED: &[u8] = b"\x80\x05\x95?\x00\x00\x00\x00\x00\x00\x00\x8c\x0bcollections\
\x94\x8c\x0bOrderedDict\x94\x93\x94)R\x94\x8c\x01a\x94]\x94(K\x01J\xfe\xff\xff\xffJp\x11\
\x01\x00\x8a\x06\x00\x00\x00\x00\x00\x01es.";

    #[test]
    fn reads_what_python_writes() {
        let read = Pickle::read(ORDERED).unwrap();
        let root = read.root();
        assert_eq!(read.class_of(root), Some(("collections", "OrderedDict")));
        let list = read.dict_get(root, "a").unwrap();
        let values: Vec<&PickleValue> = read.members(list).iter().map(|id| read.get(*id)).collect();
        assert_eq!(
            values,
            [
                &PickleValue::Int(1),
                &PickleValue::Int(-2),
                &PickleValue::Int(70000),
                &PickleValue::Int(1 << 40)
            ]
        );
    }

    #[test]
    fn writes_what_it_reads_back() {
        let mut built = Pickle::default();
        let class = built.global("music21.metadata.bundles", "MetadataEntry");
        let path = built.text("bach/bwv66.6.mxl");
        let number = built.push(PickleValue::Int(-5_000_000_000));
        let half = built.push(PickleValue::Float(0.5));
        let state = built.push(PickleValue::Dict(vec![(path, half)]));
        let object = built.push(PickleValue::Object {
            class,
            construction: Construction::New,
            args: vec![path, number],
            state: Some(state),
            items: Vec::new(),
            entries: Vec::new(),
        });
        built.set_root(object);

        let read = Pickle::read(&built.write()).unwrap();
        let root = read.root();
        assert_eq!(
            read.class_of(root),
            Some(("music21.metadata.bundles", "MetadataEntry"))
        );
        let args = read.args_of(root);
        assert_eq!(read.as_text(args[0]), Some("bach/bwv66.6.mxl"));
        assert_eq!(read.get(args[1]), &PickleValue::Int(-5_000_000_000));
        assert_eq!(
            read.get(read.dict_get(root, "bach/bwv66.6.mxl").unwrap()),
            &PickleValue::Float(0.5)
        );
    }

    #[test]
    fn a_cut_off_pickle_is_an_error() {
        assert!(Pickle::read(&ORDERED[..20]).is_err());
    }
}
