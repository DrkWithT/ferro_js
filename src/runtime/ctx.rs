use core::num::ParseIntError;
use std::cell::RefCell;
use std::fmt::Display;
use std::rc::Rc;

use crate::runtime::values::JSValue;
use crate::runtime::code::{Instruction, InlineCache, Chunk, Program};
use crate::runtime::objects::{DUD_POOL_ID, DUD_SHAPE_ID, ExoticObject, ItemPool, JS_OBJECT_COST, JS_STRING_COST, JSObjPtr, JSObjectWrap, JSStrPtr, Shape, ShapePool};

/// ### ABOUT
/// Indicates status of VM execution. See each enum member for a quick description.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EvalStatus {
    /// Execution is WIP
    Pending,
    /// Finished with no errors (excluding logical errors)
    Ok,
    /// Finished with unsupported / failed opcode
    BadOp,
    /// Finished with bad reference / property access (JS XXXError comes later)
    BadAccess,
    /// Finished with memory error i.e heap failed to allocate an object
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

/// ## ABOUT
/// Tracks callee-caller state for the VM.
#[derive(Debug)]
pub struct CallFrame {
    /// Contains `FuncEnvironment.[[This]]` and follows these cases:
    ///  - 1: Holds a new object environment upon ctor calls.
    ///  - 2: Holds a custom `this` Value from `Function.call()`.
    ///  - 3: If `use strict` applies, do not coerce `this` to globalThis. Otherwise, do so.
    pub env_v: JSValue, // ! FIXME: use JSValue instead to simplify property accesses later with object-id values + key-value...
    pub callee_p: *mut JSObjectWrap, // ! FIXME: use JSValue
    pub caller_rip: *const Instruction,
    pub caller_icp: *mut InlineCache,
    pub caller_cvp: *const JSValue,
    pub caller_bp: i32,
    pub callee_bp: i32,
}

impl Default for CallFrame {
    fn default() -> Self {
        Self {
            env_v: JSValue::Undefined,
            callee_p: std::ptr::null_mut(),
            caller_rip: std::ptr::null(),
            caller_icp: std::ptr::null_mut(),
            caller_cvp: std::ptr::null(),
            caller_bp: 0,
            callee_bp: 0,
        }
    }
}

/// ## ABOUT
/// Stores all state of the interpreter, primarily for the VM's execution and memory management.
/// ### TODOs
/// 1. Add a mark-and-sweep GC.
/// 2. Improve IC & Shape system to require less indirection and spaghetti code.
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
    /// ### ABOUT
    /// Constructs a new JSContext for the interpreter's VM.
    pub fn new(shape_population: usize, stack_sizing: usize, calls_max: u16, mut program: Program) -> Self {
        let start_ip = program.top_level.code.as_ptr();
        let start_icp = program.top_level.icaches.as_mut_ptr();
        let start_cvp = program.top_level.consts.as_ptr();

        let first_env_id = program.heap.add_item(Some(Rc::new(RefCell::new(JSObjectWrap::Exotic(
            ExoticObject {
                props: vec![],
                items: vec![],
                in_proto: JSValue::Undefined,
                out_proto: JSValue::Undefined,
                shape: DUD_SHAPE_ID
            }
        ))))).unwrap();

        let mut first_frame = CallFrame {
            env_v: JSValue::ObjectId(first_env_id),
            callee_p: std::ptr::null_mut(),
            caller_rip: std::ptr::null(),
            caller_icp: std::ptr::null_mut(),
            caller_cvp: std::ptr::null(),
            caller_bp: 0,
            callee_bp: 1
        };

        first_frame.env_v = JSValue::ObjectId(first_env_id);

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

                // ? NOTE: Set `globalThis` to reference the global environment object (reserved at stack[CALLEE_BP - 1]).
                temp_stack[0] = JSValue::ObjectId(first_env_id);

                temp_stack
            },
            top_code: program.top_level,
            ip: start_ip,
            icp: start_icp,
            cvp: start_cvp,
            bp: 1,
            sp: 1, // ! NOTE: maybe adjust this?
            cd: 1,
            cm: calls_max,
            status: EvalStatus::Pending
        }
    }

    pub fn get_curr_env(&self) -> JSValue {
        self.frames.last().expect("Expected available environment at ctx.rs ~ get_curr_env").env_v
    }

    pub fn create_child_env(&mut self) -> JSValue {
        let current_env_v = self.frames.last().expect("Expected available environment at ctx.rs ~ create_child_env").env_v;

        if let Some(new_env_oid) = self.heap.add_item(Some(Rc::new(RefCell::new(JSObjectWrap::Exotic(
            ExoticObject {
                props: vec![],
                items: vec![],
                in_proto: current_env_v,
                out_proto: JSValue::Undefined,
                shape: 0,
            }
        ))))) {
            JSValue::ObjectId(new_env_oid)
        } else {
            self.status = EvalStatus::BadAlloc;
            JSValue::Undefined
        }
    }

    pub fn create_blank_obj(&mut self) -> JSValue {
        let Some(prepared_oid) = self.heap.add_item(Some(Rc::new(RefCell::new(
            JSObjectWrap::Exotic(
                ExoticObject::default()
            )
        )))) else {
            return JSValue::Undefined;
        };

        JSValue::ObjectId(prepared_oid)
    }

    /// ### ABOUT
    /// Implements most of the important baseline conversion of JS values to JS numbers (double-precision floats). Objects to numbers are unsupported for now.
    pub fn jsvalue_to_number(&self, v: &JSValue) -> f64 {
        match v {
            JSValue::Undefined => f64::NAN,
            JSValue::Null => 0.0,
            JSValue::Boolean(b) => if *b {1.0} else {0.0},
            JSValue::Number(x) => *x,
            JSValue::StringId(sid) => {
                let text = self.spool.get_item(*sid).unwrap_or("NAN");

                let is_signed = text.chars().nth(0).unwrap() == '-';

                let text = if is_signed { text.strip_prefix("-").unwrap() } else { text };

                // ! FIXME: implement exponentials later.
                (if is_signed {-1.0} else {1.0}) * (if text.starts_with("0x") {
                    i32::from_str_radix(text, 16u32)
                        .map(f64::from)
                        .or(Ok::<f64, ParseIntError>(f64::NAN))
                        .expect("Expected converted f64 (HEX number) at ctx.rs: jsvalue_to_number")
                } else if text.starts_with("0b") {
                    i32::from_str_radix(text, 2u32)
                        .map(f64::from)
                        .or(Ok::<f64, ParseIntError>(f64::NAN))
                        .expect("Expected converted f64 (BIN number) at ctx.rs: jsvalue_to_number")
                } else if text.starts_with("0") && text.len() > 1 {
                    i32::from_str_radix(text, 8u32)
                        .map(f64::from)
                        .or(Ok::<f64, ParseIntError>(f64::NAN))
                        .expect("Expected converted f64 (OCT number) at ctx.rs: jsvalue_to_number")
                } else if text.starts_with("0") && text.len() == 1 {
                    0.0
                } else {
                    str::parse::<f64>(text).expect("Expected valid f64 literal at ctx.rs: jsvalue_to_number")
                })
            },
            // ! FIXME: implement logic that tries object.valueOf(), see ES6: 7.1.3
            JSValue::ObjectId(_) => f64::NAN
        }
    }

    pub fn jsvalue_to_boolean(&self, v: &JSValue) -> bool {
        if let Some(sid) = v.get_str_id() {
            !self.spool.get_item(sid).as_ref().expect("Expected valid, interned string reference in vm.rs ~ jsvalue_to_boolean").is_empty()
        } else {
            v.get_boolean()
        }
    }

    /// ### ABOUT
    /// Implements basics of ES6: 7.1.5
    pub fn jsvalue_to_i32(&self, v: &JSValue) -> i32 {
        let raw_num = self.jsvalue_to_number(v);

        if raw_num.is_nan() {
            0i32
        } else if raw_num == 0.0 || raw_num.is_infinite() {
            raw_num as i32
        } else {
            f64::floor(raw_num) as i32
        }
    }

    /// ### ABOUT
    /// Implements basics of ES6: 7.1.6
    pub fn jsvalue_to_u32(&self, v: &JSValue) -> u32 {
        let raw_num = self.jsvalue_to_number(v);

        if raw_num.is_nan() || raw_num == 0.0 || raw_num.is_infinite() {
            raw_num as u32
        } else {
            f64::floor(raw_num) as u32
        }
    }

    /// ### ABOUT
    /// Implements a subset of Strict Equality algorithm for same typed values. This is a helper function for both strict equality and loose equality opcodes.
    pub fn jsvalue_test_same_types_eq(&self, lhs: &JSValue, rhs: &JSValue) -> bool {
        unsafe {
            match lhs {
                JSValue::Undefined | JSValue::Null => true,
                JSValue::Boolean(lhs_bool) => *lhs_bool == rhs.get_boolean(),
                JSValue::Number(f_value) => *f_value == rhs.get_number().unwrap_or(f64::NAN),
                JSValue::StringId(lhs_sid) => if *lhs_sid == rhs.get_str_id().unwrap_or(DUD_POOL_ID) {
                    true
                } else {
                    self.spool.get_item(*lhs_sid).unwrap().as_ptr().as_ref().expect("Expected valid interned string of LHS at vm.rs: op_strict_eq") == self.spool.get_item(rhs.get_str_id().unwrap()).unwrap().as_ptr().as_ref().expect("Expected valid interned string of RHS at vm.rs: op_strict_eq")
                },
                JSValue::ObjectId(lhs_oid) => *lhs_oid == rhs.get_obj_id().unwrap_or(DUD_POOL_ID)
            }
        }
    }
}
