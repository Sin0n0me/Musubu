#![no_std]

extern crate alloc;

pub mod errors;

use crate::errors::CacheError;
use alloc::collections::btree_map::BTreeMap;
use alloc::string::String;
use musubu_ir::CompiledFunction;
use musubu_primitive::PrimitiveType;

pub type CacheResult<T> = Result<T, CacheError>;

#[derive(Debug)]
pub struct Cache {
    functions: BTreeMap<usize, CompiledFunction>,
    types: BTreeMap<usize, PrimitiveType>,
}

impl Cache {
    pub fn new() -> Self {
        Self {
            functions: BTreeMap::new(),
            types: BTreeMap::new(),
        }
    }

    pub fn register_type(&mut self, id: usize, primitive_type: PrimitiveType) {
        self.types.insert(id, primitive_type);
    }

    pub fn register_function(&mut self, id: usize, function: CompiledFunction) {
        self.functions.insert(id, function);
    }

    pub fn get_function(&self, id: &usize) -> Option<&CompiledFunction> {
        self.functions.get(id)
    }
}

#[derive(Debug)]
struct Allocater {
    name_map: BTreeMap<String, usize>,
    next: usize,
}

impl Allocater {
    pub fn new() -> Self {
        Self {
            name_map: BTreeMap::new(),
            next: 0,
        }
    }

    pub fn alloc(&mut self, name: &str) -> usize {
        let id = self.next;
        self.next += 1;
        id
    }
}
