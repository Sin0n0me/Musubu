use alloc::collections::btree_map::BTreeMap;
use alloc::vec::Vec;
use musubu_ir::CompiledFunction;

use crate::VMResult;
use crate::errors::VMError;

#[derive(Debug)]
pub struct VMCache {
    functions: BTreeMap<usize, CompiledFunction>,
}

impl VMCache {
    pub fn register_function(&mut self, id: usize, function: CompiledFunction) {
        self.functions.insert(id, function);
    }

    pub fn get(&self, id: &usize) -> VMResult<&CompiledFunction> {
        self.functions.get(id).ok_or(VMError::IllegalFunctionCall)
    }
}
