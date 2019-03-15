use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use swc_atoms::JsWord;

use super::{LiveRef, Mutable, Ref, SSA};

#[derive(Default, Clone)]
pub struct Ast {
    ident_to_mut_ref: HashMap<JsWord, Ref<Mutable>>,
}

impl Ast {
    pub fn get_mutable(&self, ident: &JsWord) -> Option<&Ref<Mutable>> {
        self.ident_to_mut_ref.get(ident)
    }

    pub fn declare_mutable(&mut self, ident: JsWord, ref_: Ref<Mutable>) {
        self.ident_to_mut_ref.insert(ident, ref_);
    }
}

#[derive(Default, Clone)]
pub struct Ir {
    seen_prefix_hashes: HashMap<u64, u64>,
    mutable_names: HashMap<LiveRef<Mutable>, JsWord>,
    ssa_names: HashMap<LiveRef<SSA>, JsWord>,
}

impl Ir {
    pub fn get_mutable(&self, ref_: &LiveRef<Mutable>) -> Option<&JsWord> {
        self.mutable_names.get(ref_)
    }

    pub fn declare_mutable(&mut self, ref_: LiveRef<Mutable>) -> JsWord {
        let name = self.unique_name(ref_.name_hint());
        let old_name = self.mutable_names.insert(ref_, name.clone());
        assert!(old_name.is_none());
        name
    }

    pub fn get_ssa(&self, ref_: &LiveRef<SSA>) -> Option<&JsWord> {
        self.ssa_names.get(ref_)
    }

    pub fn declare_ssa(&mut self, ref_: LiveRef<SSA>) -> JsWord {
        let name = self.unique_name(ref_.name_hint());
        let old_name = self.ssa_names.insert(ref_, name.clone());
        assert!(old_name.is_none());
        name
    }

    fn unique_name(&mut self, prefix: &str) -> JsWord {
        let mut hasher = DefaultHasher::new();
        prefix.hash(&mut hasher);
        let hash = hasher.finish();

        let suffix_counter = {
            // hash collisions are fine; we'll just end up being overly conservative
            let counter = self.seen_prefix_hashes.entry(hash).or_default();
            let old_value = *counter;
            *counter += 1;
            old_value
        };

        if suffix_counter == 0 && !prefix.contains('$') {
            // if this prefix has never been seen and cannot conflict with a suffix, emit it directly
            JsWord::from(prefix)
        } else {
            JsWord::from(format!("{}${}", prefix, suffix_counter).as_str())
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
        assert_eq!(p.unique_name("foo$1").as_ref(), "foo$1$0");
        assert_eq!(p.unique_name("foo").as_ref(), "foo$1");
    }

    #[test]
    fn replacement_overlap2() {
        let mut p = Ir::default();
        assert_eq!(p.unique_name("foo").as_ref(), "foo");
        assert_eq!(p.unique_name("foo").as_ref(), "foo$1");
        assert_eq!(p.unique_name("foo$1").as_ref(), "foo$1$0");
    }
}
