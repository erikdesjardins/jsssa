use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::BuildHasherDefault;

// use a deterministically-seeded map for consistent iteration order
pub type Map<K, V> = HashMap<K, V, BuildHasherDefault<DefaultHasher>>;
