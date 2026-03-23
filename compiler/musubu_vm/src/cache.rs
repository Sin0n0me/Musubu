use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::{OnceLock, RwLock};

use musubu_ir::CompiledFunction;

// first: function id
type FunctionHashMap = HashMap<usize, CompiledFunction>;

static FUNCTION_CACHE: OnceLock<RwLock<FunctionHashMap>> = OnceLock::new();

pub fn get_cache() -> &'static RwLock<FunctionHashMap> {
    FUNCTION_CACHE.get_or_init(|| RwLock::new(HashMap::new()))
}

//
pub fn calc_hash<T: Hash>(value: &T) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}
