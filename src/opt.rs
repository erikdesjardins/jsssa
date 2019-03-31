use crate::ir;
use crate::ir::fold::{Folder, RunFolder};
use crate::swc_globals;
use crate::utils::default_hash;

mod dce;

#[cfg(test)]
mod tests;

pub fn run_opts(_: &swc_globals::Initialized, ir: ir::Block) -> ir::Block {
    OptContext::new(ir).converge::<dce::Dce>("dce").into_inner()
}

struct OptContext(ir::Block);

impl OptContext {
    fn new(block: ir::Block) -> Self {
        Self(block)
    }

    fn into_inner(self) -> ir::Block {
        self.0
    }

    fn run<F: Folder + Default>(self, name: &str) -> Self {
        log::debug!("{}: running single pass", name);
        Self(F::default().run_folder(self.0))
    }

    fn converge<F: Folder + Default>(self, name: &str) -> Self {
        self.converge_with(name, |cx| cx.run::<F>(name))
    }

    fn converge_with(self, name: &str, mut f: impl FnMut(Self) -> Self) -> Self {
        let mut this = self;
        let mut last_hash = default_hash(&this.0);
        log::debug!("{}: starting opt-to-convergence, hash {}", name, last_hash);
        let mut iter = 0u64;
        loop {
            iter += 1;
            this = f(this);
            let hash = default_hash(&this.0);
            if hash == last_hash {
                log::debug!("{}: stopping opt-to-convergence, iteration {}", name, iter);
                return this;
            } else {
                log::debug!("{}: continuing opt-to-convergence, hash {}", name, hash);
            }
            last_hash = hash;
        }
    }
}
