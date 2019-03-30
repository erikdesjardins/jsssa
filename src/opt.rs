use crate::ir;
use crate::ir::fold::{Folder, RunFolder};
use crate::swc_globals;
use crate::utils::default_hash;

mod dce;

#[cfg(test)]
mod tests;

pub fn run_opts(_: &swc_globals::Initialized, ir: ir::Block) -> ir::Block {
    OptContext::new(ir)
        .run_to_convergence(|cx| cx.run(dce::Dce))
        .into_inner()
}

struct OptContext(ir::Block);

impl OptContext {
    fn new(block: ir::Block) -> Self {
        Self(block)
    }

    fn into_inner(self) -> ir::Block {
        self.0
    }

    fn run(self, mut folder: impl Folder) -> Self {
        Self(folder.run_folder(self.0))
    }

    fn run_to_convergence(self, mut f: impl FnMut(Self) -> Self) -> Self {
        let mut this = self;
        let mut last_hash = default_hash(&this.0);
        log::debug!("Starting opt-to-convergence, initial hash {}", last_hash);
        let mut iteration = 0u64;
        loop {
            iteration += 1;
            this = f(this);
            let hash = default_hash(&this.0);
            if hash == last_hash {
                log::debug!("Stopping opt-to-convergence, iteration {}", iteration);
                return this;
            } else {
                log::debug!("Continuing opt-to-convergence, hash {}", hash);
            }
            last_hash = hash;
        }
    }
}
