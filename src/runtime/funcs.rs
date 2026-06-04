use crate::runtime::values::JSValue;
use crate::runtime::code::Chunk;
use crate::runtime::ctx::JSContext;

pub type NativeFn = unsafe fn(*mut JSContext, u16) -> bool;

#[derive(Debug, Clone)]
pub enum FuncBody {
    Native(NativeFn),
    Bytecode(Chunk),
}

#[derive(Debug, Clone)]
pub struct JSFunction {
    pub body: FuncBody,
    /// [[Prototype]] attribute for implementation.
    pub in_pt: JSValue,
    /// exposed Prototype for implementation.
    pub out_pt: JSValue,
    pub arity: u16,
}
