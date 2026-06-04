use std::collections::{HashMap};
use std::rc::Rc;
use std::cell::{Cell, RefCell};

use crate::runtime::values::{JSValue};
use crate::runtime::funcs::JSFunction;
// todo: make string pool, object heap, metatable-like objects, and Function. Create an object Shape first.

pub const DUD_SHAPE_ID: i32 = -1;

#[derive(Debug)]
pub struct Shape {
    /// Maps pre-calculated string hashes to property indices.
    pub entries: HashMap<usize, usize>,
    /// Maps pre-calculated string hashes of new properties to child Shapes.
    pub links: Vec<(usize, usize)>,
    pub parent: i32,
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
    pub fn resolve_offset(&self, key_hash: usize) -> Option<usize> {
        self.entries.get(&key_hash).copied()
    }

    pub fn resolve_subshape_id(&self, key_hash: usize) -> Option<usize> {
        for (link_key, child_id) in &self.links {
            if *link_key == key_hash {
                return Some(*child_id);
            }
        }

        None
    }

    pub fn add_transition(&mut self, key_hash: usize, child_shape_id: usize) {
        if self.entries.contains_key(&key_hash) {
            return;
        }

        let entry_offset_count = self.entries.len();

        self.entries.insert(key_hash, entry_offset_count);
        self.links.push((key_hash, child_shape_id));
    }
}

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PropFlag {
    Writable,
    Enumerable,
    Configurable,
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

pub const JS_OBJECT_COST: usize = 56;
pub const JS_STRING_COST: usize = 24;

#[derive(Debug, Clone)]
pub struct ExoticObject {
    pub props: Vec<Property>,
    pub items: Vec<JSValue>,
    pub shape: i32,
}

#[derive(Debug, Clone)]
pub enum JSObjectWrap {
    /// Stores a typical, key-value object.
    Exotic(ExoticObject),
    Func(JSFunction),
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
