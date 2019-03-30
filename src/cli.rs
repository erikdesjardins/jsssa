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

    /// Run optimizations
    #[structopt(short = "O", long = "optimize")]
    pub optimize: bool,

    /// Output IR instead of JS
    #[structopt(long = "print-ir")]
    pub print_ir: bool,
}
