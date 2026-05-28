//#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use musubu_ir::CompiledFunction;
use musubu_primitive::Value;
use musubu_vm::cache::VMCache;
use musubu_vm::{VM, VMResult};

// 外部で保持してもらう

#[derive(Debug)]
#[repr(C)]
pub struct MusubuEngine {
    cache: VMCache,
}

impl MusubuEngine {
    pub fn new() -> Self {
        Self {
            cache: VMCache::new(),
        }
    }

    pub fn register_function(&mut self, function_id: usize, function: CompiledFunction) {
        self.cache.register_function(function_id, function);
    }

    pub fn run_function(&self, function_id: usize, args: Vec<Value>) -> VMResult<Option<Value>> {
        let mut vm = VM::new(&self.cache);
        vm.run_function(function_id, args)
    }
}
