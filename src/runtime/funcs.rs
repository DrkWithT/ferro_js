use crate::runtime::objects::{ExoticObject};
use crate::runtime::code::{Opcode, Instruction, Chunk};
use crate::runtime::ctx::{CallFrame, EvalStatus, JSContext};

/// Type alias for Rust-native functions to call from FerroJS. These natives are "trampolined" into from a wrapping `JSFunction`: That chunk would have `PushUndef, NativeCall, Ret` which would defer to natives without a VM vs. native check.
pub type NativeFn = unsafe fn(*mut JSContext, u16) -> bool;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum JSFuncFlag {
    NeedsEnv = (1 << 0),
    IsNative = (1 << 1),
    /// **NOTE:** Unused for now: indicates whether the special `=>` function behavior applies:
    /// - 1: No captures.
    /// - 2: `this` is `undefined`.
    IsArrow = (1 << 2),
}

#[derive(Debug, Clone)]
pub struct JSFunction {
    pub body: Chunk,
    pub data: ExoticObject,
    /// For `Function.length`
    pub arity: u16,
    pub flags: u8,
}

impl JSFunction {
    pub fn native(data_object: ExoticObject, native_id: i32, arity: u16) -> Self {
        Self {
            body: Chunk {
                icaches: vec![],
                consts: vec![],
                code: vec![
                    Instruction {
                        arg: native_id,
                        flags: 0, // ? NOTE: the arity of the function is automatically assumed... if arity < argc, other args are undefined.
                        op: Opcode::NativeCall
                    },
                    Instruction {
                        arg: 0,
                        flags: 0,
                        op: Opcode::Ret
                    }
                ]
            },
            data: data_object,
            arity,
            flags: JSFuncFlag::NeedsEnv as u8 | JSFuncFlag::IsNative as u8
        }
    }

    pub fn bcode(data_object: ExoticObject, code: Chunk, arity: u16, needs_env: bool, is_arrow: bool) -> Self {
        Self {
            body: code,
            data: data_object,
            arity,
            flags: {
                // ? NOTE: the arity of the function is automatically assumed... if arity < argc, other args are undefined.
                let bitflag_needs_env = if needs_env { JSFuncFlag::NeedsEnv as u8 } else { 0 };
                let bitflag_is_arrow = if is_arrow { JSFuncFlag::IsArrow as u8 } else { 0 };

                bitflag_needs_env | bitflag_is_arrow
            }
        }
    }

    pub fn get_flag(&self, flag: JSFuncFlag) -> bool {
        0 != self.flags & flag as u8
    }

    #[allow(unused)]
    pub fn call(&mut self, state: &mut JSContext, argc: u16) -> EvalStatus {
        unsafe {
            let caller_rip = state.ip.add(1);
            let caller_icp = state.icp;
            let caller_cvp = state.cvp;
            let caller_bp = state.bp;
            let callee_bp = state.sp - argc as i32; // callee ref
            let Some(callee_oid) = state.stack[callee_bp as usize].get_obj_id() else {
                return EvalStatus::BadOp;
            };

            let env_jsvalue = if self.get_flag(JSFuncFlag::NeedsEnv) { state.create_child_env() } else { state.get_curr_env() };

            state.frames.push(CallFrame {
                this_p: env_jsvalue,
                callee_p: state.frames.last().unwrap().callee_p,
                caller_rip,
                caller_icp,
                caller_cvp,
                caller_bp,
                callee_bp
            });

            state.bp = callee_bp;
            state.ip = self.body.code.as_ptr();
            state.cvp = self.body.consts.as_ptr();
            state.icp = self.body.icaches.as_mut_ptr();
        }

        state.cd += 1;

        EvalStatus::Pending
    }

    #[allow(unused)]
    pub fn call_ctor(&mut self, state: &mut JSContext, argc: u16) -> EvalStatus {
        eprintln!("Unimplemented Function.[[Construct]]"); // todo: implement this after objects start working properly with accessors.
        EvalStatus::BadOp
    }
}
