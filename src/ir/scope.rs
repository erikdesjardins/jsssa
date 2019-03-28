use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use swc_atoms::JsWord;

use crate::ir::{Mutable, Ref, SSA};

#[derive(Default)]
pub struct Ast {
    ident_to_mut_ref: HashMap<JsWord, Ref<Mutable>>,
}

impl Ast {
    pub fn nested(&self) -> Self {
        Self {
            ident_to_mut_ref: self.ident_to_mut_ref.clone(),
        }
    }

    pub fn get_mutable(&self, ident: &JsWord) -> Option<&Ref<Mutable>> {
        self.ident_to_mut_ref.get(ident)
    }

    pub fn declare_mutable(&mut self, ident: JsWord, ref_: Ref<Mutable>) {
        self.ident_to_mut_ref.insert(ident, ref_);
    }
}

#[derive(Default)]
pub struct Ir {
    seen_prefix_hashes: HashMap<u64, u64>,
    mutable_names: HashMap<Ref<Mutable>, JsWord>,
    ssa_names: HashMap<Ref<SSA>, JsWord>,
}

impl Ir {
    pub fn nested(&self) -> Self {
        Self {
            seen_prefix_hashes: self.seen_prefix_hashes.clone(),
            mutable_names: self.mutable_names.clone(),
            ssa_names: self.ssa_names.clone(),
        }
    }

    pub fn register_global(&mut self, name: &str) {
        *self
            .seen_prefix_hashes
            .entry(self.hash_prefix(name))
            .or_default() += 1;
    }

    pub fn get_mutable(&self, ref_: &Ref<Mutable>) -> Option<JsWord> {
        self.mutable_names.get(ref_).cloned()
    }

    pub fn declare_mutable(&mut self, ref_: Ref<Mutable>) -> JsWord {
        let name = self.unique_name(ref_.name_hint());
        let old_name = self.mutable_names.insert(ref_, name.clone());
        assert!(old_name.is_none(), "mutable vars can only be declared once");
        name
    }

    pub fn get_ssa(&self, ref_: &Ref<SSA>) -> Option<JsWord> {
        self.ssa_names.get(ref_).cloned()
    }

    pub fn declare_ssa(&mut self, ref_: Ref<SSA>) -> JsWord {
        let name = self.unique_name(ref_.name_hint());
        let old_name = self.ssa_names.insert(ref_, name.clone());
        assert!(old_name.is_none(), "SSA vars can only be declared once");
        name
    }

    fn hash_prefix(&self, prefix: &str) -> u64 {
        let mut hasher = DefaultHasher::new();
        prefix.hash(&mut hasher);
        hasher.finish()
    }

    fn unique_name(&mut self, prefix: &str) -> JsWord {
        let prefix = match prefix {
            "" => "_",
            _ => prefix,
        };

        let suffix_counter = {
            // hash collisions are fine; we'll just end up being overly conservative
            let counter = self
                .seen_prefix_hashes
                .entry(self.hash_prefix(prefix))
                .or_default();
            let old_value = *counter;
            *counter += 1;
            old_value
        };

        if suffix_counter == 0 {
            // if this prefix has never been seen, emit it directly
            JsWord::from(prefix)
        } else {
            self.unique_name(&format!("{}${}", prefix, suffix_counter))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic() {
        let mut p = Ir::default();
        assert_eq!(p.unique_name("foo").as_ref(), "foo");
        assert_eq!(p.unique_name("foo").as_ref(), "foo$1");
        assert_eq!(p.unique_name("foo").as_ref(), "foo$2");
        assert_eq!(p.unique_name("foo").as_ref(), "foo$3");
    }

    #[test]
    fn replacement_overlap1() {
        let mut p = Ir::default();
        assert_eq!(p.unique_name("foo").as_ref(), "foo");
        assert_eq!(p.unique_name("foo$1").as_ref(), "foo$1");
        assert_eq!(p.unique_name("foo").as_ref(), "foo$1$1");
    }

    #[test]
    fn replacement_overlap2() {
        let mut p = Ir::default();
        assert_eq!(p.unique_name("foo").as_ref(), "foo");
        assert_eq!(p.unique_name("foo").as_ref(), "foo$1");
        assert_eq!(p.unique_name("foo$1").as_ref(), "foo$1$1");
    }

    #[test]
    fn empty_string() {
        let mut p = Ir::default();
        assert_eq!(p.unique_name("").as_ref(), "_");
        assert_eq!(p.unique_name("").as_ref(), "_$1");
    }
}
