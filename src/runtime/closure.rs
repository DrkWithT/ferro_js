use crate::runtime::values::JSValue;
use crate::runtime::code::Chunk;

#[derive(Clone, Copy)]
pub struct JSClosure {
    pub env: JSValue,
    pub code: *mut Chunk,
}

impl JSClosure {
    pub fn new(env: JSValue, code: *mut Chunk) -> Self {
        Self {
            env,
            code
        }
    }
}
