use core::num::ParseIntError;
use std::cell::RefCell;
use std::fmt::Display;

use crate::runtime::values::JSValue;
use crate::runtime::shape::{DUD_SHAPE_ID, Shape};
use crate::runtime::property::{AddPropHint, PropFlag, Property};
use crate::runtime::code::{Instruction, InlineCache, JSFuncFlag, Chunk, Program};
use crate::runtime::closure::JSClosure;
use crate::runtime::opaque::{JSInternalTag, JSOpaque};
use crate::runtime::objects::{DUD_POOL_ID, ExoticObject, ItemPool, JS_OBJECT_COST, JS_STRING_COST, JSObjPtr, JSStrPtr, JS_CLOSURE_COST, JSClosurePtr, ShapePool};

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
    pub env_v: JSValue,
    pub caller_v: JSValue,
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
            caller_v: JSValue::Undefined,
            caller_rip: std::ptr::null(),
            caller_icp: std::ptr::null_mut(),
            caller_cvp: std::ptr::null(),
            caller_bp: 0,
            callee_bp: 0,
        }
    }
}

/// Type alias for Rust-native functions to call from FerroJS. These natives are "trampolined" into from a wrapping `JSFunction`: That chunk would have `PushUndef, NativeCall, Ret` which would defer to natives without a VM vs. native check.
pub type NativeFn = unsafe fn(*mut JSContext, u16) -> bool;

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
    pub closures: ItemPool<JSClosurePtr, JS_CLOSURE_COST>,
    pub shapes: ShapePool,
    pub frames: Vec<CallFrame>,
    /// Holds pre-allocated buffer of JSValues.
    pub stack: Vec<JSValue>,
    pub top_code: Vec<Box<Chunk>>,
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
        // ! NOTE: the last Chunk pushed to program.chunks is the top-level code. See emitter.rs in its last method.
        let start_ip = program.chunks.last().unwrap().code.as_ptr();
        let start_icp = program.chunks.last_mut().unwrap().icaches.as_mut_ptr();
        let start_cvp = program.chunks.last().unwrap().consts.as_ptr();

        let first_env_id = program.heap.add_item(Some(RefCell::new(
            ExoticObject {
                props: vec![],
                items: vec![],
                in_proto: JSValue::Undefined,
                out_proto: JSValue::Undefined,
                opaque: JSOpaque::default(),
                shape: DUD_SHAPE_ID
            }
        ))).unwrap();

        let mut first_frame = CallFrame {
            env_v: JSValue::ObjectId(first_env_id),
            caller_v: JSValue::Undefined,
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
            closures: ItemPool::<JSClosurePtr, JS_CLOSURE_COST>::new(shape_population),
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
            top_code: program.chunks,
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

        if let Some(new_env_oid) = self.heap.add_item(Some(RefCell::new(
            ExoticObject {
                props: vec![],
                items: vec![],
                in_proto: current_env_v,
                out_proto: JSValue::Undefined,
                opaque: JSOpaque::default(),
                shape: 0,
            }
        ))) {
            JSValue::ObjectId(new_env_oid)
        } else {
            self.status = EvalStatus::BadAlloc;
            JSValue::Undefined
        }
    }

    pub fn create_blank_obj(&mut self) -> JSValue {
        let Some(prepared_oid) = self.heap.add_item(Some(RefCell::new(
            ExoticObject::default()
        ))) else {
            return JSValue::Undefined;
        };

        JSValue::ObjectId(prepared_oid)
    }

    pub fn create_closure_obj(&mut self, func_oid: i32) -> JSValue {
        let maybe_closure: Option<JSClosure> = if let Some(func_chunk) = self.heap.get_item_mut(func_oid) {
            unsafe {
                let func_code = func_chunk.opaque.as_bytecode();

                if func_code.is_null() {
                    eprintln!("Failed to get function code of ferrojs-oid-{func_oid}");
                    None
                } else {
                    Some(JSClosure::new(
                        self.create_child_env(),
                        func_code
                    ))
                }
            }
        } else {
            None
        };

        if maybe_closure.is_none() {
            eprintln!("Failed to create closure of ferrojs-oid-{func_oid}");
            self.status = EvalStatus::BadAccess;
            return JSValue::Null;
        }

        let closure = maybe_closure.unwrap();

        let Some(closure_id) = self.closures.add_item(Some(RefCell::new(
            closure
        ))) else {
            eprintln!("Failed to save closure of ferrojs-oid-{func_oid}");
            self.status = EvalStatus::BadAlloc;
            return JSValue::Null;
        };

        self.heap.add_item(
            Some(RefCell::new(ExoticObject::with_opaque(
                JSOpaque::closure_id(closure_id)
            )))
        )
        .map(|cls_oid| Some(JSValue::ObjectId(cls_oid)))
        .unwrap_or(Some(JSValue::Null)).unwrap()
    }

    pub fn try_invoke_obj(&mut self, v: &JSValue, argc: u16) -> EvalStatus {
        let Some(func_oid) = v.get_obj_id() else {
            return EvalStatus::BadOp;
        };

        let JSOpaque {internal, tag, ..} = self.heap.get_item(func_oid).expect("Expected opaque data for object ID at ctx.rs ~ try_invoke_obj").opaque;

        unsafe {
            let (chunk_p, env_v) = match tag {
                JSInternalTag::Code => {
                    // println!("DEBUG ctx.rs ~ try_invoke_obj: run normal function of oid-{}", func_oid);
                    (
                        internal.code,
                        if 0 != (internal.code.as_ref().unwrap().flags & JSFuncFlag::NeedsEnv as u8) {
                            self.create_child_env()
                        } else {
                            self.get_curr_env()
                        }
                    )
                },
                JSInternalTag::ClosureID => {
                    // println!("DEBUG ctx.rs ~ try_invoke_obj: run closure-id-{}", internal.closure_id);
                    let JSClosure {env, code} = self.closures.get_item(internal.closure_id).expect("Expected valid closure at ctx.rs ~ try_invoke_obj");

                    (*code, *env)
                },
                _ => {
                    println!("DEBUG ctx.rs ~ try_invoke_obj: invalid, non-callable object...");
                    (std::ptr::null_mut(), JSValue::Undefined)
                }
            };

            let caller_rip = self.ip.add(1);
            let caller_icp = self.icp;
            let caller_cvp = self.cvp;
            let caller_bp = self.bp;
            let callee_bp = self.sp - argc as i32; // callee ref

            // print!("In funcs.rs, JSFunction::call:\nenv_js_value = ");
            // dbg!(env_jsvalue);

            self.frames.push(CallFrame {
                env_v,
                caller_v: self.frames.last().unwrap().caller_v,
                caller_rip,
                caller_icp,
                caller_cvp,
                caller_bp,
                callee_bp
            });

            self.bp = callee_bp;
            self.ip = chunk_p.as_ref_unchecked().code.as_ptr();
            self.cvp = chunk_p.as_ref_unchecked().consts.as_ptr();
            self.icp = chunk_p.as_mut_unchecked().icaches.as_mut_ptr();
        }

        self.cd += 1;

        dbg!(func_oid, self.cd, self.ip, self.cvp, self.icp);

        EvalStatus::Pending
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

    pub fn check_property_is_accessor(&self, oid: i32, key_id: usize) -> bool {
        let my_shape_id = self.heap.get_item(oid).unwrap().shape;

        let prop_offset = if let Some(slow_prop_ref) = self.shapes.fetch(my_shape_id) {
            slow_prop_ref.resolve_offset(key_id)
        } else { None };

        let parent_env_oid = self.heap.get_item(oid).unwrap().in_proto.get_obj_id();

        if prop_offset.is_none() {
            if parent_env_oid.is_none() {
                return false;
            }

            return self.check_property_is_accessor(parent_env_oid.unwrap(), key_id);
        }

        self.heap.get_item(oid).unwrap().props.get(prop_offset.unwrap()).unwrap().is_accessor()
    }

    pub fn get_property_data_value(&mut self, oid: i32, key_id: usize, ic_id: u16, try_use_getter: bool) -> Option<JSValue> {
        if oid == DUD_POOL_ID {
            println!("End of object parent chain found.");
            return None;
        }

        println!("key of str-id-{key_id}: '{}'", self.spool.get_item(key_id as i32).expect("Expected valid string constant for ID in get_property_data_value"));

        let my_shape_id = self.heap.get_item(oid).unwrap().shape;

        println!("DEBUG get_property_data_value: oid = {oid}, key_id = {key_id}");

        let (prop_offset, ic_dirty) = unsafe {
            if let Some (ic_slot) = self.icp.add(ic_id as usize).as_mut().unwrap().find(my_shape_id, key_id) {
                println!("Debug: try IC...");
                dbg!(my_shape_id, key_id);
                (Some(ic_slot), false)
            } else if let Some(slow_prop_ref) = self.shapes.fetch(my_shape_id) {
                println!("Debug: try Shape...");
                dbg!(my_shape_id, key_id);
                (slow_prop_ref.resolve_offset(key_id), true)
            } else { (None, true) }
        };

        let parent_env_oid = self.heap.get_item(oid).unwrap().in_proto.get_obj_id();

        // println!("In objects.rs at JSObjectWrap::get_property_data_value:");
        // dbg!(my_data.in_proto);
        // dbg!(prop_offset);

        if prop_offset.is_none() {
            let parent_oid = parent_env_oid.unwrap_or(DUD_POOL_ID);

            return self.get_property_data_value(parent_oid, key_id, ic_id, try_use_getter);
        }

        if ic_dirty {
            unsafe {
                self.icp.add(ic_id as usize).as_mut_unchecked().update(my_shape_id, key_id, prop_offset.unwrap());
            }
        }

        let prop_ref = self.heap.get_item(oid).unwrap().props.get(prop_offset.unwrap()).unwrap();

        Some(if !prop_ref.is_accessor() || try_use_getter {
            prop_ref.body[0]
        } else {
            prop_ref.body[1]
        })
    }

    pub fn get_property_data_mut(&mut self, oid: i32, key_id: usize, ic_id: u16) -> Option<&mut JSValue> {
        let my_shape_id = self.heap.get_item(oid).unwrap().shape;

        let (prop_offset, ic_dirty) = unsafe {
            if let Some (ic_slot) = self.icp.add(ic_id as usize).as_mut().unwrap().find(my_shape_id, key_id) {
                (Some(ic_slot), false)
            } else if let Some(slow_prop_ref) = self.shapes.fetch(my_shape_id) {
                (slow_prop_ref.resolve_offset(key_id), true)
            } else { (None, true) }
        };

        let parent_env_oid = self.heap.get_item(oid).unwrap().in_proto.get_obj_id();

        if prop_offset.is_none() {
            parent_env_oid?;

            return self.get_property_data_mut(parent_env_oid.unwrap(), key_id, ic_id);
        }

        if ic_dirty {
            unsafe {
                self.icp.add(ic_id as usize).as_mut_unchecked().update(my_shape_id, key_id, prop_offset.unwrap());
            }
        }

        self.heap.get_item_mut(oid).unwrap().props.get_mut(prop_offset.unwrap()).unwrap().body.first_mut()
    }

    pub fn set_property_data_mut(&mut self, oid: i32, key_id: usize, ic_id: u16, hint: AddPropHint, arg: &JSValue) -> bool {
        let my_shape_id = self.heap.get_item(oid).unwrap().shape;

        println!("key of str-id-{key_id}: '{}'", self.spool.get_item(key_id as i32).expect("Expected valid string constant for ID in set_property_data_mut"));
        println!("DEBUG set_property_data_mut: oid = {oid}, key_id = {key_id}");

        let (prop_offset, ic_dirty, shape_dirty) = unsafe {
            if let Some (ic_slot) = self.icp.add(ic_id as usize).as_mut().unwrap().find(my_shape_id, key_id) {
                (Some(ic_slot), false, false)
            } else if let Some(slow_prop_ref) = self.shapes.fetch(my_shape_id) {
                if let Some(present_prop_offset) = slow_prop_ref.resolve_offset(key_id) {
                    (Some(present_prop_offset), true, false)
                } else {
                    (Some(self.heap.get_item(oid).unwrap().props.len()), true, true)
                }
            } else { (None, true, true) }
        };

        if prop_offset.is_none() {
            return false;
        }

        if shape_dirty {
            let maybe_old_shape_child_id = self.shapes.fetch_mut(my_shape_id).unwrap().resolve_subshape_id(key_id);

            if let Some(existing_child_shape_id) = maybe_old_shape_child_id {
                self.heap.get_item_mut(oid).unwrap().shape = existing_child_shape_id;
            } else {
                let temp_child_shape = self.shapes.fetch_mut(my_shape_id).unwrap().derive_child(key_id, self.heap.get_item(oid).unwrap().shape);
                let temp_child_shape_id = self.shapes.store(temp_child_shape).unwrap();
                self.heap.get_item_mut(oid).unwrap().shape = temp_child_shape_id;
                self.shapes.fetch_mut(my_shape_id).unwrap().add_transition(key_id, temp_child_shape_id);
            }

            match hint {
                AddPropHint::Data => self.heap.get_item_mut(oid).unwrap().props.push(Property::data(arg, PropFlag::Writable as u8 | PropFlag::Configurable as u8)),
                _ => self.heap.get_item_mut(oid).unwrap().props.push(Property::accessor(&JSValue::Null, &JSValue::Null, PropFlag::Writable as u8 | PropFlag::Configurable as u8 | PropFlag::HasGetter as u8 | PropFlag::HasSetter as u8))
            }
        }

        if ic_dirty {
            unsafe {
                self.icp.add(ic_id as usize).as_mut_unchecked().update(my_shape_id, key_id, prop_offset.unwrap());
            }
        }

        let prop_index = prop_offset.unwrap();

        if hint == AddPropHint::Setter {
            self.heap.get_item_mut(oid).unwrap().props.get_mut(prop_index).unwrap().body[1] = *arg;
            true
        } else if hint != AddPropHint::Noop {
            self.heap.get_item_mut(oid).unwrap().props.get_mut(prop_index).unwrap().body[0] = *arg;
            true
        } else {
            false
        }
    }
}
