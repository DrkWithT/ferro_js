use std::fmt::Display;

use crate::runtime::values::JSValue;
use crate::runtime::objects::{DUD_POOL_ID, ItemPool, JS_OBJECT_COST, JS_STRING_COST, JSObjPtr, JSStrPtr};

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Opcode {
    PushUndef,
    PushNull,
    PushBool,
    PushNaN,
    PushInf,
    PushNegInf,
    PushThisRef,
    PushStr,
    PushConst,
    Dup1,
    Dup2,
    Swap,
    PopN,
    Discard,
    GetLocal,
    SetLocal,
    InitVar,
    GetVar,
    SetVar,
    MakeObj,
    GetProp,
    SetProp,
    DelProp,
    GetProto,
    SetProto,
    IncLocal,
    DecLocal,
    IncProp,
    DecProp,
    MakeClosure,
    ForceBool,
    ForceNum,
    NegBool,
    NegNum,
    Mod,
    Mul,
    Div,
    Add,
    Sub,
    BtFlip,
    BtLs,
    BtRs,
    BtAnd,
    BtOr,
    BtXor,
    StrictEq,
    StrictNe,
    LooseEq,
    LooseNe,
    Lt,
    Lte,
    Gt,
    Gte,
    JumpIf,
    JumpElse,
    Jump,
    Call,
    CallCtor,
    NativeCall,
    Ret,
    // RET_CLOSURE,
}

pub const OPCODE_NAMES: &[&str] = &[
    "PushUndef",
    "PushNull",
    "PushBool",
    "PushNaN",
    "PushInf",
    "PushNegInf",
    "PushThisRef",  // ? Pushes the reference of `this`, possibly the global object or a constructor's environment object.
    "PushStr",
    "PushConst",
    "Dup1",
    "Dup2",
    "Swap",
    "PopN",
    "Discard",  // ? Pops an expression's result and puts `undefined`
    "GetLocal", // ? Uses constant offset via immediate arg
    "SetLocal", // ? Uses constant offset via immediate arg
    "InitVar",  // ? Creates a var binding in the current environment object, but it doesn't leave any temporary.
    "GetVar",
    "SetVar",   // ? Like InitVar, but leaves a temporary. Handles `=`.
    "MakeObj",
    "GetProp",
    "SetProp",
    "DelProp",
    "GetProto",
    "SetProto", // ? Uses a "builtin" flag: If 1, an intrinsic prototype via ID is put. Otherwise, the stack's top-most JSValue is used.
    "IncLocal",
    "DecLocal",
    "IncProp",
    "DecProp",
    "MakeClosure",
    "ForceBool",
    "ForceNum",
    "NegBool",
    "NegNum",
    "Mod",
    "Mul",
    "Div",
    "Add",
    "Sub",
    "BtFlip",
    "BtLs",
    "BtRs",
    "BtAnd",
    "BtOr",
    "BtXor",
    "StrictEq",
    "StrictNe",
    "LooseEq",
    "LooseNe",
    "Lt",
    "Lte",
    "Gt",
    "Gte",
    "JumpIf",
    "JumpElse",
    "Jump",
    "Call",
    "CallCtor",
    "NativeCall",
    "Ret",
];

#[derive(Debug, Clone, Copy)]
pub struct Instruction {
    pub arg: i32,
    pub flags: u16,
    pub op: Opcode,
}

impl Display for Instruction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {arg, flags, op} = *self;

        write!(f, "{}  flags({}), {}", OPCODE_NAMES[op as usize], flags, arg)
    }
}

#[repr(u8)]
#[derive(Debug, Default, Clone, Copy)]
pub enum ICState {
    #[default]
    Unset,
    Mono,
    Poly,
    Dead,
}

#[derive(Debug, Clone, Copy)]
pub struct ICEntry {
    /// Name of property that's interned in a string pool & used via ID.
    pub key_id: usize,
    /// Shape ID.
    pub shape: i32,
    /// Cached index into Object's property buffer, resolved via any Shape.
    pub val_pos: usize,
}

impl Default for ICEntry {
    fn default() -> Self {
        Self {
            key_id: 0,
            shape: DUD_POOL_ID,
            val_pos: 0
        }
    }
}

impl ICEntry {
    pub fn is_set(&self) -> bool {
        self.shape > DUD_POOL_ID
    }
}

pub const IC_MISSES_TO_POLY: u32 = 1024;

pub const IC_MISSES_TO_DEAD: u32 = 16384;

#[derive(Debug, Default, Clone, Copy)]
pub struct InlineCache {
    pub entries: [ICEntry; 2],
    pub misses: u32,
    pub state: ICState,
}

impl InlineCache {
    pub fn dead() -> Self {
        Self {
            entries: [ ICEntry::default(), ICEntry::default() ],
            misses: 0,
            state: ICState::Dead
        }
    }

    fn transition(misses: u32) -> ICState {
        if misses < IC_MISSES_TO_POLY {
            ICState::Mono
        } else if misses < IC_MISSES_TO_DEAD {
            ICState::Poly
        } else {
            ICState::Dead
        }
    }

    pub fn update(&mut self, shape_id: i32, key_id: usize, val_pos: usize) {
        match self.state {
            ICState::Mono => {
                // println!("IC [mono] += [shape => {shape_id}, key_id => {key_id}, prop_pos => {val_pos}]"); // debug
                self.entries[0] = ICEntry { key_id, shape: shape_id, val_pos };
            },
            ICState::Poly => {
                if !self.entries[0].is_set() {
                    // println!("IC [poly] += [shape => {shape_id}, key_id => {key_id}, prop_pos => {val_pos}]"); // debug
                    self.entries[0] = ICEntry { key_id, shape: shape_id, val_pos };
                } else if !self.entries[1].is_set() {
                    // println!("IC [poly] += [shape => {shape_id}, key_id => {key_id}, prop_pos => {val_pos}]"); // debug
                    self.entries[1] = ICEntry { key_id, shape: shape_id, val_pos };
                } else {
                    let replace_pos = key_id & 1; // heuristic: if the key ID is even, replace 0th. Replace 1st otherwise.

                    // println!("IC [poly] ~= [shape => {shape_id}, key_id => {key_id}, prop_pos => {val_pos}]"); // debug
                    self.entries[replace_pos] = ICEntry { key_id, shape: shape_id, val_pos };
                }
            },
            _ => {
                // println!("IC [dead] => it's cooked");
            }
        }
    }

    pub fn find(&mut self, shape_id: i32, key_id: usize) -> Option<usize> {
        let entry_0 = &self.entries[0];
        let entry_1 = &self.entries[1];

        match self.state {
            ICState::Unset => {
                // println!("IC [unset] --> IC[mono]");
                self.state = ICState::Mono;
                None
            },
            ICState::Mono => {
                if entry_0.shape == shape_id && entry_0.key_id == key_id {
                    // println!("IC [mono]: HIT"); // debug
                    Some(entry_0.val_pos)
                } else {
                    // println!("IC [mono]: MISS"); // debug
                    self.misses += 1;
                    self.state = Self::transition(self.misses);
                    None
                }
            },
            ICState::Poly => {
                if entry_0.shape == shape_id && entry_0.key_id == key_id {
                    // println!("IC [poly]: HIT ENTRY 0");
                    Some(entry_0.val_pos)
                } else if entry_1.shape == shape_id && entry_1.key_id == key_id {
                    // println!("IC [poly]: HIT ENTRY 1");
                    Some(entry_1.val_pos)
                } else {
                    // println!("IC [poly]: MISS");
                    self.misses += 1;
                    self.state = Self::transition(self.misses);
                    None
                }
            },
            ICState::Dead => {
                // println!("IC[dead]: DEAD");
                None
            },
        }
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum JSFuncFlag {
    NeedsEnv = (1 << 0),
    IsNative = (1 << 1),
    /// **NOTE:** Unused for now: indicates whether the special `=>` function behavior applies:
    /// - 1: No captures.
    /// - 2: `this` is `undefined`.
    IsArrow = (1 << 2),
    IsStrict = (1 << 3),
}

#[derive(Debug, Default, Clone)]
pub struct Chunk {
    pub icaches: Vec<InlineCache>,
    pub consts: Vec<JSValue>,
    pub code: Vec<Instruction>,
    pub arity: u16,
    /// See `JSFuncFlag` at runtime/code.rs for possible flags.
    pub flags: u8,
}

pub struct Program {
    /// GC-sweepable pool of objects
    pub heap: ItemPool<JSObjPtr, JS_OBJECT_COST>,
    /// Interned string pool
    pub spool: ItemPool<JSStrPtr, JS_STRING_COST>,
    /// Bytecode of JS code Boxes. This is for pointer stability so each exotic object can have a mutable-view ptr to the same chunk address.
    pub chunks: Vec<Box<Chunk>>,
    /// Saved script file-path
    pub name: String,
}

pub fn dump_chunk(chunk: &Chunk, id_num: Option<u16>) {
    let Chunk { consts, code , .. } = chunk;

    if let Some(chunk_id) = id_num {
        println!("---- Chunk of oid-{chunk_id} ----\n\n");
    } else {
        println!("---- Chunk of MAIN ----\n\n");
    }

    println!("--- CONSTS ---\n");

    for (cid, constant) in consts.iter().enumerate() {
        println!("\tC{cid} = {constant}");
    }

    println!("--- CODE ---\n");

    for (ip, inst) in code.iter().enumerate() {
        println!("\t{ip}: {inst}");
    }

    println!("--- END CHUNK ---\n");
}

pub fn dump_bytecode(program: &Program) {
    let Program {heap , chunks, name, ..} = program;

    println!("---- PROGRAM '{name}' ----\n--- MAIN ---\n");

    dump_chunk(chunks.last().unwrap(), None);

    println!("--- FUNCTIONS ---\n");

    for (oid, func_cell) in heap.items.iter().enumerate() {
        if let Some(object_cell) = func_cell {
            unsafe {
                let chunk_ptr = object_cell.borrow().opaque.as_bytecode();

                if !chunk_ptr.is_null() {
                    let arity = chunk_ptr.as_ref_unchecked().arity;
                    let flags = chunk_ptr.as_ref_unchecked().flags;

                    println!("\x1b[1;33mFunction\x1b[0m(arity = \x1b[1;31m{arity}\x1b[0m, flags = \x1b[1;31m{flags}\x1b[0m)\n");
                    dump_chunk(chunk_ptr.as_ref_unchecked(), Some(oid as u16));
                }
            }
        }

        if oid > heap.next_id as usize {
            break;
        }
    }
}
