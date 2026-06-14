use crate::runtime::values::JSValue;

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PropFlag {
    Writable = (1 << 0),
    Enumerable = (1 << 1),
    Configurable = (1 << 2),
    HasGetter = (1 << 4),
    HasSetter = (1 << 5)
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
pub struct Property {
    /// Stores `[[Value]]` or [[Get]] and `[[Set]]`.
    pub body: [JSValue; 2],
    pub flags: u8
}

impl Property {
    pub fn data(v: &JSValue, flags: u8) -> Self {
        Self {
            body: [*v, JSValue::Undefined],
            flags
        }
    }

    pub fn accessor(getter: &JSValue, setter: &JSValue, flags: u8) -> Self {
        Self {
            body: [*getter, *setter],
            flags: {
                let mut temp_flags = flags;

                if !getter.is_undefined() && !getter.is_null() {
                    temp_flags |= PropFlag::HasGetter as u8;
                }

                if !setter.is_undefined() && !setter.is_null() {
                    temp_flags |= PropFlag::HasSetter as u8;
                }

                temp_flags
            }
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

    pub fn is_accessor(&self) -> bool {
        (self.flags & (PropFlag::HasGetter as u8 | PropFlag::HasSetter as u8)) != 0
    }

    pub fn has_getter(&self) -> bool {
        0 != (self.flags & PropFlag::HasGetter as u8)
    }

    pub fn has_setter(&self) -> bool {
        0 != (self.flags & PropFlag::HasSetter as u8)
    }
}
