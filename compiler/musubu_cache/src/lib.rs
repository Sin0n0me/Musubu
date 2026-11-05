#![no_std]

extern crate alloc;

pub mod errors;

use core::fmt::Debug;

use crate::errors::CacheError;
use alloc::collections::btree_map::BTreeMap;
use alloc::string::String;
use musubu_ir::CompiledFunction;
use musubu_primitive::PrimitiveType;

pub type CacheResult<T> = Result<T, CacheError>;

#[derive(Debug)]
pub struct Cache {
    function_cache: FunctionCache,
    type_cache: TypeCache,
}

impl Default for Cache {
    fn default() -> Self {
        Self::new()
    }
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

    pub fn register_function(
        &mut self,
        id: usize,
        function: CompiledFunction,
    ) -> Result<(), musubu_ir::InvalidIR> {
        function.validate()?;
        self.function_cache.register(id, function);
        Ok(())
    }

    pub fn get_function(&self, id: &usize) -> Option<&CompiledFunction> {
        self.function_cache.get(id)
    }

    pub fn get_function_id(&self, name: &str) -> Option<usize> {
        let id = *self.function_cache.allocator.name_map.get(name)?;
        self.get_function(&id).map(|_| id)
    }

    pub fn get_function_name(&self, id: usize) -> Option<&str> {
        self.function_cache
            .allocator
            .name_map
            .iter()
            .find_map(|(name, value)| (*value == id).then_some(name.as_str()))
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
}

impl TypeCache {
    fn new() -> Self {
        Self {
            types: BTreeMap::new(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn allocator_reuses_names_without_consuming_ids() {
        let mut cache = Cache::new();
        let allocator = cache.get_function_allocator();
        assert_eq!(allocator.alloc("first".into()), 0);
        assert_eq!(allocator.alloc("first".into()), 0);
        assert_eq!(allocator.alloc("second".into()), 1);
        assert_eq!(allocator.alloc("first".into()), 0);
        assert_eq!(allocator.alloc("third".into()), 2);
    }

    #[test]
    fn caches_allocate_independently() {
        let mut first = Cache::new();
        let mut second = Cache::new();
        first.get_function_allocator().alloc("a".into());
        assert_eq!(second.get_function_allocator().alloc("b".into()), 0);
    }

    #[test]
    fn sparse_function_registration_and_replacement() {
        let mut cache = Cache::new();
        assert!(cache.get_function(&42).is_none());
        cache
            .register_function(
                42,
                CompiledFunction {
                    code: vec![],
                    source_ranges: alloc::vec::Vec::new(),
                    registers: 3,
                    parameters: vec![],
                    return_type: PrimitiveType::Unit,
                },
            )
            .unwrap();
        cache
            .register_function(
                7,
                CompiledFunction {
                    code: vec![],
                    source_ranges: alloc::vec::Vec::new(),
                    registers: 1,
                    parameters: vec![],
                    return_type: PrimitiveType::Unit,
                },
            )
            .unwrap();
        cache
            .register_function(
                42,
                CompiledFunction {
                    code: vec![],
                    source_ranges: alloc::vec::Vec::new(),
                    registers: 8,
                    parameters: vec![],
                    return_type: PrimitiveType::Unit,
                },
            )
            .unwrap();
        assert_eq!(cache.get_function(&42).unwrap().registers, 8);
        assert_eq!(cache.get_function(&7).unwrap().registers, 1);
        assert!(cache.get_function(&0).is_none());
    }

    #[test]
    fn type_registration_does_not_modify_function_cache() {
        let mut cache = Cache::new();
        cache.register_type(7, PrimitiveType::default_integer());
        assert_eq!(
            cache.type_cache.get(&7),
            Some(&PrimitiveType::default_integer())
        );
        cache.register_type(7, PrimitiveType::default_float());
        assert_eq!(
            cache.type_cache.get(&7),
            Some(&PrimitiveType::default_float())
        );
        assert!(cache.get_function(&7).is_none());
    }
}
