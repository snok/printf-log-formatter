use crate::enums::{LogLevel, Quotes};
use clap::Parser;

#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = "Printf log formatter")]
#[command(next_line_help = true)]
pub struct Opts {
    #[arg(value_enum, short, long, default_value_t = LogLevel::Error)]
    pub log_level: LogLevel,

    #[arg(short, long, value_delimiter = ',')]
    pub filenames: Vec<String>,

    #[arg(value_enum, short, long, default_value_t = Quotes::Single)]
    pub quotes: Quotes,
}
