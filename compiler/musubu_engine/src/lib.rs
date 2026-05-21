#![no_std]

extern crate alloc;

use alloc::collections::btree_map::BTreeMap;
use musubu_ir::CompiledFunction;

// 外部で保持してもらう

#[derive(Debug)]
#[repr(C)]
pub struct MusubuEngine {
    cache: VMCache,
}

impl MusubuEngine {
    pub fn register_function(&mut self, function_id: usize, function: CompiledFunction) {
        self.cache.functions.insert(function_id, function);
    }


    pub fn run_function( function_id: usize, args: Vec<Value>)) -> Option<Value>{
        
    }

}

#[derive(Debug)]
struct VMCache {
    functions: BTreeMap<usize, CompiledFunction>,
}
