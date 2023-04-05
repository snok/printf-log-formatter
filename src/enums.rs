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
    pub fn values() -> &'static [Self] {
        &[
            Self::Debug,
            Self::Info,
            Self::Warning,
            Self::Error,
            Self::Exception,
            Self::Critical,
        ]
    }

    pub fn regex_str(&self) -> &str {
        match self {
            Self::Debug => "debug",
            Self::Info => "info",
            Self::Warning => "warning|warn",
            Self::Error => "error",
            Self::Exception => "exception",
            Self::Critical => "critical",
        }
    }

    pub fn regex_strings_above_log_level(level: &LogLevel) -> String {
        Self::values()
            .iter()
            .filter(|&l| l >= level)
            .map(|l| l.regex_str())
            .collect::<Vec<_>>()
            .join("|")
    }
}
