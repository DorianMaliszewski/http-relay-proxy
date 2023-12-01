#[derive(clap::Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct CliArguments {
    #[arg(short, long, default_value = "localhost")]
    pub listen_addr: String,
    #[arg(short, long, default_value_t = 3333)]
    pub port: i16,
    #[arg(short, long)]
    pub forward_to: String,
    #[arg(
        short = 'u',
        long,
        default_value_t = false,
        help = "Use this to update your snapshots",
        requires = "record_dir"
    )]
    pub record: bool,
    #[arg(
        short = 'd',
        long = "dir",
        help = "Directory where to store/to get records",
        default_value = ""
    )]
    pub record_dir: String,
}
