#![no_std]

extern crate alloc;

mod diagnostic;

use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};
use musubu_cache::Cache;
use musubu_ir::CompiledFunction;
use musubu_primitive::Value;
pub use musubu_vm::ExecutionLimits;

static NEXT_PROGRAM: AtomicU64 = AtomicU64::new(1);

fn program_identity() -> u64 {
    // Never recycle identities, even when an engine address is reused.
    NEXT_PROGRAM
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
        .expect("program identity space exhausted")
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct FunctionToken {
    pub program: u64,
    pub function: usize,
}
use musubu_vm::{VM, VMResult};

// 外部で保持してもらう

#[derive(Debug)]
#[repr(C)]
pub struct MusubuEngine {
    cache: Cache,
    compile_error: String,
    runtime_error: String,
    limits: ExecutionLimits,
    program: u64,
    source: String,
    filename: String,
}

impl Default for MusubuEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl MusubuEngine {
    pub fn new() -> Self {
        Self {
            cache: Cache::new(),
            compile_error: String::new(),
            runtime_error: String::new(),
            limits: ExecutionLimits::default(),
            program: program_identity(),
            source: String::new(),
            filename: String::new(),
        }
    }

    pub fn register_function(
        &mut self,
        function_id: usize,
        function: CompiledFunction,
    ) -> Result<(), musubu_ir::InvalidIR> {
        self.cache.register_function(function_id, function)?;
        self.program = program_identity();
        Ok(())
    }

    /// Atomically replaces all compiled functions, including their name allocator.
    pub fn replace_program(&mut self, program: Self) {
        self.cache = program.cache;
        self.source = program.source;
        self.filename = program.filename;
        self.program = program_identity();
    }

    pub fn run_function(&self, function_id: usize, args: Vec<Value>) -> VMResult<Option<Value>> {
        let mut vm = VM::with_limits(&self.cache, self.limits);
        vm.run_function(function_id, args)
    }

    pub fn set_source(&mut self, filename: &str, source: &str) {
        self.filename = String::from(filename);
        self.source = String::from(source);
    }

    pub fn run_function_diagnostic(
        &self,
        id: usize,
        args: Vec<Value>,
    ) -> Result<Option<Value>, String> {
        let mut vm = VM::with_limits(&self.cache, self.limits);
        vm.run_function(id, args).map_err(|error| {
            diagnostic::render(
                &self.cache,
                &self.filename,
                &self.source,
                &error,
                vm.stack_trace(),
            )
        })
    }

    pub fn set_execution_limits(&mut self, limits: ExecutionLimits) -> bool {
        if limits.instructions == 0 || limits.call_depth == 0 {
            return false;
        }
        self.limits = limits;
        true
    }

    pub fn get_cache(&mut self) -> &mut Cache {
        self.program = program_identity();
        &mut self.cache
    }

    pub fn get_function_token(&self, name: &str) -> Option<FunctionToken> {
        self.get_function_id(name).map(|function| FunctionToken {
            program: self.program,
            function,
        })
    }

    pub fn token_function(&self, token: FunctionToken) -> Option<usize> {
        (token.program == self.program && self.get_function(token.function).is_some())
            .then_some(token.function)
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
