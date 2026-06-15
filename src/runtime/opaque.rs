use crate::runtime::code::Chunk;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum JSInternalTag {
    /// For plain generic JS objects.
    Empty,
    /// Holds a `Chunk` of runtime code.
    Code,
    /// Stores a closure pool ID.
    ClosureID
}

#[derive(Clone, Copy)]
pub union JSInternal {
    pub code: *mut Chunk,
    pub closure_id: i32,
    pub dud: u8,
}

#[derive(Clone, Copy)]
pub struct JSOpaque {
    pub internal: JSInternal,
    pub tag: JSInternalTag,
}

impl Default for JSOpaque {
    fn default() -> Self {
        Self {
            internal: JSInternal { dud: 0 },
            tag: JSInternalTag::Empty
        }
    }
}

impl JSOpaque {
    pub fn bytecode(code: *mut Chunk) -> Self {
        Self {
            internal: JSInternal {
                code,
            },
            tag: JSInternalTag::Code
        }
    }

    pub fn closure_id(handle_id: i32) -> Self {
        Self {
            internal: JSInternal {
                closure_id: handle_id
            },
            tag: JSInternalTag::ClosureID
        }
    }

    pub fn has_discriminant(&self, tag: JSInternalTag) -> bool {
        self.tag == tag
    }

    /// # Safety
    /// This function tries to get the contained `JSInternal` as a bytecode reference. However, non-bytecode internals will give a null-mut ptr that must be checked for properly. It's also up to the user to ensure that the pointer doesn't dangle!
    pub unsafe fn as_bytecode(&self) -> *mut Chunk {
        if self.tag == JSInternalTag::Code {
            unsafe {self.internal.code}
        } else {
            std::ptr::null_mut()
        }
    }

    /// # Safety
    /// This function tries to get the contained `JSInternal` as a more stable handle to some closure. However, non-closures will give `None`.
    pub fn as_closure_id(&self) -> Option<i32> {
        if self.tag == JSInternalTag::ClosureID {
            unsafe {Some(self.internal.closure_id)}
        } else {
            None
        }
    }
}
