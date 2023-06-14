use std::{
    hash::{Hash, Hasher},
    ops::Index,
};

macro_rules! id_impl {
    ($name:ident) => {
        /// u32 allows up to 4,294,967,295 entities with just 4 bytes of storage - which is more than enough forever
        #[derive(Clone, Copy, Eq, PartialEq)]
        pub struct $name(u32);
        // Allows easy construction from a usize with `$name::from(usize)`
        impl From<usize> for $name {
            fn from(val: usize) -> Self {
                return Self(val as u32);
            }
        }
        // Allows easy conversion to a usize with `val.into()`
        impl From<$name> for usize {
            fn from(val: $name) -> usize {
                return val.0 as usize;
            }
        }
        // Allows us to directly use this type as an indexer into a vec, which is a nice convenience sugar
        impl<T> Index<$name> for Vec<T> {
            type Output = T;

            fn index(&self, index: $name) -> &Self::Output {
                return &self[index.0 as usize];
            }
        }
        impl Hash for $name {
            fn hash<H: Hasher>(&self, state: &mut H) {
                self.0.hash(state);
            }
        }
    };
}

id_impl!(ModuleId);
id_impl!(PathId);

/// Defines a small struct which maintains the canonical path for a given module
/// Technically we could "do away" with this and solely use paths for everything
/// But this provides a nice abstraction to help distinguish different code locations
///
/// Both IDs are a u32 so combined this struct is just 8 bytes - which is the same size as a 64-bit pointer!
/// This means that copying a Module should be the same speed and size as passing a pointer - which saves us having to
/// worry as much about trying to deduplicate references and ownership - we just default to copying everything
#[derive(Clone, Copy, Eq)]
pub struct Module {
    pub path_id: PathId,
    pub module_id: ModuleId,
}
impl PartialEq<Module> for Module {
    fn eq(&self, other: &Self) -> bool {
        return self.module_id == other.module_id;
    }
}
impl Hash for Module {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.module_id.hash(state);
    }
}
