use std::cell::{RefCell};

use crate::runtime::values::JSValue;
use crate::runtime::closure::JSClosure;
use crate::runtime::opaque::JSOpaque;
use crate::runtime::shape::{DUD_SHAPE_ID, Shape};
use crate::runtime::property::{Property};

pub const JS_OBJECT_COST: usize = 56;
pub const JS_STRING_COST: usize = 24;
pub const JS_CLOSURE_COST: usize = 24;

pub const MAX_POOL_ID: i32 = i32::MAX >> 7;
pub const DUD_POOL_ID: i32 = -1;

#[derive(Clone)]
pub struct ExoticObject {
    pub props: Vec<Property>,
    pub items: Vec<JSValue>,
    pub in_proto: JSValue,
    pub out_proto: JSValue,
    pub opaque: JSOpaque,
    pub shape: i32,
}

impl Default for ExoticObject {
    fn default() -> Self {
        Self {
            props: vec![],
            items: vec![],
            in_proto: JSValue::Undefined,
            out_proto: JSValue::Undefined,
            opaque: JSOpaque::default(),
            shape: DUD_SHAPE_ID
        }
    }
}

impl ExoticObject {
    pub fn with_opaque(opaque: JSOpaque) -> Self {
        Self {
            props: vec![],
            items: vec![],
            in_proto: JSValue::Undefined,
            out_proto: JSValue::Undefined,
            opaque,
            shape: DUD_SHAPE_ID
        }
    }
}


pub type JSObjPtr = Option<RefCell<ExoticObject>>;
pub type JSObjRef<'stored_obj_lt> = Option<&'stored_obj_lt ExoticObject>;
pub type JSObjMut<'stored_obj_lt> = Option<&'stored_obj_lt mut ExoticObject>;

pub type JSStrPtr = Option<Box<String>>;
pub type JSStrRef<'src_str_lt> = Option<&'src_str_lt str>;

pub type JSClosurePtr = Option<RefCell<JSClosure>>;
pub type JSClosureMut<'closure_lt> = Option<&'closure_lt mut JSClosure>;

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

// Manages a pool of GC-collectible JS objects.
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
            self.holes.pop().unwrap()
        } else {
            self.next_id += 1;
            self.next_id
        };

        if result < i32::MAX { Some(result) } else { None }
    }

    pub fn mark_tenured(&mut self) {
        self.tenure_count = self.next_id + 1;
    }

    pub fn get_item(&'_ self, heap_id: i32) -> JSObjRef<'_> {
        if heap_id < 0 || heap_id as usize >= self.items.len() {
            return None;
        }

        unsafe {
            Some(self.items[heap_id as usize].as_ref().unwrap().as_ptr().as_ref().unwrap())
        }
    }

    pub fn get_item_mut(&'_ mut self, heap_id: i32) -> JSObjMut<'_> {
        if heap_id < 0 || heap_id as usize >= self.items.len() {
            return None;
        }

        unsafe {
            Some(self.items[heap_id as usize].as_mut().unwrap().as_ptr().as_mut_unchecked())
        }
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

// Interns all static / runtime string constants.
impl ItemPool<JSStrPtr, JS_STRING_COST> {
    pub fn new(population: usize) -> Self {
        Self {
            holes: Vec::default(),
            items: {
                let mut temp_cells = Vec::with_capacity(population);

                temp_cells.resize(population, None);

                temp_cells
            },
            item_cost: JS_STRING_COST,
            ripe_cost: ((population * 2) / 3) * JS_STRING_COST,
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
            self.holes.pop().expect("Expected freed ID in ItemPool<JSStrPtr>::next_id()")
        } else {
            self.next_id += 1;
            self.next_id
        };

        if result < i32::MAX { Some(result) } else { None }
    }

    pub fn mark_tenured(&mut self) {
        self.tenure_count = self.next_id + 1;
    }

    pub fn get_item(&self, str_id: i32) -> JSStrRef<'_> {
        if str_id < 0 || str_id as usize >= self.items.len() {
            return None;
        }

        Some(self.items.get(str_id as usize).unwrap().as_deref().unwrap())
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

// Manages a pool of GC-collectible closures.
impl ItemPool<JSClosurePtr, JS_CLOSURE_COST> {
    pub fn new(population: usize) -> Self {
        Self {
            holes: Vec::default(),
            items: {
                let mut temp_cells = Vec::with_capacity(population);

                temp_cells.resize(population, None);

                temp_cells
            },
            item_cost: JS_CLOSURE_COST,
            ripe_cost: ((population * 2) / 3) * JS_CLOSURE_COST,
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
            self.holes.pop().expect("Expected freed ID in ItemPool<JSClosurePtr>::next_id()")
        } else {
            self.next_id += 1;
            self.next_id
        };

        if result < i32::MAX { Some(result) } else { None }
    }

    pub fn mark_tenured(&mut self) {
        self.tenure_count = self.next_id + 1;
    }

    pub fn get_item(&mut self, closure_id: i32) -> JSClosureMut<'_> {
        if closure_id < 0 || closure_id as usize >= self.items.len() {
            return None;
        }

        Some(self.items[closure_id as usize].as_mut().unwrap().get_mut())
    }

    pub fn add_item(&mut self, value: JSClosurePtr) -> Option<i32> {
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

#[derive(Debug, Default)]
pub struct ShapePool {
    pub shapes: Vec<Shape>,
    pub next_sid: i32,
}

// Interns all object Shapes.
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
