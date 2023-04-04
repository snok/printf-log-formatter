use std::str::FromStr;
use clap::CommandFactory;
use clap::error::{Error, ErrorKind};
use clap::{ValueEnum};
use crate::options::Opts;
use anyhow::Result;

#[derive(Debug, PartialEq, Clone, ValueEnum)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warning,
    Error,
    Exception,
    Critical,
}


impl LogLevel {
    fn value(&self) -> i32 {
        match self {
            Self::Trace => 0,
            Self::Debug => 1,
            Self::Info => 2,
            Self::Warning => 3,
            Self::Error => 4,
            Self::Exception => 4,
            Self::Critical => 4,
        }
    }

    pub fn any_options_starts_with(s: &str) -> Result<Option<Self>> {
        match s {
            // Trace
            "t" => Ok(None),
            "tr" => Ok(None),
            "tra" => Ok(None),
            "trac" => Ok(None),
            "trace" => Ok(Some(Self::Trace)),
            // Debug
            "d" => Ok(None),
            "de" => Ok(None),
            "deb" => Ok(None),
            "debu" => Ok(None),
            "debug" => Ok(Some(Self::Debug)),
            // Info
            "i" => Ok(None),
            "in" => Ok(None),
            "inf" => Ok(None),
            "info" => Ok(Some(Self::Info)),
            // Warning
            "w" => Ok(None),
            "wa" => Ok(None),
            "war" => Ok(None),
            "warn" => Ok(None),
            "warni" => Ok(None),
            "warnin" => Ok(None),
            "warning" => Ok(Some(Self::Warning)),
            // Error
            "e" => Ok(None),
            "er" => Ok(None),
            "err" => Ok(None),
            "erro" => Ok(None),
            "error" => Ok(Some(Self::Error)),
            // Exception
            "ex" => Ok(None),
            "exc" => Ok(None),
            "exce" => Ok(None),
            "excep" => Ok(None),
            "except" => Ok(None),
            "excepti" => Ok(None),
            "exceptio" => Ok(None),
            "exception" => Ok(Some(Self::Error)),
            // Critical
            "c" => Ok(None),
            "cr" => Ok(None),
            "cri" => Ok(None),
            "crit" => Ok(None),
            "criti" => Ok(None),
            "critic" => Ok(None),
            "critica" => Ok(None),
            "critical" => Ok(Some(Self::Error)),
            _ => Err(anyhow::Error::msg("No match"))
        }
    }
}


impl FromStr for LogLevel {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "trace" => Ok(Self::Trace),
            "debug" => Ok(Self::Debug),
            "info" => Ok(Self::Info),
            "warning" => Ok(Self::Warning),
            "warn" => Ok(Self::Warning),
            "error" => Ok(Self::Error),
            "exception" => Ok(Self::Exception),
            "critical" => Ok(Self::Critical),
            _ => Err(Opts::command().error(
                ErrorKind::ArgumentConflict,
                format!("`{}` is not a valid log level", s),
            )),
        }
    }
}