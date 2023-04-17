use clap::Parser;
use clap::ValueEnum;

#[derive(Debug, PartialEq, Copy, Clone, PartialOrd, Eq, Ord, ValueEnum)]
pub enum LogLevel {
    Debug,
    Info,
    Warning,
    Error,
    Exception,
    Critical,
}

impl LogLevel {
    pub fn maybe_from_str(s: &str) -> Option<LogLevel> {
        match s {
            "debug" => Some(Self::Debug),
            "info" => Some(Self::Info),
            "warn" | "warning" => Some(Self::Warning),
            "error" => Some(Self::Error),
            "exception" => Some(Self::Exception),
            "critical" => Some(Self::Critical),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, ValueEnum)]
pub enum Quotes {
    Single,
    Double,
}

impl Quotes {
    pub fn char(&self) -> char {
        match self {
            Self::Single => '\'',
            Self::Double => '"',
        }
    }
}

#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = "Printf log formatter")]
#[command(next_line_help = true)]
pub struct Opts {
    #[arg(value_enum, short, long, default_value_t = LogLevel::Error)]
    pub log_level: LogLevel,

    #[arg(required = true)]
    pub filenames: Vec<String>,

    #[arg(value_enum, short, long, default_value_t = Quotes::Single)]
    pub quotes: Quotes,
}
