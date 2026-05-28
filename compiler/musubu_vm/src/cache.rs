use alloc::collections::btree_map::BTreeMap;
use alloc::vec::Vec;
use musubu_ir::CompiledFunction;

use crate::VMResult;
use crate::errors::VMError;

#[derive(Debug)]
pub struct VMCache {
    functions: BTreeMap<usize, CompiledFunction>,
    next_function: usize,
}

impl VMCache {
    pub fn new() -> Self {
        Self {
            functions: BTreeMap::new(),
            next_function: 0,
        }
    }

    pub fn register_function(&mut self, id: usize, function: CompiledFunction) {
        self.functions.insert(id, function);
    }

    pub fn get(&self, id: &usize) -> VMResult<&CompiledFunction> {
        self.functions.get(id).ok_or(VMError::IllegalFunctionCall)
    }
}
