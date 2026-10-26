#![no_std]

extern crate alloc;

use alloc::string::String;
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
    compile_error: String,
    runtime_error: String,
}

impl MusubuEngine {
    pub fn new() -> Self {
        Self {
            cache: Cache::new(),
            compile_error: String::new(),
            runtime_error: String::new(),
        }
    }

    pub fn register_function(&mut self, function_id: usize, function: CompiledFunction) {
        self.cache.register_function(function_id, function);
    }

    /// Atomically replaces all compiled functions, including their name allocator.
    pub fn replace_program(&mut self, program: Self) {
        self.cache = program.cache;
    }

    pub fn run_function(&self, function_id: usize, args: Vec<Value>) -> VMResult<Option<Value>> {
        let mut vm = VM::new(&self.cache);
        vm.run_function(function_id, args)
    }

    pub fn get_cache(&mut self) -> &mut Cache {
        &mut self.cache
    }

    pub fn get_function_id(&self, name: &str) -> Option<usize> {
        self.cache.get_function_id(name)
    }

    pub fn get_function(&self, id: usize) -> Option<&CompiledFunction> {
        self.cache.get_function(&id)
    }

    pub fn runtime_error(&self) -> &str {
        &self.runtime_error
    }

    pub fn set_runtime_error(&mut self, error: String) {
        self.runtime_error = error;
    }

    pub fn clear_runtime_error(&mut self) {
        self.runtime_error.clear();
    }

    /// The UTF-8 diagnostic from the most recent failed compilation.
    pub fn compile_error(&self) -> &str {
        &self.compile_error
    }

    pub fn set_compile_error(&mut self, error: String) {
        self.compile_error = error;
    }

    pub fn clear_compile_error(&mut self) {
        self.compile_error.clear();
    }
}
