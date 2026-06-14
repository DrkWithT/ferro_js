use std::collections::HashMap;

pub const DUD_SHAPE_ID: i32 = 0;
pub const DEFAULT_SHAPE_POPULATION: usize = 4096;

#[derive(Debug)]
pub struct Shape {
    /// Maps pre-calculated string hashes to property indices.
    pub entries: HashMap<usize, usize>,
    /// Maps pre-calculated string hashes of new properties to child Shapes.
    pub links: HashMap<usize, i32>,
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
            links: HashMap::default(),
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
        self.links.get(&key_id).cloned()
    }

    /// Returns an existing shape for a new transition (additional property name to shape ID pair) ONLY IF the links has it.
    pub fn add_transition(&mut self, key_hash: usize, child_shape_id: i32) -> i32 {
        if let Some(child_shape_id) = self.links.get(&key_hash) {
            return *child_shape_id;
        }

        self.links.insert(key_hash, child_shape_id);

        child_shape_id
    }

    pub fn derive_child(&mut self, added_key: usize, child_shape_id: i32) -> Self {
        self.links.insert(added_key, child_shape_id);

        Self {
            entries: {
                let mut old_entries = self.entries.clone();

                old_entries.insert(added_key, old_entries.len());

                old_entries
            },
            links: HashMap::default(),
            parent: self.id,
            id: child_shape_id
        }
    }
}
