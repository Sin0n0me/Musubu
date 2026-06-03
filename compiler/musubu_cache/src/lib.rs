#![no_std]

extern crate alloc;

pub mod errors;

use core::fmt::Debug;

use crate::errors::CacheError;
use alloc::collections::btree_map::BTreeMap;
use alloc::string::{String, ToString};
use musubu_ir::CompiledFunction;
use musubu_primitive::PrimitiveType;

pub type CacheResult<T> = Result<T, CacheError>;

#[derive(Debug)]
pub struct Cache {
    function_cache: FunctionCache,
    type_cache: TypeCache,
}

impl Cache {
    pub fn new() -> Self {
        Self {
            function_cache: FunctionCache::new(),
            type_cache: TypeCache::new(),
        }
    }

    pub fn register_type(&mut self, id: usize, primitive_type: PrimitiveType) {
        self.type_cache.register(id, primitive_type);
    }

    pub fn register_function(&mut self, id: usize, function: CompiledFunction) {
        self.function_cache.register(id, function);
    }

    pub fn get_function(&self, id: &usize) -> Option<&CompiledFunction> {
        self.function_cache.get(id)
    }

    pub fn get_function_allocator(&mut self) -> &mut impl Allocator {
        self.function_cache.get_allocator()
    }
}

trait CacheCollector<T> {
    fn register(&mut self, id: usize, value: T);

    fn get(&self, id: &usize) -> Option<&T>;
}

#[derive(Debug)]
struct FunctionCache {
    functions: BTreeMap<usize, CompiledFunction>,
    allocator: LinearAllocator,
}

impl FunctionCache {
    fn new() -> Self {
        Self {
            functions: BTreeMap::new(),
            allocator: LinearAllocator::new(),
        }
    }

    fn get_allocator(&mut self) -> &mut impl Allocator {
        &mut self.allocator
    }
}

impl CacheCollector<CompiledFunction> for FunctionCache {
    fn register(&mut self, id: usize, value: CompiledFunction) {
        self.functions.insert(id, value);
    }

    fn get(&self, id: &usize) -> Option<&CompiledFunction> {
        self.functions.get(id)
    }
}

#[derive(Debug)]
struct TypeCache {
    types: BTreeMap<usize, PrimitiveType>,
    allocator: LinearAllocator,
}

impl TypeCache {
    fn new() -> Self {
        Self {
            types: BTreeMap::new(),
            allocator: LinearAllocator::new(),
        }
    }
}

impl CacheCollector<PrimitiveType> for TypeCache {
    fn register(&mut self, id: usize, value: PrimitiveType) {
        self.types.insert(id, value);
    }

    fn get(&self, id: &usize) -> Option<&PrimitiveType> {
        self.types.get(id)
    }
}

pub trait Allocator: Debug {
    fn alloc(&mut self, name: String) -> usize;
}

#[derive(Debug)]
struct LinearAllocator {
    name_map: BTreeMap<String, usize>,
    next: usize,
}

impl LinearAllocator {
    fn new() -> Self {
        Self {
            name_map: BTreeMap::new(),
            next: 0,
        }
    }
}

impl Allocator for LinearAllocator {
    fn alloc(&mut self, name: String) -> usize {
        *self.name_map.entry(name).or_insert_with(|| {
            let id = self.next;
            self.next += 1;
            id
        })
    }
}
