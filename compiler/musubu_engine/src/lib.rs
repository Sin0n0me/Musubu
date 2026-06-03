//#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use musubu_cache::Cache;
use musubu_ir::CompiledFunction;
use musubu_primitive::Value;
use musubu_vm::{VM, VMResult};

// 外部で保持してもらう

#[derive(Debug)]
#[repr(C)]
pub struct MusubuEngine {
    cache: Cache,
}

impl MusubuEngine {
    pub fn new() -> Self {
        Self {
            cache: Cache::new(),
        }
    }

    pub fn register_function(&mut self, function_id: usize, function: CompiledFunction) {
        self.cache.register_function(function_id, function);
    }

    pub fn run_function(&self, function_id: usize, args: Vec<Value>) -> VMResult<Option<Value>> {
        let mut vm = VM::new(&self.cache);
        vm.run_function(function_id, args)
    }

    pub fn get_cache(&mut self) -> &mut Cache {
        &mut self.cache
    }
}
