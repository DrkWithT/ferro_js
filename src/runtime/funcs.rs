use crate::runtime::objects::{ExoticObject};
use crate::runtime::code::Chunk;
use crate::runtime::ctx::{JSContext, EvalStatus};

pub type NativeFn = unsafe fn(*mut JSContext, u16) -> bool;

#[derive(Debug, Clone)]
pub enum FuncBody {
    Native(NativeFn),
    Bytecode(Chunk),
}

#[derive(Debug, Clone)]
pub struct JSFunction {
    pub body: FuncBody,
    pub data: ExoticObject,
    /// For `Function.length`
    pub arity: u16,
    /// Unused for now: indicates whether the special `=>` function behavior applies:
    /// - 1: No captures.
    /// - 2: `this` is `undefined`.
    pub is_arrow: bool,
}

impl JSFunction {
    pub fn native(f: NativeFn, data_object: ExoticObject, arity: u16) -> Self {
        Self {
            body: FuncBody::Native(f),
            data: data_object,
            arity,
            is_arrow: false
        }
    }

    pub fn bcode(f: Chunk, data_object: ExoticObject, arity: u16, is_arrow: bool) -> Self {
        Self {
            body: FuncBody::Bytecode(f),
            data: data_object,
            arity,
            is_arrow
        }
    }

    #[allow(unused)]
    pub fn call(&mut self, state: &mut JSContext, argc: u16) -> EvalStatus {
        // todo
        eprintln!("Unimplemented Function.[[Call]]");
        EvalStatus::BadOp
    }

    #[allow(unused)]
    pub fn call_with(&mut self, state: &mut JSContext, argc: u16) -> EvalStatus {
        // todo
        eprintln!("Unimplemented Function.call(thisArg, ...)");
        EvalStatus::BadOp
    }

    #[allow(unused)]
    pub fn call_ctor(&mut self, state: &mut JSContext, argc: u16) -> EvalStatus {
        // todo
        eprintln!("Unimplemented Function.[[Construct]]");
        EvalStatus::BadOp
    }
}
