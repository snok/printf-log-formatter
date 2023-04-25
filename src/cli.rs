use anyhow::bail;
use clap::Parser;
use clap::ValueEnum;

use crate::THREAD_LOCAL_STATE;
use anyhow::Result;

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

pub fn emit_error(reason: &str) {
    eprintln!(
        "{reason}. Please open an issue at https://github.com/snok/printf-log-formatter/issues/new"
    );
}

pub fn get_char(string: &str, col_offset: usize) -> Result<char> {
    if let Some(c) = string.chars().nth(col_offset) {
        return match c {
            '\'' => Ok('\''),
            '"' => Ok('"'),
            'f' => Ok(string.chars().nth(col_offset + 1).unwrap()),
            _ => bail!("noo"),
        };
    }
    emit_error("Failed to infer quote character");
    bail!("Failed to inherit quotes")
}

pub fn get_quotes(lineno: usize, col_offset: usize) -> Result<char> {
    let content = THREAD_LOCAL_STATE.with(|tl| tl.content.clone());
    let vec_content = content.split('\n').map(str::to_owned).collect::<Vec<_>>();

    if let Ok(t) = get_char(&vec_content[lineno - 1], col_offset) {
        Ok(t)
    } else {
        let filename = THREAD_LOCAL_STATE.with(|tl| tl.filename.clone());
        emit_error(&format!(
            "Failed to infer quote from `{filename}` line {lineno}"
        ));
        bail!("Failed to infer quote")
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
}
