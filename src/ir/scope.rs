use std::collections::HashSet;

use swc_atoms::JsWord;

use crate::collections::StackedMap;
use crate::ir::{Lbl, Mut, Ref, Ssa};
use crate::utils::default_hash;

#[derive(Default)]
pub struct Ast<'a> {
    ident_to_mut_ref: StackedMap<'a, JsWord, Ref<Mut>>,
    ident_to_label_ref: StackedMap<'a, JsWord, Ref<Lbl>>,
}

impl<'a> Ast<'a> {
    pub fn nested(&'a self) -> Ast<'a> {
        Self {
            ident_to_mut_ref: self.ident_to_mut_ref.child(),
            ident_to_label_ref: self.ident_to_label_ref.child(),
        }
    }

    pub fn get_mutable(&self, ident: &JsWord) -> Option<&Ref<Mut>> {
        self.ident_to_mut_ref.get_all(ident)
    }

    pub fn get_mutable_in_current(&self, ident: &JsWord) -> Option<&Ref<Mut>> {
        self.ident_to_mut_ref.get_self(ident)
    }

    pub fn declare_mutable(&mut self, ident: JsWord) -> Ref<Mut> {
        let ref_ = Ref::new(&ident);
        let old_ref = self.ident_to_mut_ref.insert_self(ident, ref_.clone());
        assert!(old_ref.is_none(), "mutable vars can only be declared once");
        ref_
    }

    pub fn get_label(&self, ident: &JsWord) -> Option<&Ref<Lbl>> {
        self.ident_to_label_ref.get_all(ident)
    }

    pub fn declare_label(&mut self, ident: JsWord) -> Ref<Lbl> {
        let ref_ = Ref::new(&ident);
        let old_ref = self.ident_to_label_ref.insert_self(ident, ref_.clone());
        assert!(old_ref.is_none(), "labels can only be declared once");
        ref_
    }
}

pub struct Ir<'a> {
    minified_name_counter: Option<usize>,
    seen_prefix_hashes: StackedMap<'a, u64, u64>,
    mutable_names: StackedMap<'a, Ref<Mut>, JsWord>,
    label_names: StackedMap<'a, Ref<Lbl>, JsWord>,
    ssa_names: StackedMap<'a, Ref<Ssa>, JsWord>,
}

pub struct Opt {
    pub minify: bool,
}

impl<'a> Ir<'a> {
    pub fn with_globals(globals: HashSet<&str>, options: Opt) -> Self {
        Self {
            minified_name_counter: match options.minify {
                true => Some(0),
                false => None,
            },
            seen_prefix_hashes: globals
                .into_iter()
                .map(|name| (default_hash(name), 1))
                .collect(),
            mutable_names: Default::default(),
            label_names: Default::default(),
            ssa_names: Default::default(),
        }
    }

    pub fn no_globals() -> Self {
        Self {
            minified_name_counter: None,
            seen_prefix_hashes: Default::default(),
            mutable_names: Default::default(),
            label_names: Default::default(),
            ssa_names: Default::default(),
        }
    }

    pub fn nested(&'a self) -> Ir<'a> {
        Self {
            minified_name_counter: self.minified_name_counter,
            seen_prefix_hashes: self.seen_prefix_hashes.child(),
            mutable_names: self.mutable_names.child(),
            label_names: self.label_names.child(),
            ssa_names: self.ssa_names.child(),
        }
    }

    pub fn get_mutable(&self, ref_: &Ref<Mut>) -> Option<JsWord> {
        self.mutable_names.get_all(ref_).cloned()
    }

    pub fn declare_mutable(&mut self, ref_: Ref<Mut>) -> JsWord {
        let name = self.unique_name(ref_.name_hint());
        let old_name = self.mutable_names.insert_self(ref_, name.clone());
        assert!(old_name.is_none(), "mutable vars can only be declared once");
        name
    }

    pub fn get_label(&self, ref_: &Ref<Lbl>) -> Option<JsWord> {
        self.label_names.get_all(ref_).cloned()
    }

    pub fn declare_label(&mut self, ref_: Ref<Lbl>) -> JsWord {
        let name = self.unique_name(ref_.name_hint());
        let old_name = self.label_names.insert_self(ref_, name.clone());
        assert!(old_name.is_none(), "labels can only be declared once");
        name
    }

    pub fn get_ssa(&self, ref_: &Ref<Ssa>) -> Option<JsWord> {
        self.ssa_names.get_all(ref_).cloned()
    }

    pub fn declare_ssa(&mut self, ref_: Ref<Ssa>) -> JsWord {
        let name = self.unique_name(ref_.name_hint());
        let old_name = self.ssa_names.insert_self(ref_, name.clone());
        assert!(old_name.is_none(), "ssa vars can only be declared once");
        name
    }

    fn minified_name(mut index: usize) -> String {
        #[rustfmt::skip]
        const FIRST_CHAR: [char; 26 * 2 + 2] = [
            'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z',
            'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z',
            '_', '$'
        ];
        #[rustfmt::skip]
        const OTHER_CHARS: [char; 26 * 2 + 10 + 2] = [
            'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z',
            'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z',
            '0', '1', '2', '3', '4', '5', '6', '7', '8', '9',
            '_', '$'
        ];

        let mut name = String::with_capacity(4);
        name.push(FIRST_CHAR[index % FIRST_CHAR.len()]);
        index /= FIRST_CHAR.len();
        while index > OTHER_CHARS.len() {
            name.push(OTHER_CHARS[index % OTHER_CHARS.len()]);
            index /= OTHER_CHARS.len();
        }
        if index > 0 {
            name.push(OTHER_CHARS[index - 1]);
        }
        name
    }

    fn unique_name(&mut self, prefix: &str) -> JsWord {
        if let Some(counter) = &mut self.minified_name_counter {
            let name = loop {
                let name = Ir::minified_name(*counter);
                if self
                    .seen_prefix_hashes
                    .get_all(&default_hash(name.as_str()))
                    .is_none()
                {
                    break name;
                }
                *counter += 1;
            };
            *counter += 1;
            JsWord::from(name)
        } else {
            let prefix = match prefix {
                "" => "_",
                _ => prefix,
            };

            let suffix_counter = {
                // hash collisions are fine; we'll just end up being overly conservative
                let hash = default_hash(prefix);
                let old_value = *self.seen_prefix_hashes.get_all(&hash).unwrap_or(&0);
                self.seen_prefix_hashes.insert_self(hash, old_value + 1);
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic() {
        let mut p = Ir::no_globals();
        assert_eq!(p.unique_name("foo").as_ref(), "foo");
        assert_eq!(p.unique_name("foo").as_ref(), "foo$1");
        assert_eq!(p.unique_name("foo").as_ref(), "foo$2");
        assert_eq!(p.unique_name("foo").as_ref(), "foo$3");
    }

    #[test]
    fn replacement_overlap1() {
        let mut p = Ir::no_globals();
        assert_eq!(p.unique_name("foo").as_ref(), "foo");
        assert_eq!(p.unique_name("foo$1").as_ref(), "foo$1");
        assert_eq!(p.unique_name("foo").as_ref(), "foo$1$1");
    }

    #[test]
    fn replacement_overlap2() {
        let mut p = Ir::no_globals();
        assert_eq!(p.unique_name("foo").as_ref(), "foo");
        assert_eq!(p.unique_name("foo").as_ref(), "foo$1");
        assert_eq!(p.unique_name("foo$1").as_ref(), "foo$1$1");
    }

    #[test]
    fn empty_string() {
        let mut p = Ir::no_globals();
        assert_eq!(p.unique_name("").as_ref(), "_");
        assert_eq!(p.unique_name("").as_ref(), "_$1");
    }

    #[test]
    fn minify_deconflict() {
        let mut p = Ir::with_globals(Some("b").into_iter().collect(), Opt { minify: true });
        assert_eq!(p.unique_name("").as_ref(), "a");
        assert_eq!(p.unique_name("").as_ref(), "c");
    }
}
