use std::sync::{Arc, Mutex};
use crate::cli::Opts;

use crate::ast::LoggerVisitor;
use crate::enums::Quotes;
use anyhow::Result;
use clap::Parser;
use futures::future::join_all;
use log::info;
use ruff_python_ast::visitor::walk_stmt;
use rustpython_parser::parse_program;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::RwLock;

mod ast;
mod enums;
mod format;
mod fstring;
mod cli;

#[derive(Debug)]
struct Change {
    lineno: usize,
    col_offset: usize,
    end_lineno: usize,
    end_col_offset: usize,
    new_string_content: String,
    new_string_variables: Vec<String>,
}

async fn get_changes(content: &str, filename: &str) -> Vec<Change> {
    let mut visitor = LoggerVisitor { changes: vec![] };
    for stmt in parse_program(content, filename).unwrap() {
        walk_stmt(&mut visitor, &stmt);
    }
    return visitor.changes;
}

async fn fix_content(content: String, filename: &str, quotes: &Quotes) -> Result<(Vec<u8>, bool)> {
    let changes = get_changes(&content, filename).await;

    let mut vec_content = content.split('\n').map(str::to_owned).collect::<Vec<_>>();
    let mut popped_rows = 0;

    for change in &changes {
        let new_logger = format!(
            "{}{}{}, {}",
            quotes.char(),
            change.new_string_content,
            quotes.char(),
            change.new_string_variables.join(", ")
        );

        if change.lineno != change.end_lineno {
            vec_content[change.lineno - 1 - popped_rows]
                .replace_range(&change.col_offset.., &new_logger);
            vec_content[change.end_lineno - 1 - popped_rows]
                .replace_range(..change.end_col_offset, "");
            // Delete any in-between rows since these will now be empty
            for row in change.lineno..change.end_lineno {
                vec_content.remove(row - popped_rows);
                popped_rows += 1;
            }
        } else {
            vec_content[change.lineno - 1 - popped_rows]
                .replace_range(&change.col_offset..&change.end_col_offset, &new_logger);
        }
    }
    Ok((
        vec_content.join("\n").as_bytes().to_owned(),
        !changes.is_empty(),
    ))
}

async fn fix_file(filename: String, settings: Arc<RwLock<Opts>>) -> Result<bool> {
    info!("Processing file {}", filename);

    // Read file into string
    let mut content = String::new();
    File::open(&filename)
        .await?
        .read_to_string(&mut content)
        .await?;

    let unlocked_settings = settings.read().await;
    let (fixed_content, changed) = fix_content(content, &filename, &unlocked_settings.quotes).await?;

    // Write updated content back to file
    File::create(&filename)
        .await?
        .write_all(&fixed_content)
        .await?;

    Ok(changed)
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    // Load CLI arguments
    let opts = Opts::parse();
    let settings = Arc::new(RwLock::new(opts.clone()));

    // Fix files concurrently
    let mut tasks = vec![];
    for filename in opts.filenames {
        if filename.ends_with(".py") {
            tasks.push(tokio::task::spawn(fix_file(filename.clone(), Arc::clone(&settings))));
        }
    }
    let results = join_all(tasks).await;

    // Set exit code
    let something_changed = results.into_iter().any(|result| result.unwrap().unwrap());
    std::process::exit(if something_changed { 1 } else { 0 });
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_panic::assert_panic;

    #[derive(Debug)]
    struct TestCase {
        input: String,
        expected_output: String,
    }

    #[rustfmt::skip]
    fn format_test_cases() -> Vec<TestCase> {
        vec![
            // Simple
            TestCase { input: "logger.info('{}'.format(1))".to_string(), expected_output: "logger.info('%s', 1)".to_string() },
            // With formatting
            TestCase { input: "logger.info('{:02f}'.format(1))".to_string(), expected_output: "logger.info('%s', 1)".to_string() },
            // Named variable
            TestCase { input: "logger.info('{foo}'.format(foo=1))".to_string(), expected_output: "logger.info('%s', 1)".to_string() },
            // With formatting
            TestCase { input: "logger.info('{foo:02f}'.format(foo=1))".to_string(), expected_output: "logger.info('%s', 1)".to_string() },
            // Weird ordering
            TestCase { input: "logger.info('{x} + {} == {y}'.format(3, y=4, x=1))".to_string(), expected_output: "logger.info('%s + %s == %s', 1, 3, 4)".to_string() },
            // Packed single line
            TestCase { input: "logger.info('{}'.format(1)) or 1 + 1 == 3".to_string(), expected_output: "logger.info('%s', 1) or 1 + 1 == 3".to_string() },
            // Variable
            TestCase { input: "foo=1\nlogger.info('{}'.format(foo))".to_string(), expected_output: "foo=1\nlogger.info('%s', foo)".to_string() },
            TestCase { input: "foo=1\nlogger.info('{foo}'.format(foo=foo))".to_string(), expected_output: "foo=1\nlogger.info('%s', foo)".to_string() },
            // Multi-line
            TestCase { input: "logger.info(\n\t'{}'.format(\n\t\t1\n\t)\n)".to_string(), expected_output: "logger.info(\n\t'%s', 1\n)".to_string() },
            // Contained by class
            TestCase { input: "class Foo:\n\tdef bar(self):\n\t\tlogger.info('{}'.format(1))\n".to_string(), expected_output: "class Foo:\n\tdef bar(self):\n\t\tlogger.info('%s', 1)\n".to_string() },
        ]
    }

    #[tokio::test]
    async fn test_fix_content__format() {
        for test_case in format_test_cases() {
            let (content, changed) = fix_content(test_case.input, "filename", &Quotes::Single)
                .await
                .unwrap();
            assert_eq!(String::from_utf8_lossy(&content), test_case.expected_output);
        }
    }

    #[test]
    fn test_fix_content__format_with_too_many_arguments_panics() {
        assert_panic!(
            tokio_test::block_on(async {
                fix_content(
                    "logger.info('{}'.format(1,2))".to_string(),
                    "filename",
                    &Quotes::Single,
                )
                .await
                .unwrap();
            }),
            String,
            "Found excess argument `2` in logger. Run with RUST_LOG=debug for verbose logging.",
        );
    }

    #[rustfmt::skip]
    fn fstring_test_cases() -> Vec<TestCase> {
        vec![
            // Simple
            TestCase { input: "logger.info(f'{1}')".to_string(), expected_output: "logger.info('%s', 1)".to_string() },
            // With formatting
            TestCase { input: "logger.info(f'{1:02f}')".to_string(), expected_output: "logger.info('%s', 1)".to_string() },
            // Variable
            TestCase { input: "logger.info(f'{foo}')".to_string(), expected_output: "logger.info('%s', foo)".to_string() },
            // Packed single line
            TestCase { input: "logger.info(f'{1}') or 1 + 1 == 3".to_string(), expected_output: "logger.info('%s', 1) or 1 + 1 == 3".to_string() },
        ]
    }

    #[tokio::test]
    async fn test_fix_content__fstring() {
        for test_case in fstring_test_cases() {
            let (content, changed) = fix_content(test_case.input, "filename", &Quotes::Single)
                .await
                .unwrap();
            assert_eq!(String::from_utf8_lossy(&content), test_case.expected_output);
        }
    }
}
