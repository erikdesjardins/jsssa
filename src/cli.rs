use structopt::StructOpt;

#[derive(StructOpt, Debug)]
#[structopt(about)]
pub struct Options {
    /// Logging verbosity (-v info, -vv debug, -vvv trace)
    #[structopt(short = "v", long = "verbose", parse(from_occurrences), global = true)]
    pub verbose: u8,

    /// Input file ("-" means stdin)
    #[structopt(default_value = "-")]
    pub input: String,

    /// Output file ("-" means stdout)
    #[structopt(short = "o", default_value = "-")]
    pub output: String,

    /// Minify when emitting JS
    #[structopt(short = "M", long = "minify")]
    pub minify: bool,

    /// Run optimizations (implies all --opt-* flags)
    #[structopt(short = "O", long = "optimize")]
    pub optimize: bool,

    /// Run optimization passes on IR
    #[structopt(long = "opt-ir")]
    pub opt_ir: bool,

    /// Inline SSA values when emitting JS
    #[structopt(long = "opt-inline-ssa")]
    pub opt_inline_ssa: bool,

    /// Run optimization passes on AST
    #[structopt(long = "opt-ast")]
    pub opt_ast: bool,

    /// Output IR instead of JS
    #[structopt(long = "emit-ir")]
    pub emit_ir: bool,
}

impl Options {
    pub fn from_args() -> Self {
        let mut this: Self = StructOpt::from_args();

        this.opt_ir |= this.optimize;
        this.opt_inline_ssa |= this.optimize;
        this.opt_ast |= this.optimize;

        this
    }
}
