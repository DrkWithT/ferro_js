use crate::runtime::values::{JSValue};
use crate::runtime::objects::{DUD_POOL_ID};
use crate::runtime::code::{Opcode};
use crate::runtime::ctx::{CallFrame, EvalStatus, JSContext};


pub const DEFAULT_JS_STACK_SIZE: usize = 16384;
pub const DEFAULT_JS_RECUR_LIMIT: u16 = 128;

unsafe fn op_push_undef(context: &mut JSContext, stack: *mut JSValue) {
    context.sp += 1;

    unsafe {
        *stack.offset(context.sp as isize) = JSValue::Undefined;
        context.ip = context.ip.add(1);
    }
}

unsafe fn op_push_null(context: &mut JSContext, stack: *mut JSValue) {
    context.sp += 1;

    unsafe {
        *stack.offset(context.sp as isize) = JSValue::Null;
        context.ip = context.ip.add(1);
    }
}

unsafe fn op_push_bool(context: &mut JSContext, stack: *mut JSValue) {
    context.sp += 1;
    
    unsafe {
        let ip_flags = context.ip.read().flags;

        *stack.offset(context.sp as isize) = JSValue::boolean(ip_flags == 1);
        context.ip = context.ip.add(1);
    }
}

unsafe fn op_push_nan(context: &mut JSContext, stack: *mut JSValue) {
    context.sp += 1;

    unsafe {
        *stack.offset(context.sp as isize) = JSValue::number(f64::NAN);
        context.ip = context.ip.add(1);
    }
}

unsafe fn op_push_inf(context: &mut JSContext, stack: *mut JSValue) {
    context.sp += 1;

    unsafe {
        *stack.offset(context.sp as isize) = JSValue::number(f64::INFINITY);
        context.ip = context.ip.add(1);
    }
}

unsafe fn op_push_neg_inf(context: &mut JSContext, stack: *mut JSValue) {
    context.sp += 1;

    unsafe {
        *stack.offset(context.sp as isize) = JSValue::number(f64::NEG_INFINITY);
        context.ip = context.ip.add(1);
    }
}

#[allow(unused)]
unsafe fn op_push_this_ref(context: &mut JSContext, stack: *mut JSValue) {
    context.status = EvalStatus::BadOp
}

#[allow(unused)]
unsafe fn op_push_str(context: &mut JSContext, stack: *mut JSValue) {
    context.sp += 1;

    unsafe {
        let sid: i32 = (*context.ip).arg;
        *stack.add(context.sp as usize) = JSValue::StringId(sid);
        context.ip = context.ip.add(1);
    }
}

unsafe fn op_push_const(context: &mut JSContext, stack: *mut JSValue) {
    context.sp += 1;

    unsafe {
        let constant_id = context.ip.read().arg;

        *stack.offset(context.sp as isize) = context.cvp.add(constant_id as usize).read();
        context.ip = context.ip.add(1);
    }
}

unsafe fn op_dup1(context: &mut JSContext, stack: *mut JSValue) {
    context.sp += 1;

    unsafe {
        *stack.add(context.sp as usize) = *stack.add((context.sp - 1) as usize);
        context.ip = context.ip.add(1);
    }
}

unsafe fn op_dup2(context: &mut JSContext, stack: *mut JSValue) {
    context.sp += 1;

    // ? Stack: a b ^ --> a a b ^
    unsafe {
        *stack.add(context.sp as usize) = *stack.add((context.sp - 1) as usize);
        *stack.add((context.sp - 1) as usize) = *stack.add((context.sp - 2) as usize);
        context.ip = context.ip.add(1);
    }
}

unsafe fn op_swap(context: &mut JSContext, stack: *mut JSValue) {
    let current_sp = context.sp as usize;
    
    // ? Stack: a b --> b a
    unsafe {
        std::ptr::swap(stack.add(current_sp), stack.add(current_sp - 1));
        context.ip = context.ip.add(1);
    }
}

unsafe fn op_pop_n(context: &mut JSContext, stack: *mut JSValue) {
    unsafe {
        let curr_sp = context.sp as usize;
        let pop_count = context.ip.read().flags;

        for peek_off in 0 .. pop_count {
            *stack.add(curr_sp - peek_off as usize) = JSValue::Undefined;
        }

        context.sp -= pop_count as i32;
        context.ip = context.ip.add(1);
    }
}

unsafe fn op_discard(context: &mut JSContext, stack: *mut JSValue) {
    unsafe {
        *stack.add(context.sp as usize) = JSValue::Undefined;
        context.ip = context.ip.add(1);
    }
}

unsafe fn op_get_local(context: &mut JSContext, stack: *mut JSValue) {
    context.sp += 1;

    unsafe {
        let local_id = context.ip.read().arg;
        *stack.add(context.sp as usize) = *stack.add((context.bp + local_id) as usize);
        context.ip = context.ip.add(1);
    }
}

unsafe fn op_set_local(context: &mut JSContext, stack: *mut JSValue) {
    unsafe {
        let local_id = context.ip.read().arg;

        *stack.add((context.bp + local_id) as usize) = *stack.add(context.sp as usize);
        context.sp -= 1;

        context.ip = context.ip.add(1);
    }
}

unsafe fn op_get_var(context: &mut JSContext, stack: *mut JSValue) {
    unsafe {
        let key_id = stack.add(context.sp as usize).read().get_str_id().unwrap_or(0) as usize;
        let ic_id = context.ip.read().flags;

        if let Some(var_value) = context.frames.last().expect("Expected JS lexical environment in vm.rs: op_get_var").this_p.as_ref().unwrap().get_property_data_value(context, key_id, ic_id) {
            *stack.add(context.sp as usize) = var_value;
            context.ip = context.ip.add(1);
        } else {
            eprintln!("See vm.rs ~ op_get_var");
            context.status = EvalStatus::BadAccess;
        }
    }
}

unsafe fn op_set_var(context: &mut JSContext, stack: *mut JSValue) {
    unsafe {
        let key_id = stack.add(context.sp as usize - 1).read().get_str_id().unwrap_or(0) as usize;
        let temp_value = stack.add(context.sp as usize).as_ref_unchecked();
        let ic_id = context.ip.read().flags;

        if context.frames.last().expect("Expected JS lexical environment in vm.rs: op_set_var").this_p.as_mut_unchecked().set_property_data_mut(context, key_id, ic_id, temp_value) {
            context.sp -= 2;
            context.ip = context.ip.add(1);
        } else {
            eprintln!("See vm.rs ~ op_set_var");
            context.status = EvalStatus::BadAccess;
        }
    }
}

#[allow(unused)]
unsafe fn op_make_obj(context: &mut JSContext, stack: *mut JSValue) {
    // todo
    context.status = EvalStatus::BadOp;
}

#[allow(unused)]
unsafe fn op_get_own_prop(context: &mut JSContext, stack: *mut JSValue) {
    // todo
    context.status = EvalStatus::BadOp;
}

#[allow(unused)]
unsafe fn op_set_own_prop(context: &mut JSContext, stack: *mut JSValue) {
    // todo
    context.status = EvalStatus::BadOp;
}

#[allow(unused)]
unsafe fn op_get_prop(context: &mut JSContext, stack: *mut JSValue) {
    // todo
    context.status = EvalStatus::BadOp;
}

#[allow(unused)]
unsafe fn op_set_prop(context: &mut JSContext, stack: *mut JSValue) {
    // todo
    context.status = EvalStatus::BadOp;
}

#[allow(unused)]
unsafe fn op_del_prop(context: &mut JSContext, stack: *mut JSValue) {
    // todo
    context.status = EvalStatus::BadOp;
}

#[allow(unused)]
unsafe fn op_get_proto(context: &mut JSContext, stack: *mut JSValue) {
    // todo
    context.status = EvalStatus::BadOp;
}

#[allow(unused)]
unsafe fn op_set_proto(context: &mut JSContext, stack: *mut JSValue) {
    // todo
    context.status = EvalStatus::BadOp;
}

unsafe fn op_force_bool(context: &mut JSContext, stack: *mut JSValue) {
    unsafe {
        let truthiness = match stack.add(context.sp as usize).as_ref().expect("Expected JSValue in stack at vm.rs: op_force_bool") {
            JSValue::Undefined | JSValue::Null => false,
            JSValue::Number(f64_value) => {
                !f64::is_nan(*f64_value) && *f64_value != 0.0f64
            },
            JSValue::Boolean(bool_value) => {
                *bool_value
            },
            _ => true
        };
        
        context.sp += 1;
        *stack.add(context.sp as usize) = JSValue::Boolean(truthiness);
        context.ip = context.ip.add(1);
    }
}

unsafe fn op_force_num(context: &mut JSContext, stack: *mut JSValue) {
    unsafe {
        let numeric_v = match stack.add(context.sp as usize).as_ref().expect("Expected JSValue in stack at vm.rs: op_force_num") {
            JSValue::Undefined | JSValue::ObjectId(_) => f64::NAN,
            JSValue::Null => 0.0,
            JSValue::Boolean(bool_value) => if *bool_value {1.0} else {0.0},
            JSValue::Number(f64_value) => *f64_value,
            JSValue::StringId(sid) => {
                // By the JS specification: +"..." will try a parsing conversion to a number.
                if let Some(sv) = context.spool.get_item(*sid) {
                    str::parse::<f64>(sv.as_ref().as_ptr().as_ref().expect("Expected valid string by ID at vm.rs: op_force_num")).unwrap_or(f64::NAN)
                } else {
                    f64::NAN
                }
            }
        };
        
        context.sp += 1;
        *stack.add(context.sp as usize) = JSValue::Number(numeric_v);
        context.ip = context.ip.add(1);
    }
}

unsafe fn op_neg_bool(context: &mut JSContext, stack: *mut JSValue) {
    unsafe {
        let truthiness = match stack.add(context.sp as usize).as_ref().expect("Expected JSValue in stack at vm.rs: op_neg_bool") {
            JSValue::Undefined | JSValue::Null => true,
            JSValue::Number(f64_value) => {
                f64::is_nan(*f64_value) || *f64_value == 0.0f64
            },
            JSValue::Boolean(bool_value) => {
                !*bool_value
            },
            _ => false
        };
        
        context.sp += 1;
        *stack.add(context.sp as usize) = JSValue::Boolean(truthiness);
        context.ip = context.ip.add(1);
    }
}

unsafe fn op_neg_num(context: &mut JSContext, stack: *mut JSValue) {
    unsafe {
        let src_num = if let Some(num_v) = stack.add(context.sp as usize).as_ref() {
            num_v.get_number().unwrap_or(f64::NAN)
        } else {
            f64::NAN
        };
        
        context.sp += 1;
        *stack.add(context.sp as usize) = JSValue::Number(src_num);
        context.ip = context.ip.add(1);
    }
}

#[allow(unused)]
unsafe fn op_mod(context: &mut JSContext, stack: *mut JSValue) {
    // todo
    context.status = EvalStatus::BadOp;
}

#[allow(unused)]
unsafe fn op_mul(context: &mut JSContext, stack: *mut JSValue) {
    // todo
    context.status = EvalStatus::BadOp;
}

#[allow(unused)]
unsafe fn op_div(context: &mut JSContext, stack: *mut JSValue) {
    // todo
    context.status = EvalStatus::BadOp;
}

unsafe fn op_add(context: &mut JSContext, stack: *mut JSValue) {
    unsafe {
        let lhs_num = stack.add(context.sp as usize - 1).as_ref().expect("Expected valid reference to LHS value on stack at vm.rs: op_add").get_number().unwrap_or(f64::NAN);
        let rhs_num = stack.add(context.sp as usize).as_ref().expect("Expected valid reference to LHS value on stack at vm.rs: op_add").get_number().unwrap_or(f64::NAN);

        context.sp -= 1;
        *stack.add(context.sp as usize) = JSValue::Number(lhs_num + rhs_num);
        context.ip = context.ip.add(1);
    }
}

unsafe fn op_sub(context: &mut JSContext, stack: *mut JSValue) {
    unsafe {
        let lhs_num = stack.add(context.sp as usize - 1).as_ref().expect("Expected valid reference to LHS value on stack at vm.rs: op_add").get_number().unwrap_or(f64::NAN);
        let rhs_num = stack.add(context.sp as usize).as_ref().expect("Expected valid reference to RHS value on stack at vm.rs: op_add").get_number().unwrap_or(f64::NAN);

        context.sp -= 1;
        *stack.add(context.sp as usize) = JSValue::Number(lhs_num - rhs_num);
        context.ip = context.ip.add(1);
    }
}

#[allow(unused)]
unsafe fn op_bt_and(context: &mut JSContext, stack: *mut JSValue) {
    // todo
    context.status = EvalStatus::BadOp;
}

#[allow(unused)]
unsafe fn op_bt_or(context: &mut JSContext, stack: *mut JSValue) {
    // todo
    context.status = EvalStatus::BadOp;
}

unsafe fn op_strict_eq(context: &mut JSContext, stack: *mut JSValue) {
    unsafe {
        let lhs_ref = stack.add(context.sp as usize - 1).as_ref().expect("Expected valid reference to LHS value on stack at vm.rs: op_strict_eq");
        let rhs_ref = stack.add(context.sp as usize).as_ref().expect("Expected valid reference to RHS value on stack at vm.rs: op_strict_eq");

        let flag = if lhs_ref.tag() != rhs_ref.tag() {
            false
        } else {
            match lhs_ref {
                JSValue::Undefined | JSValue::Null => true,
                JSValue::Boolean(lhs_bool) => *lhs_bool == rhs_ref.get_boolean(),
                JSValue::Number(f_value) => *f_value == rhs_ref.get_number().unwrap_or(f64::NAN),
                JSValue::StringId(lhs_sid) => if *lhs_sid == rhs_ref.get_str_id().unwrap_or(DUD_POOL_ID) {
                    true
                } else {
                    context.spool.get_item(*lhs_sid).unwrap().as_ptr().as_ref().expect("Expected valid interned string of LHS at vm.rs: op_strict_eq") == context.spool.get_item(rhs_ref.get_str_id().unwrap()).unwrap().as_ptr().as_ref().expect("Expected valid interned string of RHS at vm.rs: op_strict_eq")
                },
                JSValue::ObjectId(lhs_oid) => *lhs_oid == rhs_ref.get_obj_id().unwrap_or(DUD_POOL_ID)
            }
        };

        context.sp -= 1;
        *stack.add(context.sp as usize) = JSValue::Boolean(flag);
        context.ip = context.ip.add(1);
    }
}

unsafe fn op_strict_ne(context: &mut JSContext, stack: *mut JSValue) {
    unsafe {
        let lhs_ref = stack.add(context.sp as usize - 1).as_ref().expect("Expected valid reference to LHS value on stack at vm.rs: op_strict_ne");
        let rhs_ref = stack.add(context.sp as usize).as_ref().expect("Expected valid reference to RHS value on stack at vm.rs: op_strict_ne");

        let flag = if lhs_ref.tag() != rhs_ref.tag() {
            false
        } else {
            match lhs_ref {
                JSValue::Undefined | JSValue::Null => false,
                JSValue::Boolean(lhs_bool) => *lhs_bool != rhs_ref.get_boolean(),
                JSValue::Number(f_value) => *f_value != rhs_ref.get_number().unwrap_or(f64::NAN),
                JSValue::StringId(lhs_sid) => if *lhs_sid != rhs_ref.get_str_id().unwrap_or(DUD_POOL_ID) {
                    true
                } else {
                    context.spool.get_item(*lhs_sid).unwrap().as_ptr().as_ref().expect("Expected valid interned string of LHS at vm.rs: op_strict_eq") != context.spool.get_item(rhs_ref.get_str_id().unwrap()).unwrap().as_ptr().as_ref().expect("Expected valid interned string of RHS at vm.rs: op_strict_eq")
                },
                JSValue::ObjectId(lhs_oid) => *lhs_oid != rhs_ref.get_obj_id().unwrap_or(DUD_POOL_ID)
            }
        };

        context.sp -= 1;
        *stack.add(context.sp as usize) = JSValue::Boolean(flag);
        context.ip = context.ip.add(1);
    }
}

#[allow(unused)]
unsafe fn op_loose_eq(context: &mut JSContext, stack: *mut JSValue) {
    // todo
    context.status = EvalStatus::BadOp;
}

#[allow(unused)]
unsafe fn op_loose_ne(context: &mut JSContext, stack: *mut JSValue) {
    // todo
    context.status = EvalStatus::BadOp;
}

#[allow(unused)]
unsafe fn op_lt(context: &mut JSContext, stack: *mut JSValue) {
    // todo
    context.status = EvalStatus::BadOp;
}

#[allow(unused)]
unsafe fn op_lte(context: &mut JSContext, stack: *mut JSValue) {
    // todo
    context.status = EvalStatus::BadOp;
}

#[allow(unused)]
unsafe fn op_gt(context: &mut JSContext, stack: *mut JSValue) {
    // todo
    context.status = EvalStatus::BadOp;
}

#[allow(unused)]
unsafe fn op_gte(context: &mut JSContext, stack: *mut JSValue) {
    // todo
    context.status = EvalStatus::BadOp;
}

unsafe fn op_jump_if(context: &mut JSContext, stack: *mut JSValue) {
    unsafe {
        let arg_truthiness = stack.add(context.sp as usize).as_ref().expect("Expected JSValue in stack at vm.rs: op_jump_if").get_boolean();
        let jump_offset = context.ip.read().arg;
        
        context.ip = if arg_truthiness {
            context.ip.offset(jump_offset as isize)
        } else {
            context.sp -= 1;
            context.ip.add(1)
        };
    }
}

unsafe fn op_jump_else(context: &mut JSContext, stack: *mut JSValue) {
    unsafe {
        let arg_falsiness = !stack.add(context.sp as usize).as_ref().expect("Expected JSValue in stack at vm.rs: op_jump_if").get_boolean();
        let jump_offset = context.ip.read().arg;
        
        context.ip = if arg_falsiness {
            context.ip.offset(jump_offset as isize)
        } else {
            context.sp -= 1;
            context.ip.add(1)
        };
    }
}

unsafe fn op_jump(context: &mut JSContext, _: *mut JSValue) {
    unsafe {
        let jump_offset = context.ip.as_ref().expect("Expected valid instruction at IP of vm.rs: op_jump").arg;

        context.ip = context.ip.offset(jump_offset as isize);
    }
}

#[allow(unused)]
unsafe fn op_call(context: &mut JSContext, stack: *mut JSValue) {
    // todo: check context.cd < context.cm
    context.status = EvalStatus::BadOp;
}

#[allow(unused)]
unsafe fn op_call_ctor(context: &mut JSContext, stack: *mut JSValue) {
    // todo: check context.cd < context.cm
    context.status = EvalStatus::BadOp;
}

unsafe fn op_ret(context: &mut JSContext, stack: *mut JSValue) {
    unsafe {
        let CallFrame {caller_rip, caller_cvp, caller_bp, callee_bp, ..} = context.frames.last().expect("Expected present call frame at vm.rs: op_ret");
        let result_v = stack.add(context.sp as usize).read();
        
        context.sp = *callee_bp;
        *stack.add(context.sp as usize) = result_v;
        context.bp = *caller_bp;
        context.ip = *caller_rip;
        context.cvp = *caller_cvp;

        context.frames.pop();
        context.cd -= 1;

        if context.cd < 1 && context.status == EvalStatus::Pending{
            context.status = EvalStatus::Ok
        }
    }
}

pub fn run_vm(context: &mut JSContext) -> EvalStatus {
    unsafe {
        let stack_base_ptr = context.stack.as_mut_ptr();
        let context_stack_p = context.stack.as_mut_ptr();

        while context.status == EvalStatus::Pending {
            match context.ip.read().op {
                Opcode::PushUndef => op_push_undef(context, context_stack_p),
                Opcode::PushNull => op_push_null(context, stack_base_ptr),
                Opcode::PushBool => op_push_bool(context, stack_base_ptr),
                Opcode::PushNaN => op_push_nan(context, stack_base_ptr),
                Opcode::PushInf => op_push_inf(context, stack_base_ptr),
                Opcode::PushNegInf => op_push_neg_inf(context, stack_base_ptr),
                Opcode::PushThisRef => op_push_this_ref(context, stack_base_ptr),
                Opcode::PushStr => op_push_str(context, stack_base_ptr),
                Opcode::PushConst => op_push_const(context, stack_base_ptr),
                Opcode::Dup1 => op_dup1(context, stack_base_ptr),
                Opcode::Dup2 => op_dup2(context, stack_base_ptr),
                Opcode::Swap => op_swap(context, stack_base_ptr),
                Opcode::PopN => op_pop_n(context, stack_base_ptr),
                Opcode::Discard => op_discard(context, stack_base_ptr),
                Opcode::GetLocal => op_get_local(context, stack_base_ptr),
                Opcode::SetLocal => op_set_local(context, stack_base_ptr),
                Opcode::GetVar => op_get_var(context, stack_base_ptr),
                Opcode::SetVar => op_set_var(context, stack_base_ptr),
                Opcode::MakeObj => op_make_obj(context, stack_base_ptr),
                Opcode::GetOwnProp => op_get_own_prop(context, stack_base_ptr),
                Opcode::SetOwnProp => op_set_own_prop(context, stack_base_ptr),
                Opcode::GetProp => op_get_prop(context, stack_base_ptr),
                Opcode::SetProp => op_set_prop(context, stack_base_ptr),
                Opcode::DelProp => op_del_prop(context, stack_base_ptr),
                Opcode::GetProto => op_get_proto(context, stack_base_ptr),
                Opcode::SetProto => op_set_proto(context, stack_base_ptr),
                Opcode::ForceBool => op_force_bool(context, stack_base_ptr),
                Opcode::ForceNum => op_force_num(context, stack_base_ptr),
                Opcode::NegBool => op_neg_bool(context, stack_base_ptr),
                Opcode::NegNum => op_neg_num(context, stack_base_ptr),
                Opcode::Mod => op_mod(context, stack_base_ptr),
                Opcode::Mul => op_mul(context, stack_base_ptr),
                Opcode::Div => op_div(context, stack_base_ptr),
                Opcode::Add => op_add(context, stack_base_ptr),
                Opcode::Sub => op_sub(context, stack_base_ptr),
                Opcode::BtAnd => op_bt_and(context, stack_base_ptr),
                Opcode::BtOr => op_bt_or(context, stack_base_ptr),
                Opcode::StrictEq => op_strict_eq(context, stack_base_ptr),
                Opcode::StrictNe => op_strict_ne(context, stack_base_ptr),
                Opcode::LooseEq => op_loose_eq(context, stack_base_ptr),
                Opcode::LooseNe => op_loose_ne(context, stack_base_ptr),
                Opcode::Lt => op_lt(context, stack_base_ptr),
                Opcode::Lte => op_lte(context, stack_base_ptr),
                Opcode::Gt => op_gt(context, stack_base_ptr),
                Opcode::Gte => op_gte(context, stack_base_ptr),
                Opcode::JumpIf => op_jump_if(context, stack_base_ptr),
                Opcode::JumpElse => op_jump_else(context, stack_base_ptr),
                Opcode::Jump => op_jump(context, stack_base_ptr),
                Opcode::Call => op_call(context, stack_base_ptr),
                Opcode::CallCtor => op_call_ctor(context, stack_base_ptr),
                Opcode::Ret => op_ret(context, stack_base_ptr),
            };
        }
    }

    context.status
}