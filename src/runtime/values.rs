use std::fmt::Display;

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum JSVTag {
    Undefined,
    Null,
    Boolean,
    Number,
    String,
    Object,
}

#[derive(Debug, Clone, Copy)]
pub enum JSValue {
    Undefined,
    Null,
    Boolean(bool),
    Number(f64),
    /// Stores ID into global `StringPool`.
    StringId(i32),
    /// Stores ID into global `ObjectPool`.
    ObjectId(i32),
}

impl JSValue {
    pub fn undefined() -> Self {
        Self::Undefined
    }

    pub fn null() -> Self {
        Self::Null
    }

    pub fn boolean(b: bool) -> Self {
        Self::Boolean(b)
    }

    pub fn number(f: f64) -> Self {
        Self::Number(f)
    }

    pub fn str_id(id: i32) -> Self {
        Self::StringId(id)
    }

    pub fn obj_id(id: i32) -> Self {
        Self::ObjectId(id)
    }

    pub fn tag(&self) -> JSVTag {
        match self {
            Self::Undefined => JSVTag::Undefined,
            Self::Null => JSVTag::Null,
            Self::Boolean(_) => JSVTag::Boolean,
            Self::Number(_) => JSVTag::Number,
            Self::StringId(_) => JSVTag::String,
            Self::ObjectId(_) => JSVTag::Object,
        }
    }

    pub fn is_undefined(&self) -> bool {
        matches!(*self, Self::Undefined)
    }

    pub fn is_null(&self) -> bool {
        matches!(*self, Self::Null)
    }

    pub fn get_boolean(&self) -> bool {
        match self {
            Self::Undefined | Self::Null => false,
            Self::Boolean(b) => *b,
            Self::Number(f) => {
                !f.is_nan() && *f != 0.0
            },
            Self::StringId(_) | Self::ObjectId(_) => true,
        }
    }

    pub fn get_number(&self) -> Option<f64> {
        if let Self::Number(n) = self {
            return Some(*n);
        }

        match self {
            Self::Undefined => Some(f64::NAN),
            Self::Null => Some(0.0),
            Self::Boolean(b) => Some(if *b {1.0} else {0.0}),
            _ => None,
        }
    }

    pub fn get_str_id(&self) -> Option<i32> {
        if let Self::StringId(id) = self {
            return Some(*id);
        }

        None
    }

    pub fn get_obj_id(&self) -> Option<i32> {
        if let Self::ObjectId(id) = self {
            return Some(*id);
        }

        None
    }

    /// **NOTE:** For correct strict equality comparisons, chain this method with a custom function to compare strings or objects.
    pub fn primitive_strict_eq(&self, rhs: &Self) -> bool {
        if self.tag() != rhs.tag() {
            return false;
        }

        match self {
            Self::Undefined | Self::Null => true,
            Self::Boolean(left_b) => *left_b == rhs.get_boolean(),
            Self::Number(left_n) => *left_n == rhs.get_number().expect("Expected number value in JSValue::strict_eq()"),
            Self::StringId(left_sid) => *left_sid == rhs.get_str_id().expect("Expected string-pool-id in JSValue::strict_eq()"),
            Self::ObjectId(left_oid) => *left_oid == rhs.get_obj_id().expect("Expected object-id in JSValue::strict_eq()"),
        }
    }

    /// **NOTE:** For correct strict equality comparisons, chain this method with a custom function to compare strings.
    pub fn primitive_lt(&self, rhs: &Self) -> bool {
        let lhs_n = self.get_number();
        let rhs_n = rhs.get_number();

        if lhs_n.is_none() || rhs_n.is_none() {
            return false;
        }

        lhs_n.expect("Expected LHS as number in JSValue::primitive_lt().") < rhs_n.expect("Expected RHS as a number in JSValue::primitive_lt().")
    }

    /// **NOTE:** For correct strict equality comparisons, chain this method with a custom function to compare strings.
    pub fn primitive_gt(&self, rhs: &Self) -> bool {
        let lhs_n = self.get_number();
        let rhs_n = rhs.get_number();

        if lhs_n.is_none() || rhs_n.is_none() {
            return false;
        }

        lhs_n.expect("Expected LHS as number in JSValue::primitive_lt().") > rhs_n.expect("Expected RHS as a number in JSValue::primitive_lt().")
    }

}

/// **NOTE:** Into is used here since JS has special `toBoolean` logic which is not 1-to-1 reversible.
#[allow(clippy::from_over_into)]
impl Into<bool> for JSValue {
    fn into(self) -> bool {
        match self {
            Self::Undefined | Self::Null => false,
            Self::Boolean(b) => b,
            Self::Number(f) => {
                !f.is_nan() && f != 0.0
            },
            Self::StringId(_) | Self::ObjectId(_) => true,
        }
    }
}

impl From<i32> for JSValue {
    fn from(value: i32) -> Self {
        Self::Number(value as f64)
    }
}

impl From<u32> for JSValue {
    fn from(value: u32) -> Self {
        Self::Number(value as f64)
    }
}

impl Display for JSValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Undefined => write!(f, "JSValue(undefined)"),
            Self::Null => write!(f, "JSValue(null)"),
            Self::Boolean(b) => write!(f, "JSValue({})", *b),
            Self::Number(n) => write!(f, "JSValue({})", *n),
            Self::StringId(sid) => write!(f, "JSValue(sid-{})", *sid),
            Self::ObjectId(oid) => write!(f, "JSValue(oid-{})", *oid),
        }
    }
}
