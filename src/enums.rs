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
