use anyhow::Result;
use clap::Parser;
use clap::__derive_refs::once_cell::sync::OnceCell;
use futures::{stream, StreamExt};
use std::process::exit;
use tokio::fs;

use crate::cli::Opts;
use crate::fix_file::fix_file;

mod cli;
mod fix_file;
mod gen_visitor;
mod parse_format;
mod parse_fstring;
mod visitor;

// Since a lot of the formatter logic happens on the other side of the Visitor
// trait passing down filenames and content is tricky.
// To not have to pass everything down the stack, we rely on global values.
// Settings are the same for all files, so we make this global, while
// filename and content are specific to each tokio task, so they're thread-local.

static SETTINGS: OnceCell<Opts> = OnceCell::new();

#[derive(Debug, Clone)]
struct ThreadLocal {
    filename: String,
    content: String,
}

tokio::task_local! {
    static THREAD_LOCAL_STATE: ThreadLocal;
}

#[derive(Debug)]
pub struct Change {
    lineno: usize,
    col_offset: usize,
    end_lineno: usize,
    end_col_offset: usize,
    new_string_content: String,
    new_string_variables: Vec<String>,
    quote: char,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Load arguments
    let opts = Opts::parse();
    SETTINGS.set(opts.clone()).unwrap();

    // Filter down filenames to Python files only
    let filenames = opts.filenames.into_iter().filter(|f| {
        std::path::Path::new(f)
            .extension()
            .map_or(false, |ext| ext.eq_ignore_ascii_case("py"))
    });

    // Create a tokio task per file
    let tasks_stream = stream::iter(filenames).map(|filename| async move {
        let content = fs::read_to_string(&filename).await?;
        THREAD_LOCAL_STATE
            .scope(ThreadLocal { filename, content }, fix_file())
            .await
    });

    // Run tasks concurrently
    // *Added a limit of 256 to avoid `too many open files` errors
    let results = tasks_stream.buffer_unordered(256).collect::<Vec<_>>().await;

    // Set exit code; 1 if something was changed, otherwise 0
    let something_changed = results.into_iter().any(std::result::Result::unwrap);
    exit(i32::from(something_changed));
}
