use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;

use swc_atoms::JsWord;

use crate::ir::{Mutable, Ref, SSA};

#[derive(Default)]
pub struct Ast<'a> {
    ident_to_mut_ref: HashMap<JsWord, Ref<Mutable>>,
    parent: Option<&'a Ast<'a>>,
}

impl<'a> Ast<'a> {
    pub fn nested(&'a self) -> Ast<'a> {
        Self {
            ident_to_mut_ref: Default::default(),
            parent: Some(self),
        }
    }

    pub fn get_mutable(&self, ident: &JsWord) -> Option<&Ref<Mutable>> {
        self.get_mutable_in_current(ident)
            .or_else(|| self.parent.and_then(|p| p.get_mutable_in_current(ident)))
    }

    pub fn get_mutable_in_current(&self, ident: &JsWord) -> Option<&Ref<Mutable>> {
        self.ident_to_mut_ref.get(ident)
    }

    pub fn declare_mutable(&mut self, ident: JsWord) -> Ref<Mutable> {
        let ref_ = Ref::new(&ident);
        self.ident_to_mut_ref.insert(ident, ref_.clone());
        ref_
    }
}

#[derive(Default)]
pub struct Ir<'a> {
    seen_prefix_hashes: HashMap<u64, u64>,
    mutable_names: HashMap<Ref<Mutable>, JsWord>,
    ssa_names: HashMap<Ref<SSA>, JsWord>,
    _freeze_parent_while_nested: PhantomData<&'a ()>,
}

impl Ir<'_> {
    pub fn nested(&self) -> Ir<'_> {
        Self {
            seen_prefix_hashes: self.seen_prefix_hashes.clone(),
            mutable_names: self.mutable_names.clone(),
            ssa_names: self.ssa_names.clone(),
            _freeze_parent_while_nested: PhantomData,
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
