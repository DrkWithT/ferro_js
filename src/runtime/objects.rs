use std::collections::{HashMap};
use std::rc::Rc;
use std::cell::{Cell, RefCell};

use crate::runtime::values::JSValue;
use crate::runtime::funcs::JSFunction;
use crate::runtime::ctx::{JSContext};

pub const DUD_SHAPE_ID: i32 = 0;
pub const DEFAULT_SHAPE_POPULATION: usize = 4096;

#[derive(Debug)]
pub struct Shape {
    /// Maps pre-calculated string hashes to property indices.
    pub entries: HashMap<usize, usize>,
    /// Maps pre-calculated string hashes of new properties to child Shapes.
    pub links: Vec<(usize, i32)>,
    pub parent: i32,
    /// NOTE: **MUST** be updated after this `Shape` is cloned from its parent.
    pub id: i32,
}

impl Default for Shape {
    /// Creates the empty Shape (layout structure) of freshly created & blank objects.
    /// Example:
    /// ```js
    /// var x = {}; // x.[[shape]] = Shape::default()
    /// ```
    fn default() -> Self {
        Self {
            entries: HashMap::default(),
            links: vec![],
            parent: DUD_SHAPE_ID,
            id: 0
        }
    }
}

impl Shape {
    pub fn resolve_offset(&self, key_id: usize) -> Option<usize> {
        self.entries.get(&key_id).copied()
    }

    pub fn resolve_subshape_id(&self, key_id: usize) -> Option<i32> {
        for (link_key, child_id) in &self.links {
            if *link_key == key_id {
                return Some(*child_id);
            }
        }

        None
    }

    /// Returns an existing shape for a new transition (additional property name to shape ID pair) ONLY IF the links has it.
    pub fn add_transition(&mut self, key_hash: usize, child_shape_id: i32) -> i32 {
        if let Some((_, pre_link_child_shape_id)) = self.links.iter().find(|item| {
            item.0 == key_hash
        }) {
            return *pre_link_child_shape_id;
        }

        self.links.push((key_hash, child_shape_id));

        child_shape_id
    }

    pub fn derive_child(&mut self, added_key: usize, child_shape_id: i32) -> Self {
        self.links.push((added_key, child_shape_id));

        Self {
            entries: {
                let mut old_entries = self.entries.clone();

                old_entries.insert(added_key, old_entries.len());

                old_entries
            },
            links: vec![],
            parent: self.id,
            id: child_shape_id
        }
    }
}

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PropFlag {
    Writable = (1 << 0),
    Enumerable = (1 << 1),
    Configurable = (1 << 2),
    IsAccessor = (1 << 4),
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AddPropHint {
    Noop,
    Data,
    Getter,
    Setter,
}

#[derive(Debug, Clone)]
pub enum PropBody {
    Data(JSValue),
    Accessor((JSValue, JSValue)),
}

#[derive(Debug, Clone)]
pub struct Property {
    pub body: PropBody,
    pub flags: u8,
}

impl Property {
    pub fn data(v: &JSValue, flags: u8) -> Self {
        Self {
            body: PropBody::Data(*v),
            flags
        }
    }

    pub fn accessor(getter: &JSValue, setter: &JSValue, flags: u8) -> Self {
        Self {
            body: PropBody::Accessor((*getter, *setter)),
            flags
        }
    }

    pub fn is_writable(&self) -> bool {
        0 != self.flags & PropFlag::Writable as u8
    }

    pub fn is_configurable(&self) -> bool {
        0 != self.flags & PropFlag::Configurable as u8
    }

    pub fn is_enumerable(&self) -> bool {
        0 != self.flags & PropFlag::Enumerable as u8
    }
}

pub const JS_OBJECT_COST: usize = 56;
pub const JS_STRING_COST: usize = 24;

#[derive(Debug, Clone)]
pub struct ExoticObject {
    pub props: Vec<Property>,
    pub items: Vec<JSValue>,
    pub in_proto: JSValue,
    pub out_proto: JSValue,
    pub shape: i32,
}

impl Default for ExoticObject {
    fn default() -> Self {
        Self {
            props: vec![],
            items: vec![],
            in_proto: JSValue::Undefined,
            out_proto: JSValue::Undefined,
            shape: DUD_SHAPE_ID
        }
    }
}

#[derive(Debug, Clone)]
pub enum JSObjectWrap {
    /// Stores a typical, key-value object.
    Exotic(ExoticObject),
    Func(JSFunction),
}

impl JSObjectWrap {
    pub fn as_object(&self) -> Option<&ExoticObject> {
        if let Self::Exotic(object_ref) = self {
            Some(object_ref)
        } else {
            let Self::Func(func_ref) = self else {
                return None;
            };

            Some(&func_ref.data)
        }
    }

    pub fn as_object_mut(&mut self) -> Option<&mut ExoticObject> {
        if let Self::Exotic(object_ref) = self  {
            Some(object_ref)
        } else {
            let Self::Func(func_ref) = self else {
                return None;
            };

            Some(&mut func_ref.data)
        }
    }

    pub fn as_func(&self) -> Option<&JSFunction> {
        if let Self::Func(func_ref) = self {
            return Some(func_ref);
        }

        None
    }

    pub fn as_func_mut(&mut self) -> Option<&mut JSFunction> {
        if let Self::Func(func_ref) = self {
            return Some(func_ref);
        }

        None
    }

    pub fn check_property_is_accessor(&self, ctx: &JSContext, key_id: usize) -> bool {
        let my_data = self.as_object().expect("Expected valid object ref at objects.rs ~ check_property_is_accessor");
        let my_shape_id = my_data.shape;

        let prop_offset = if let Some(slow_prop_ref) = ctx.shapes.fetch(my_shape_id) {
            slow_prop_ref.resolve_offset(key_id)
        } else { None };

        let parent_env_oid = my_data.in_proto.get_obj_id();

        if prop_offset.is_none() {
            if let Some(parent_obj) = ctx.heap.get_item(parent_env_oid.unwrap_or(DUD_POOL_ID)) {
                unsafe {
                    return parent_obj.as_ptr().as_ref_unchecked().check_property_is_accessor(ctx, key_id);
                }
            } else if parent_env_oid.is_none() {
                return false;
            }
        }

        match &my_data.props.get(prop_offset.unwrap()).unwrap().body {
            PropBody::Data(_) => false,
            PropBody::Accessor((_, _)) => true
        }
    }

    pub fn get_property_data_value(&self, ctx: &mut JSContext, key_id: usize, ic_id: u16, try_use_getter: bool) -> Option<JSValue> {
        let my_data = self.as_object()?;
        let my_shape_id = my_data.shape;

        // dbg!(my_shape_id);

        let (prop_offset, ic_dirty) = unsafe {
            if let Some (ic_slot) = ctx.icp.add(ic_id as usize).as_mut().unwrap().find(my_shape_id, key_id) {
                // println!("Debug: try IC...");
                // dbg!(my_shape_id, key_id);
                (Some(ic_slot), false)
            } else if let Some(slow_prop_ref) = ctx.shapes.fetch(my_shape_id) {
                // println!("Debug: try Shape...");
                // dbg!(my_shape_id, key_id);
                (slow_prop_ref.resolve_offset(key_id), true)
            } else { (None, true) }
        };

        let parent_env_oid = my_data.in_proto.get_obj_id();

        // println!("In objects.rs at JSObjectWrap::get_property_data_value:");
        // dbg!(my_data.in_proto);
        // dbg!(prop_offset);

        if prop_offset.is_none() {
            if let Some(parent_obj) = ctx.heap.get_item(parent_env_oid.unwrap_or(DUD_POOL_ID)) {
                unsafe { // ! TODO: fix unwrap panic here... is the env parent / prototype real?
                    return parent_obj.as_ptr().as_mut_unchecked().get_property_data_value(ctx, key_id, ic_id, try_use_getter);
                }
            } else if parent_env_oid.is_none() {
                return None;
            }
        }

        if ic_dirty {
            unsafe {
                ctx.icp.add(ic_id as usize).as_mut_unchecked().update(my_shape_id, key_id, prop_offset.unwrap());
            }
        }

        match &my_data.props.get(prop_offset.unwrap()).unwrap().body {
            PropBody::Data(prop_v) => Some(*prop_v),
            PropBody::Accessor((get_fn, set_fn)) => Some(if try_use_getter {*get_fn} else {*set_fn})
        }
    }

    pub fn get_property_data_mut(&mut self, ctx: &mut JSContext, key_id: usize, ic_id: u16) -> Option<&mut JSValue> {
        let my_data = self.as_object_mut()?;
        let my_shape_id = my_data.shape;

        let (prop_offset, ic_dirty) = unsafe {
            if let Some (ic_slot) = ctx.icp.add(ic_id as usize).as_mut().unwrap().find(my_shape_id, key_id) {
                (Some(ic_slot), false)
            } else if let Some(slow_prop_ref) = ctx.shapes.fetch(my_shape_id) {
                (slow_prop_ref.resolve_offset(key_id), true)
            } else { (None, true) }
        };

        let parent_env_oid = my_data.in_proto.get_obj_id();

        if prop_offset.is_none() {
            if let Some(parent_obj) = ctx.heap.get_item(parent_env_oid.unwrap_or(DUD_POOL_ID)) {
                unsafe {
                    return parent_obj.as_ptr().as_mut_unchecked().get_property_data_mut(ctx, key_id, ic_id);
                }
            } else if parent_env_oid.is_none() {
                return None;
            }
        }

        if ic_dirty {
            unsafe {
                ctx.icp.add(ic_id as usize).as_mut_unchecked().update(my_shape_id, key_id, prop_offset.unwrap());
            }
        }

        match &mut my_data.props.get_mut(prop_offset.unwrap()).unwrap().body {
            PropBody::Data(prop_v) => Some(prop_v),
            // TODO: "invoke" this back in the opcode to evaluate getter logic.
            PropBody::Accessor((getter, _)) => Some(getter)
        }
    }

    pub fn set_property_data_mut(&mut self, ctx: &mut JSContext, key_id: usize, ic_id: u16, hint: AddPropHint, arg: &JSValue) -> bool {
        let Some(my_data) = self.as_object_mut() else { return false; };
        let my_shape_id = my_data.shape;

        let (prop_offset, ic_dirty, shape_dirty) = unsafe {
            if let Some (ic_slot) = ctx.icp.add(ic_id as usize).as_mut().unwrap().find(my_shape_id, key_id) {
                (Some(ic_slot), false, false)
            } else if let Some(slow_prop_ref) = ctx.shapes.fetch(my_shape_id) {
                if let Some(present_prop_offset) = slow_prop_ref.resolve_offset(key_id) {
                    (Some(present_prop_offset), true, false)
                } else {
                    (Some(my_data.props.len()), true, true)
                }
            } else { (None, true, true) }
        };

        if prop_offset.is_none() {
            return false;
        }

        if shape_dirty {
            let maybe_old_shape_child_id = ctx.shapes.fetch_mut(my_shape_id).expect("Expected valid shape reference by ID in objects.rs ~ set_property_data_mut!").resolve_subshape_id(key_id);

            if let Some(existing_child_shape_id) = maybe_old_shape_child_id {
                my_data.shape = existing_child_shape_id;
            } else {
                let temp_child_shape = ctx.shapes.fetch_mut(my_shape_id).unwrap().derive_child(key_id, my_data.shape);
                let temp_child_shape_id = ctx.shapes.store(temp_child_shape).expect("Exhausted shape IDs to i32::MAX in objects.rs ~ set_property_data_mut!");
                my_data.shape = temp_child_shape_id;
                ctx.shapes.fetch_mut(my_shape_id).unwrap().add_transition(key_id, temp_child_shape_id);
            }

            my_data.props.push(Property {
                body: if hint == AddPropHint::Data {PropBody::Data(*arg)} else {PropBody::Accessor((JSValue::Undefined, JSValue::Undefined))},
                flags: PropFlag::Writable as u8 | PropFlag::Configurable as u8
            });
        }

        if ic_dirty {
            unsafe {
                ctx.icp.add(ic_id as usize).as_mut_unchecked().update(my_shape_id, key_id, prop_offset.unwrap());
            }
        }

        match &mut my_data.props.get_mut(prop_offset.unwrap()).unwrap().body {
            PropBody::Data(prop_v) => {
                *prop_v = *arg;
                true
            },
            PropBody::Accessor((getter, setter)) => {
                if hint == AddPropHint::Getter {
                    *getter = *arg;
                } else {
                    *setter = *arg;
                }

                true
            }
        }
    }
}


pub type JSObjPtr = Option<Rc<RefCell<JSObjectWrap>>>;
pub type JSStrPtr = Option<Rc<Cell<String>>>;


#[derive(Default)]
pub struct ItemPool<T, const COST: usize> {
    pub items: Vec<T>,
    pub holes: Vec<i32>,
    pub item_cost: usize,
    pub ripe_cost: usize,
    pub cost: usize,
    pub next_id: i32,
    pub tenure_count: i32,
}

impl ItemPool<JSObjPtr, JS_OBJECT_COST> {
    pub fn new(population: usize) -> Self {
        Self {
            holes: Vec::default(),
            items: {
                let mut temp_cells = Vec::with_capacity(population);

                temp_cells.resize(population, None);

                temp_cells
            },
            item_cost: JS_OBJECT_COST,
            ripe_cost: ((population * 2) / 3) * JS_OBJECT_COST,
            cost: 0,
            next_id: -1,
            tenure_count: 0
        }
    }

    pub fn get_used_id_count(&self) -> i32 {
        self.next_id + 1
    }

    pub fn is_ripe_for_gc(&self) -> bool {
        self.cost >= self.ripe_cost
    }

    fn next_id(&mut self) -> Option<i32> {
        let result = if !self.holes.is_empty() {
            self.holes.pop().expect("Expected freed ID in ItemPool<JSObjPtr>::next_id()")
        } else {
            self.next_id += 1;
            self.next_id
        };

        if result < i32::MAX { Some(result) } else { None }
    }

    pub fn mark_tenured(&mut self) {
        self.tenure_count = self.next_id + 1;
    }

    pub fn get_item(&self, heap_id: i32) -> JSObjPtr {
        if heap_id < 0 || heap_id as usize >= self.items.len() {
            return None;
        }

        self.items.get(heap_id as usize).unwrap().clone()
    }

    pub fn add_item(&mut self, value: JSObjPtr) -> Option<i32> {
        let next_slot_id = self.next_id();

        if let Some(next_id) = next_slot_id {
            self.items[next_id as usize] = value;
        }

        next_slot_id
    }

    pub fn remove_item(&mut self, target_id: i32) -> bool {
        if target_id <= self.tenure_count || target_id as usize >= self.items.len() {
            return false;
        }

        {
            //? Here, let RAII destruct the targeted object from its cell.
            let _ = self.items[target_id as usize].take();
            self.holes.push(target_id);
        }

        true
    }

    pub fn tenure_current(&mut self) {
        self.tenure_count = self.next_id;
    }
}

impl ItemPool<JSStrPtr, JS_STRING_COST> {
    pub fn new(population: usize) -> Self {
        Self {
            holes: Vec::default(),
            items: {
                let mut temp_cells = Vec::with_capacity(population);

                temp_cells.resize(population, None);

                temp_cells
            },
            item_cost: JS_OBJECT_COST,
            ripe_cost: ((population * 2) / 3) * JS_OBJECT_COST,
            cost: 0,
            next_id: -1,
            tenure_count: 0
        }
    }

    pub fn get_used_id_count(&self) -> i32 {
        self.next_id + 1
    }

    pub fn is_ripe_for_gc(&self) -> bool {
        self.cost >= self.ripe_cost
    }

    fn next_id(&mut self) -> Option<i32> {
        let result = if !self.holes.is_empty() {
            self.holes.pop().expect("Expected freed ID in ItemPool<JSObjPtr>::next_id()")
        } else {
            self.next_id += 1;
            self.next_id
        };

        if result < i32::MAX { Some(result) } else { None }
    }

    pub fn mark_tenured(&mut self) {
        self.tenure_count = self.next_id + 1;
    }

    pub fn get_item(&self, str_id: i32) -> JSStrPtr {
        if str_id < 0 || str_id as usize >= self.items.len() {
            return None;
        }

        self.items.get(str_id as usize).unwrap().clone()
    }

    pub fn add_item(&mut self, value: JSStrPtr) -> Option<i32> {
        let next_slot_id = self.next_id();

        if let Some(next_id) = next_slot_id {
            self.items[next_id as usize] = value;
        }

        next_slot_id
    }

    pub fn remove_item(&mut self, target_id: i32) -> bool {
        if target_id <= self.tenure_count || target_id as usize >= self.items.len() {
            return false;
        }

        {
            //? Here, let RAII destruct the targeted object from its cell.
            let _ = self.items[target_id as usize].take();
            self.holes.push(target_id);
        }

        true
    }

    pub fn tenure_current(&mut self) {
        self.tenure_count = self.next_id;
    }
}

pub const MAX_POOL_ID: i32 = i32::MAX >> 7;
pub const DUD_POOL_ID: i32 = -1;

#[derive(Debug, Default)]
pub struct ShapePool {
    pub shapes: Vec<Shape>,
    pub next_sid: i32,
}

impl ShapePool {
    pub fn fetch(&self, sid: i32) -> Option<&Shape> {
        self.shapes.get(sid as usize)
    }

    pub fn fetch_mut(&mut self, sid: i32) -> Option<&mut Shape> {
        self.shapes.get_mut(sid as usize)
    }

    pub fn store(&mut self, s: Shape) -> Option<i32> {
        let sid = self.next_sid;

        if sid >= MAX_POOL_ID {
            return None;
        }

        self.shapes[sid as usize] = s;

        self.next_sid = sid + 1;

        Some(sid)
    }
}
