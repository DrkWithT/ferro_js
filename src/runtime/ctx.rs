use crate::runtime::values::JSValue;
use crate::runtime::code::{Instruction};
use crate::runtime::objects::{ItemPool, JSObjPtr, JSStrPtr, JS_OBJECT_COST, JS_STRING_COST};


#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EvalStatus {
    Pending,
    Ok,
    BadOp,
    BadAccess,
    BadAlloc,
}

#[derive(Debug, Clone)]
pub struct CallFrame {
    pub this_p: JSObjPtr,
    pub caller_rip: *const Instruction,
    pub caller_bp: i32,
    pub callee_bp: i32,
}

pub struct JSContext {
    /// Holds and manages JS object memory.
    pub heap: ItemPool<JSObjPtr, JS_OBJECT_COST>,
    /// Holds interned strings.
    pub spool: ItemPool<JSStrPtr, JS_STRING_COST>,
    pub frames: Vec<CallFrame>,
    /// Holds pre-allocated buffer of JSValues.
    pub stack: Vec<JSValue>,
    pub ip: *const Instruction,
    pub bp: i32,
    pub sp: i32,
    pub status: EvalStatus,
}
