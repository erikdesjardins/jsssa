use structopt::StructOpt;

#[derive(StructOpt, Debug)]
pub struct Options {
    /// Logging verbosity (-v info, -vv debug, -vvv trace)
    #[structopt(
        short = "v",
        long = "verbose",
        parse(from_occurrences),
        raw(global = "true")
    )]
    pub verbose: u8,

    /// Input file ("-" means stdin)
    #[structopt(default_value = "-")]
    pub input: String,

    /// Output file ("-" means stdout)
    #[structopt(short = "o", default_value = "-")]
    pub output: String,

    /// Run optimizations (implies all --opt-* flags)
    #[structopt(short = "O", long = "optimize")]
    pub optimize: bool,

    /// Run optimization passes on IR
    #[structopt(long = "opt-ir")]
    pub opt_ir: bool,

    /// Inline SSA values when emitting JS
    #[structopt(long = "opt-inline-ssa")]
    pub opt_inline_ssa: bool,

    /// Output IR instead of JS
    #[structopt(long = "print-ir")]
    pub print_ir: bool,
}

impl Options {
    pub fn from_args() -> Self {
        let mut this: Self = StructOpt::from_args();

        this.opt_ir |= this.optimize;
        this.opt_inline_ssa |= this.optimize;

        this
    }
}
