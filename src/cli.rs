#[derive(clap::Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct CliArguments {
    #[arg(
        short = 'u',
        long,
        default_value_t = false,
        help = "Use this flag to update your snapshots",
    )]
    pub record: bool,
}
