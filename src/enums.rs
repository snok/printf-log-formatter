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

#[derive(Debug, Clone, ValueEnum)]
pub enum Quotes {
    Single,
    Double
}

impl Quotes {
    pub fn char(&self) -> char {
        match self {
            Self::Single => '\'',
            Self::Double => '"'
        }
    }
}