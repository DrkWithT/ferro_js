use crate::runtime::values::{JSVTag, JSValue};
use crate::runtime::objects::{DUD_POOL_ID};
use crate::runtime::property::AddPropHint;
use crate::runtime::code::{Opcode};
use crate::runtime::ctx::{ CallFrame, EvalStatus, JSContext};


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
    context.sp += 1;

    unsafe {
        *stack.add(context.sp as usize) = stack.add(context.bp as usize - 1).read();
        context.ip = context.ip.add(1);
    }
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
        let Some(env_obj_id) = context.frames.last().expect("Expected JS lexical environment in vm.rs: op_get_var").env_v.get_obj_id() else { context.status = EvalStatus::BadAccess; return; };
        let key_id = stack.add(context.sp as usize).as_ref_unchecked().get_str_id().unwrap_or(0) as usize;
        let ic_id = context.ip.read().flags & 0x7fff; // ! 16th bit saved as special flag IF an IC is present

        // println!("op_get_var:\nenv_obj_id = {env_obj_id}, key_id = {key_id}, ic_id = {ic_id}");

        if let Some(result_v) = context.get_property_data_value(env_obj_id, key_id, ic_id, false) {
            *stack.add(context.sp as usize) = result_v;
            context.ip = context.ip.add(1);
        } else {
            eprintln!("See vm.rs ~ op_get_var");
            context.status = EvalStatus::BadAccess;
        }
    }
}

unsafe fn op_set_var(context: &mut JSContext, stack: *mut JSValue) {
    unsafe {
        let Some(env_obj_id) = context.frames.last().expect("Expected JS lexical environment in vm.rs: op_set_var").env_v.get_obj_id() else { context.status = EvalStatus::BadAccess; return; };
        let key_id = stack.add(context.sp as usize - 1).as_ref_unchecked().get_str_id().unwrap_or(0) as usize;
        let temp_value = stack.add(context.sp as usize).as_ref_unchecked();
        let ic_id = context.ip.read().flags & 0x7fff;

        if context.set_property_data_mut(env_obj_id, key_id, ic_id,  AddPropHint::Data, temp_value) {
            context.sp -= 2;
            context.ip = context.ip.add(1);
            // println!("DEBUG vm.rs ~ set_var: updated env of obj-id-{env_obj_id} with key-sid-{key_id}");
        } else {
            eprintln!("See vm.rs ~ op_set_var");
            context.status = EvalStatus::BadAccess;
        }
    }
}

unsafe fn op_make_obj(context: &mut JSContext, stack: *mut JSValue) {
    context.sp += 1;

    unsafe {
        *stack.add(context.sp as usize) = context.create_blank_obj();
        context.ip = context.ip.add(1);
    }
}

unsafe fn op_get_prop(context: &mut JSContext, stack: *mut JSValue) {
    unsafe {
        let Some(target_oid) = stack.add(context.sp as usize - 1).as_ref_unchecked().get_obj_id() else {
            context.status = EvalStatus::BadAccess;
            return;
        };

        let ic_id = context.ip.as_ref_unchecked().flags & 0x7fff;
        // TODO: refactor accesses to no longer need a KEY ID, instead taking a &JSValue...
        let Some(key_id) = stack.add(context.sp as usize).as_ref_unchecked().get_str_id() else {
            context.status = EvalStatus::BadOp;
            return;
        };

        if !context.check_property_is_accessor(target_oid, key_id as usize) {
            // Case 1: Data property requires stack transition of: obj key^ -> ans^
            if let Some(maybe_result) = context.get_property_data_value(target_oid, key_id as usize, ic_id, false) {
                *stack.add(context.sp as usize) = maybe_result;
                context.ip = context.ip.add(1);
            } else {
                context.status = EvalStatus::BadAccess;
            }
        } else {
            // Case 2: Data property requires stack transitions of: obj obj fn_key^ -> obj getter^ -> ans^ (after 1-arg CALL)
            let getter_oid = context.get_property_data_value(target_oid, key_id as usize, ic_id, true).unwrap_or(JSValue::Undefined);

            *stack.add(context.sp as usize) = getter_oid;
            context.status = context.try_invoke_obj(&getter_oid, 0);
        }
    }
}

unsafe fn op_set_prop(context: &mut JSContext, stack: *mut JSValue) {
    unsafe {
        let curr_sp = context.sp as usize;

        let Some(target_oid) = stack.add(curr_sp - 2).as_ref_unchecked().get_obj_id() else {
            context.status = EvalStatus::BadAccess;
            return;
        };

        let ic_id = context.ip.as_ref_unchecked().flags & 0x7fff;
        // TODO: refactor accesses to no longer need a KEY ID, instead taking a &JSValue...
        let Some(key_id) = stack.add(curr_sp - 1).as_ref_unchecked().get_str_id() else {
            context.status = EvalStatus::BadOp;
            return;
        };

        let src_value = stack.add(curr_sp).as_ref_unchecked();

        // ! This insertion hint is used from compile-time determined metadata in this opcode for initializing object properties.
        let insertion_hint = match context.ip.read().arg {
            1 => AddPropHint::Data, // initialize data prop
            2 => AddPropHint::Getter, // initialize getter
            3 => AddPropHint::Setter, // initialize setter
            _ => AddPropHint::Noop
        };

        if insertion_hint != AddPropHint::Noop {
            let data_set_ok = context.set_property_data_mut(target_oid, key_id as usize, ic_id, insertion_hint, src_value);

            context.sp -= 2;
            context.ip = context.ip.add(1);
            context.status = if data_set_ok {EvalStatus::Pending} else {EvalStatus::BadAccess};
            return;
        }

        if !context.check_property_is_accessor(target_oid, key_id as usize) {
            // Case 1: Data property requires stack transition of: obj key val^ -> obj^
            let data_set_ok = context.set_property_data_mut(target_oid, key_id as usize, ic_id, AddPropHint::Data, src_value);

            context.ip = context.ip.add(1);
            context.status = if data_set_ok {EvalStatus::Pending} else {EvalStatus::BadAccess};
        } else {
            // Case 2: Data property requires stack transitions of: obj key val^ -> obj setter val^ -> ans^ (after 1-arg CALL). Then the setter is invoked.
            let setter_oid = context.get_property_data_value(target_oid, key_id as usize, ic_id, false).unwrap_or(JSValue::Undefined);

            *stack.add(curr_sp - 1) = setter_oid;
            context.status = context.try_invoke_obj(&setter_oid, 1);
        }
    }
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

unsafe fn op_inc_local(context: &mut JSContext, stack: *mut JSValue) {
    unsafe {
        let old_ref = stack.add(context.bp as usize + context.ip.as_ref_unchecked().arg as usize).as_mut_unchecked();
        let result_v = JSValue::number(context.jsvalue_to_number(old_ref) + 1.0);
        let ip_flags = context.ip.as_ref_unchecked().flags;

        context.sp += 1;

        if 0x8000 == ip_flags & 0x8000 { // prefix case
            *old_ref = result_v;
            *stack.add(context.sp as usize) = *old_ref;
        } else { // postfix case
            *stack.add(context.sp as usize) = *old_ref;
            *old_ref = result_v;
        }

        context.ip = context.ip.add(1);
    }
}

unsafe fn op_dec_local(context: &mut JSContext, stack: *mut JSValue) {
    unsafe {
        let old_ref = stack.add(context.bp as usize + context.ip.as_ref_unchecked().arg as usize).as_mut_unchecked();
        let result_v = JSValue::number(context.jsvalue_to_number(old_ref) - 1.0);
        let ip_flags = context.ip.as_ref_unchecked().flags;

        context.sp += 1;

        if 0x8000 == ip_flags & 0x8000 { // prefix case
            *old_ref = result_v;
            *stack.add(context.sp as usize) = *old_ref;
        } else { // postfix case
            *stack.add(context.sp as usize) = *old_ref;
            *old_ref = result_v;
        }

        context.ip = context.ip.add(1);
    }
}

#[allow(unused)]
unsafe fn op_inc_prop(context: &mut JSContext, stack: *mut JSValue) {
    unsafe {
        let Some(target_obj_id) = stack.add(context.sp as usize).as_ref_unchecked().get_obj_id() else { context.status = EvalStatus::BadAccess; return; };
        let prop_key_id = context.ip.as_ref_unchecked().arg;
        let ip_flags = context.ip.as_ref_unchecked().flags;

        let prop_old_v = context.get_property_data_value(target_obj_id, prop_key_id as usize, ip_flags & 0x7fff, false).unwrap_or(JSValue::Undefined);
        let prop_updated_num = JSValue::Number(context.jsvalue_to_number(&prop_old_v) + 1.0);

        if 0x8000 == ip_flags & 0x8000 { // special flag: prefix case
            context.set_property_data_mut(target_obj_id, prop_key_id as usize, ip_flags & 0x7fff, AddPropHint::Data, &prop_updated_num);
            *stack.add(context.sp as usize) = prop_updated_num;
        } else {
            *stack.add(context.sp as usize) = prop_old_v;
            context.set_property_data_mut(target_obj_id, prop_key_id as usize, ip_flags & 0x7fff, AddPropHint::Data, &prop_updated_num);
        }

        context.ip = context.ip.add(1);
    }
}

#[allow(unused)]
unsafe fn op_dec_prop(context: &mut JSContext, stack: *mut JSValue) {
    unsafe {
        let Some(target_obj_id) = stack.add(context.sp as usize).as_ref_unchecked().get_obj_id() else { context.status = EvalStatus::BadAccess; return; };
        let prop_key_id = context.ip.as_ref_unchecked().arg;
        let ip_flags = context.ip.as_ref_unchecked().flags;

        let prop_old_v = context.get_property_data_value(target_obj_id, prop_key_id as usize, ip_flags & 0x7fff, false).unwrap_or(JSValue::Undefined);
        let prop_updated_num = JSValue::Number(context.jsvalue_to_number(&prop_old_v) - 1.0);

        if 0x8000 == ip_flags & 0x8000 { // special flag: prefix case
            context.set_property_data_mut(target_obj_id, prop_key_id as usize, ip_flags & 0x7fff, AddPropHint::Data, &prop_updated_num);
            *stack.add(context.sp as usize) = prop_updated_num;
        } else {
            *stack.add(context.sp as usize) = prop_old_v;
            context.set_property_data_mut(target_obj_id, prop_key_id as usize, ip_flags & 0x7fff, AddPropHint::Data, &prop_updated_num);
        }

        context.ip = context.ip.add(1);
    }
}

/// **NOTE:** Converts a function reference on the stack into a closure that uses the current env or a child env.
unsafe fn op_make_closure(context: &mut JSContext, stack: *mut JSValue) {
    unsafe {
        let callable_oid = stack.add(context.sp as usize).as_ref_unchecked().get_obj_id().unwrap_or(DUD_POOL_ID);

        // println!("DEBUG vm.rs, op_make_closure:\ncallable_oid = {callable_oid}");

        *stack.add(context.sp as usize) = context.create_closure_obj(callable_oid);
        context.ip = context.ip.add(1);
    }
}

unsafe fn op_force_bool(context: &mut JSContext, stack: *mut JSValue) {
    unsafe {
        let truthiness = match stack.add(context.sp as usize).as_ref_unchecked() {
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
        let numeric_v = match stack.add(context.sp as usize).as_ref_unchecked() {
            JSValue::Undefined | JSValue::ObjectId(_) => f64::NAN,
            JSValue::Null => 0.0,
            JSValue::Boolean(bool_value) => if *bool_value {1.0} else {0.0},
            JSValue::Number(f64_value) => *f64_value,
            JSValue::StringId(sid) => {
                // By the JS specification: +"..." will try a parsing conversion to a number.
                if let Some(sv) = context.spool.get_item(*sid) {
                    str::parse::<f64>(sv).unwrap_or(f64::NAN)
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
        let temp_v = !context.jsvalue_to_boolean(stack.add(context.sp as usize).as_ref_unchecked());

        *stack.add(context.sp as usize) = JSValue::Boolean(temp_v);
        context.ip = context.ip.add(1);
    }
}

unsafe fn op_neg_num(context: &mut JSContext, stack: *mut JSValue) {
    unsafe {
        let src_num = -context.jsvalue_to_number(stack.add(context.sp as usize).as_ref_unchecked());

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
    context.sp -= 1;

    unsafe {
        let lhs_ref = stack.add(context.sp as usize).as_mut_unchecked();
        let lhs_v = context.jsvalue_to_number(lhs_ref);
        let rhs_v = context.jsvalue_to_number(stack.add(context.sp as usize + 1).as_ref_unchecked());

        *lhs_ref = JSValue::Number(lhs_v * rhs_v);
    }
    context.status = EvalStatus::BadOp;
}

#[allow(unused)]
unsafe fn op_div(context: &mut JSContext, stack: *mut JSValue) {
    context.sp -= 1;

    unsafe {
        let lhs_ref = stack.add(context.sp as usize).as_mut_unchecked();
        let lhs_v = context.jsvalue_to_number(lhs_ref);
        let rhs_v = context.jsvalue_to_number(stack.add(context.sp as usize + 1).as_ref_unchecked());

        *lhs_ref = JSValue::Number(if (lhs_v == 0.0 && rhs_v == 0.0) || (lhs_v.is_nan() || rhs_v.is_nan()) || (lhs_v.is_infinite() && rhs_v.is_infinite()) {
            f64::NAN
        } else if rhs_v.is_infinite() {
            (if lhs_v.is_sign_negative() { -1.0 } else { 1.0 }) * 0.0
        } else {
            lhs_v / rhs_v
        });

        context.ip = context.ip.add(1);
    }
}

unsafe fn op_add(context: &mut JSContext, stack: *mut JSValue) {
    unsafe {
        let lhs_num = stack.add(context.sp as usize - 1).as_ref_unchecked().get_number().unwrap_or(f64::NAN);
        let rhs_num = stack.add(context.sp as usize).as_ref_unchecked().get_number().unwrap_or(f64::NAN);

        context.sp -= 1;
        *stack.add(context.sp as usize) = JSValue::Number(lhs_num + rhs_num);
        context.ip = context.ip.add(1);
    }
}

unsafe fn op_sub(context: &mut JSContext, stack: *mut JSValue) {
    unsafe {
        let lhs_num = stack.add(context.sp as usize - 1).as_ref_unchecked().get_number().unwrap_or(f64::NAN);
        let rhs_num = stack.add(context.sp as usize).as_ref_unchecked().get_number().unwrap_or(f64::NAN);

        context.sp -= 1;
        *stack.add(context.sp as usize) = JSValue::Number(lhs_num - rhs_num);

        context.ip = context.ip.add(1);
    }
}

unsafe fn op_bt_flip(context: &mut JSContext, stack: *mut JSValue) {
    unsafe {
        let arg_p = stack.add(context.sp as usize);
        let arg_i32 = context.jsvalue_to_i32(arg_p.as_ref_unchecked());

        *arg_p = JSValue::Number((!arg_i32) as f64);

        context.ip = context.ip.add(1);
    }
}

unsafe fn op_bt_ls(context: &mut JSContext, stack: *mut JSValue) {
    context.sp -= 1;

    unsafe {
        let lhs_p = stack.add(context.sp as usize);
        let rhs_sh = context.jsvalue_to_u32(
            stack.add(context.sp as usize + 1).as_ref_unchecked()
        ) & 0x1F;

        *lhs_p = JSValue::Number(
            (context.jsvalue_to_i32(lhs_p.as_ref_unchecked()) << rhs_sh) as f64
        );
        context.ip = context.ip.add(1);
    }
}

unsafe fn op_bt_rs(context: &mut JSContext, stack: *mut JSValue) {
    context.sp -= 1;

    unsafe {
        let lhs_p = stack.add(context.sp as usize);
        let rhs_sh = context.jsvalue_to_u32(
            stack.add(context.sp as usize + 1).as_ref_unchecked()
        ) & 0x1F;

        *lhs_p = JSValue::Number(
            (context.jsvalue_to_i32(lhs_p.as_ref_unchecked()) >> rhs_sh) as f64
        );
        context.ip = context.ip.add(1);
    }
}

unsafe fn op_bt_and(context: &mut JSContext, stack: *mut JSValue) {
    context.sp -= 1;

    unsafe {
        let lhs_p = stack.add(context.sp as usize);
        let lhs_i32 = context.jsvalue_to_i32(lhs_p.as_ref_unchecked());
        let rhs_i32 = context.jsvalue_to_i32(stack.add(context.sp as usize + 1).as_ref_unchecked());

        *lhs_p = JSValue::Number((lhs_i32 & rhs_i32) as f64);

        context.ip = context.ip.add(1);
    }
}

unsafe fn op_bt_xor(context: &mut JSContext, stack: *mut JSValue) {
    context.sp -= 1;

    unsafe {
        let lhs_p = stack.add(context.sp as usize);
        let lhs_i32 = context.jsvalue_to_i32(lhs_p.as_ref_unchecked());
        let rhs_i32 = context.jsvalue_to_i32(stack.add(context.sp as usize + 1).as_ref_unchecked());

        *lhs_p = JSValue::Number((lhs_i32 ^ rhs_i32) as f64);

        context.ip = context.ip.add(1);
    }
}

unsafe fn op_bt_or(context: &mut JSContext, stack: *mut JSValue) {
    context.sp -= 1;

    unsafe {
        let lhs_p = stack.add(context.sp as usize);
        let lhs_i32 = context.jsvalue_to_i32(lhs_p.as_ref_unchecked());
        let rhs_i32 = context.jsvalue_to_i32(stack.add(context.sp as usize + 1).as_ref_unchecked());

        *lhs_p = JSValue::Number((lhs_i32 | rhs_i32) as f64);

        context.ip = context.ip.add(1);
    }
}

unsafe fn op_strict_eq(context: &mut JSContext, stack: *mut JSValue) {
    context.sp -= 1;

    unsafe {
        let lhs_ref = stack.add(context.sp as usize).as_ref_unchecked();
        let rhs_ref = stack.add(context.sp as usize + 1).as_ref_unchecked();

        let flag = if lhs_ref.tag() != rhs_ref.tag() {
            false
        } else {
            context.jsvalue_test_same_types_eq(lhs_ref, rhs_ref)
        };

        *stack.add(context.sp as usize) = JSValue::Boolean(flag);
        context.ip = context.ip.add(1);
    }
}

unsafe fn op_strict_ne(context: &mut JSContext, stack: *mut JSValue) {
    context.sp -= 1;

    unsafe {
        let lhs_ref = stack.add(context.sp as usize).as_ref_unchecked();
        let rhs_ref = stack.add(context.sp as usize + 1).as_ref_unchecked();

        let flag = if lhs_ref.tag() != rhs_ref.tag() {
            true
        } else {
            !context.jsvalue_test_same_types_eq(lhs_ref, rhs_ref)
        };

        *stack.add(context.sp as usize) = JSValue::Boolean(flag);
        context.ip = context.ip.add(1);
    }
}

unsafe fn op_loose_eq(context: &mut JSContext, stack: *mut JSValue) {
    context.sp -= 1;

    unsafe {
        let lhs_ref = stack.add(context.sp as usize).as_ref_unchecked();
        let rhs_ref = stack.add(context.sp as usize + 1).as_ref_unchecked();

        let flag = if lhs_ref.tag() == rhs_ref.tag() {
            context.jsvalue_test_same_types_eq(lhs_ref, rhs_ref)
        } else if lhs_ref.is_undefined() || lhs_ref.is_null() || rhs_ref.is_undefined() || rhs_ref.is_null() {
            true
        } else if lhs_ref.tag() == JSVTag::Object && rhs_ref.tag() == JSVTag::Object {
            lhs_ref.get_obj_id().unwrap_or(DUD_POOL_ID) == rhs_ref.get_obj_id().unwrap_or(DUD_POOL_ID)
        } else if lhs_ref.tag() == JSVTag::Number || rhs_ref.tag() == JSVTag::Number {
            context.jsvalue_to_number(lhs_ref) == context.jsvalue_to_number(rhs_ref)
        } else {
            false
        };

        *stack.add(context.sp as usize) = JSValue::Boolean(flag);
        context.ip = context.ip.add(1);
    }
}

unsafe fn op_loose_ne(context: &mut JSContext, stack: *mut JSValue) {
    context.sp -= 1;

    unsafe {
        let lhs_ref = stack.add(context.sp as usize).as_ref_unchecked();
        let rhs_ref = stack.add(context.sp as usize + 1).as_ref_unchecked();

        let flag = if lhs_ref.tag() == rhs_ref.tag() {
            !context.jsvalue_test_same_types_eq(lhs_ref, rhs_ref)
        } else if lhs_ref.is_undefined() || lhs_ref.is_null() || rhs_ref.is_undefined() || rhs_ref.is_null() {
            false
        } else if lhs_ref.tag() == JSVTag::Object && rhs_ref.tag() == JSVTag::Object {
            lhs_ref.get_obj_id().unwrap_or(DUD_POOL_ID) != rhs_ref.get_obj_id().unwrap_or(DUD_POOL_ID)
        } else if lhs_ref.tag() == JSVTag::Number || rhs_ref.tag() == JSVTag::Number {
            context.jsvalue_to_number(lhs_ref) != context.jsvalue_to_number(rhs_ref)
        } else {
            true
        };

        *stack.add(context.sp as usize) = JSValue::Boolean(flag);
        context.ip = context.ip.add(1);
    }
}

unsafe fn op_lt(context: &mut JSContext, stack: *mut JSValue) {
    context.sp -= 1;

    unsafe {
        let lhs_v = stack.add(context.sp as usize).as_ref_unchecked();
        let rhs_v = stack.add(context.sp as usize + 1).as_ref_unchecked();

        if let Some(lhs_sid) = lhs_v.get_str_id() && let Some(rhs_sid) = rhs_v.get_str_id() {
            let lhs_str = context.spool.get_item(lhs_sid).expect("Expected LHS string at op_lt");
            let rhs_str = context.spool.get_item(rhs_sid).expect("Expected LHS string at op_lt");

            *stack.add(context.sp as usize) = JSValue::Boolean(lhs_str < rhs_str);
        } else {
            let lhs_num = context.jsvalue_to_number(lhs_v);
            let rhs_num = context.jsvalue_to_number(rhs_v);

            if lhs_num.is_nan() || rhs_num.is_nan() {
                *stack.add(context.sp as usize) = JSValue::Undefined;
            } else {
                *stack.add(context.sp as usize) = JSValue::Boolean(lhs_num < rhs_num);
            }
        }

        context.ip = context.ip.add(1);
    }
}

#[allow(unused)]
unsafe fn op_lte(context: &mut JSContext, stack: *mut JSValue) {
    // todo
    context.status = EvalStatus::BadOp;
}

unsafe fn op_gt(context: &mut JSContext, stack: *mut JSValue) {
    context.sp -= 1;

    unsafe {
        let lhs_v = stack.add(context.sp as usize).as_ref_unchecked();
        let rhs_v = stack.add(context.sp as usize + 1).as_ref_unchecked();

        if let Some(lhs_sid) = lhs_v.get_str_id() && let Some(rhs_sid) = rhs_v.get_str_id() {
            let lhs_str = context.spool.get_item(lhs_sid).expect("Expected LHS string at op_lt");
            let rhs_str = context.spool.get_item(rhs_sid).expect("Expected LHS string at op_lt");

            *stack.add(context.sp as usize) = JSValue::Boolean(lhs_str > rhs_str);
        } else {
            let lhs_num = context.jsvalue_to_number(lhs_v);
            let rhs_num = context.jsvalue_to_number(rhs_v);

            if lhs_num.is_nan() || rhs_num.is_nan() {
                *stack.add(context.sp as usize) = JSValue::Undefined;
            } else {
                *stack.add(context.sp as usize) = JSValue::Boolean(lhs_num > rhs_num);
            }
        }

        context.ip = context.ip.add(1);
    }
}

#[allow(unused)]
unsafe fn op_gte(context: &mut JSContext, stack: *mut JSValue) {
    // todo
    context.status = EvalStatus::BadOp;
}

unsafe fn op_jump_if(context: &mut JSContext, stack: *mut JSValue) {
    unsafe {
        let arg_truthiness = context.jsvalue_to_boolean(stack.add(context.sp as usize).as_ref_unchecked());
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
        let arg_falsiness = !context.jsvalue_to_boolean(stack.add(context.sp as usize).as_ref_unchecked());
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
        let jump_offset = context.ip.as_ref_unchecked().arg;

        context.ip = context.ip.offset(jump_offset as isize);
    }
}

unsafe fn op_call(context: &mut JSContext, stack: *mut JSValue) {
    if context.cd > context.cm {
        eprintln!("Exceeded max call depth of \x1b[1;31m{}\x1b[0m!", context.cm);
        context.status = EvalStatus::BadOp;
        return;
    }

    unsafe {
        let callee_argc = context.ip.as_ref_unchecked().arg;
        let callee_slot = context.sp - callee_argc;

        let callee_v = stack.add(callee_slot as usize).as_ref_unchecked();
        context.status = context.try_invoke_obj(callee_v, callee_argc as u16);
    }
}

#[allow(unused)]
unsafe fn op_call_ctor(context: &mut JSContext, stack: *mut JSValue) {
    // todo: check context.cd < context.cm
    context.status = EvalStatus::BadOp;
}

#[allow(unused)]
unsafe fn op_native_call(context: &mut JSContext, stack: *mut JSValue) {
    context.status = EvalStatus::BadOp;
}

unsafe fn op_ret(context: &mut JSContext, stack: *mut JSValue) {
    unsafe {
        let CallFrame {caller_rip, caller_icp, caller_cvp, caller_bp, callee_bp, ..} = context.frames.last().expect("Expected present call frame at vm.rs: op_ret");
        let result_v = stack.add(context.sp as usize).read();

        context.sp = *callee_bp - 1;
        *stack.add(context.sp as usize) = result_v;
        context.bp = *caller_bp;
        context.ip = *caller_rip;
        context.cvp = *caller_cvp;
        context.icp = *caller_icp;

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
                Opcode::GetProp => op_get_prop(context, stack_base_ptr),
                Opcode::SetProp => op_set_prop(context, stack_base_ptr),
                Opcode::DelProp => op_del_prop(context, stack_base_ptr),
                Opcode::GetProto => op_get_proto(context, stack_base_ptr),
                Opcode::SetProto => op_set_proto(context, stack_base_ptr),
                Opcode::IncLocal => op_inc_local(context, stack_base_ptr),
                Opcode::DecLocal => op_dec_local(context, stack_base_ptr),
                Opcode::IncProp => op_inc_prop(context, stack_base_ptr),
                Opcode::DecProp => op_dec_prop(context, stack_base_ptr),
                Opcode::MakeClosure => op_make_closure(context, stack_base_ptr),
                Opcode::ForceBool => op_force_bool(context, stack_base_ptr),
                Opcode::ForceNum => op_force_num(context, stack_base_ptr),
                Opcode::NegBool => op_neg_bool(context, stack_base_ptr),
                Opcode::NegNum => op_neg_num(context, stack_base_ptr),
                Opcode::Mod => op_mod(context, stack_base_ptr),
                Opcode::Mul => op_mul(context, stack_base_ptr),
                Opcode::Div => op_div(context, stack_base_ptr),
                Opcode::Add => op_add(context, stack_base_ptr),
                Opcode::Sub => op_sub(context, stack_base_ptr),
                Opcode::BtFlip => op_bt_flip(context, stack_base_ptr),
                Opcode::BtLs => op_bt_ls(context, stack_base_ptr),
                Opcode::BtRs => op_bt_rs(context, stack_base_ptr),
                Opcode::BtAnd => op_bt_and(context, stack_base_ptr),
                Opcode::BtOr => op_bt_or(context, stack_base_ptr),
                Opcode::BtXor => op_bt_xor(context, stack_base_ptr),
                Opcode::StrictEq => op_strict_eq(context, stack_base_ptr),
                Opcode::StrictNe => op_strict_ne(context, stack_base_ptr),
                Opcode::LooseEq => op_loose_eq(context, stack_base_ptr),
                Opcode::LooseNe => op_loose_ne(context, stack_base_ptr),
                Opcode::Lt => op_lt(context, stack_base_ptr), // todo
                Opcode::Lte => op_lte(context, stack_base_ptr), // todo
                Opcode::Gt => op_gt(context, stack_base_ptr),
                Opcode::Gte => op_gte(context, stack_base_ptr),
                Opcode::JumpIf => op_jump_if(context, stack_base_ptr),
                Opcode::JumpElse => op_jump_else(context, stack_base_ptr),
                Opcode::Jump => op_jump(context, stack_base_ptr),
                Opcode::Call => op_call(context, stack_base_ptr),
                Opcode::CallCtor => op_call_ctor(context, stack_base_ptr),
                Opcode::NativeCall => op_native_call(context, stack_base_ptr),
                Opcode::Ret => op_ret(context, stack_base_ptr),
            };
        }
    }

    context.status
}