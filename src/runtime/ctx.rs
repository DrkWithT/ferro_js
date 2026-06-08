use std::cell::RefCell;
use std::fmt::Display;
use std::rc::Rc;

use crate::runtime::values::JSValue;
use crate::runtime::code::{Instruction, InlineCache, Chunk, Program};
use crate::runtime::objects::{DUD_SHAPE_ID, ExoticObject, ItemPool, JS_OBJECT_COST, JS_STRING_COST, JSObjPtr, JSObjectWrap, JSStrPtr, Shape, ShapePool};


#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EvalStatus {
    Pending,
    Ok,
    BadOp,
    BadAccess,
    BadAlloc,
}

impl Display for EvalStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", match self {
            Self::Ok => "OK",
            Self::BadOp => "Invalid / unimplemented opcode!",
            Self::BadAccess => "Invalid object reference / property access!",
            Self::BadAlloc => "Invalid allocation of VM heap!",
            _ => "(unknown error)"
        })
    }
}

#[derive(Debug)]
pub struct CallFrame {
    /// Contains `FuncEnvironment.[[This]]` and follows these cases:
    ///  - 1: Holds a new object environment upon ctor calls.
    ///  - 2: Holds a custom `this` Value from `Function.call()`.
    ///  - 3: If `use strict` applies, do not coerce `this` to globalThis. Otherwise, do so.
    pub this_p: *mut JSObjectWrap,
    pub callee_p: *mut JSObjectWrap,
    pub caller_rip: *const Instruction,
    pub caller_cvp: *const JSValue,
    pub caller_bp: i32,
    pub callee_bp: i32,
}

impl Default for CallFrame {
    fn default() -> Self {
        Self {
            this_p: std::ptr::null_mut(),
            callee_p: std::ptr::null_mut(),
            caller_rip: std::ptr::null(),
            caller_cvp: std::ptr::null(),
            caller_bp: 0,
            callee_bp: 0,
        }
    }
}

pub struct JSContext {
    /// Holds and manages JS object memory.
    pub heap: ItemPool<JSObjPtr, JS_OBJECT_COST>,
    /// Holds interned strings.
    pub spool: ItemPool<JSStrPtr, JS_STRING_COST>,
    pub shapes: ShapePool,
    pub frames: Vec<CallFrame>,
    /// Holds pre-allocated buffer of JSValues.
    pub stack: Vec<JSValue>,
    pub top_code: Chunk,
    pub ip: *const Instruction,
    pub icp: *mut InlineCache,
    pub cvp: *const JSValue,
    pub bp: i32,
    pub sp: i32,
    /// recursion depth
    pub cd: u16,
    pub cm: u16,
    pub status: EvalStatus,
}

impl JSContext {
    pub fn new(shape_population: usize, stack_sizing: usize, calls_max: u16, mut program: Program) -> Self {
        let start_ip = program.top_level.code.as_ptr();
        let start_icp = program.top_level.icaches.as_mut_ptr();
        let start_cvp = program.top_level.consts.as_ptr();

        let mut first_frame = CallFrame {
            this_p: std::ptr::null_mut(),
            callee_p: std::ptr::null_mut(),
            caller_rip: std::ptr::null(),
            caller_cvp: start_cvp,
            caller_bp: 0,
            callee_bp: 0
        };

        let first_env_id = program.heap.add_item(Some(Rc::new(RefCell::new(JSObjectWrap::Exotic(
            ExoticObject {
                props: vec![],
                items: vec![],
                shape: DUD_SHAPE_ID
            }
        ))))).unwrap();

        first_frame.this_p = program.heap.get_item(first_env_id).unwrap().as_ptr();

        Self {
            heap: std::mem::take(&mut program.heap),
            spool: std::mem::take(&mut program.spool),
            shapes: {
                let mut shape_buf = Vec::<Shape>::with_capacity(shape_population);
                shape_buf.resize_with(shape_population, || {
                    Shape::default()
                });

                ShapePool {
                    shapes: shape_buf,
                    next_sid: 1
                }
            },
            frames: vec![
                first_frame
            ],
            stack: {
                let mut temp_stack = Vec::<JSValue>::with_capacity(stack_sizing);

                temp_stack.resize(stack_sizing, JSValue::undefined());

                temp_stack
            },
            top_code: program.top_level,
            ip: start_ip,
            icp: start_icp,
            cvp: start_cvp,
            bp: 0,
            sp: 1,
            cd: 1,
            cm: calls_max,
            status: EvalStatus::Pending
        }
    }
}
