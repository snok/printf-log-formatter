use anyhow::Result;
use clap::Parser;
use clap::__derive_refs::once_cell::sync::OnceCell;
use futures::{stream, StreamExt};
use rustpython_parser::parse_program;
use std::process::exit;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::ast::LoggerVisitor;
use crate::cli::Opts;
use crate::gen_visitor::walk_stmt;

mod ast;
mod cli;

mod gen_visitor;
mod parse_format;
mod parse_fstring;

// Define global settings, so we can reference things
// like the specified log level from anywhere in the program.
// Without this we would need to completely rewrite the
// visitor trait to pass down settings values.
static SETTINGS: OnceCell<Opts> = OnceCell::new();

#[derive(Debug)]
struct Change {
    lineno: usize,
    col_offset: usize,
    end_lineno: usize,
    end_col_offset: usize,
    new_string_content: String,
    new_string_variables: Vec<String>,
}

// Parse the program and find all the changes that need to be made
async fn get_changes(content: &str, filename: &str) -> Vec<Change> {
    let mut visitor = LoggerVisitor { changes: vec![] };
    match parse_program(content, filename) {
        Ok(program) => {
            for stmt in program {
                walk_stmt(&mut visitor, &stmt);
            }
        }
        Err(_) => {
            eprintln!("Failed to parse `{}`", filename);
            exit(1)
        }
    }
    visitor.changes
}

// Rewrite file contents
async fn fix_content(content: String, filename: &str) -> Result<(Vec<String>, bool)> {
    let changes = get_changes(&content, filename).await;

    let mut vec_content = content.split('\n').map(str::to_owned).collect::<Vec<_>>();
    let mut popped_rows = 0;

    let quotes = SETTINGS.get().unwrap().quotes.clone();
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
            for row in change.lineno..change.end_lineno - 1 {
                vec_content.remove(row - popped_rows);
                popped_rows += 1;
            }
        } else {
            vec_content[change.lineno - 1 - popped_rows]
                .replace_range(&change.col_offset..&change.end_col_offset, &new_logger);
        }
    }

    Ok((vec_content, !changes.is_empty()))
}

async fn fix_file(filename: String) -> Result<bool> {
    // Read file into string
    let mut content = String::new();
    File::open(&filename)
        .await?
        .read_to_string(&mut content)
        .await?;

    let (fixed_content, changed) = fix_content(content, &filename).await?;

    // Write updated content back to file
    if changed {
        let mut file = File::create(&filename).await?;
        let mut cleaned_content = vec![];
        for content in fixed_content {
            cleaned_content.push(content.replace('\n', "\\n"));
        }
        file.write_all(cleaned_content.join("\n").as_bytes())
            .await?
    }

    Ok(changed)
}

#[tokio::main]
async fn main() -> Result<()> {
    // Load CLI arguments
    let opts = Opts::parse();
    SETTINGS.set(opts.clone()).unwrap();

    // Fix files concurrently
    let filenames = opts.filenames.into_iter().filter(|f| f.ends_with(".py"));

    let tasks_stream =
        stream::iter(filenames).map(|filename| tokio::task::spawn(fix_file(filename)));

    let results = tasks_stream.buffer_unordered(256).collect::<Vec<_>>().await;

    // Set exit code
    let something_changed = results.into_iter().any(|result| result.unwrap().unwrap());
    exit(if something_changed { 1 } else { 0 });
}

#[cfg(test)]
mod tests {
    use crate::cli::{LogLevel, Quotes};
    use assert_panic::assert_panic;

    use super::*;

    #[derive(Debug)]
    struct TestCase {
        input: String,
        expected_output: String,
    }

    #[rustfmt::skip]
    fn format_test_cases() -> Vec<TestCase> {
        vec![
            // Simple
            TestCase { input: "logger.error('{}'.format(1))".to_string(), expected_output: "logger.error('%s', 1)".to_string() },
            // With formatting
            TestCase { input: "logger.error('{:02f}'.format(1))".to_string(), expected_output: "logger.error('%s', 1)".to_string() },
            // Named variable
            TestCase { input: "logger.error('{foo}'.format(foo=1))".to_string(), expected_output: "logger.error('%s', 1)".to_string() },
            // With formatting
            TestCase { input: "logger.error('{foo:02f}'.format(foo=1))".to_string(), expected_output: "logger.error('%s', 1)".to_string() },
            // Weird ordering
            TestCase { input: "logger.error('{x} + {} == {y}'.format(3, y=4, x=1))".to_string(), expected_output: "logger.error('%s + %s == %s', 1, 3, 4)".to_string() },
            // Packed single line
            TestCase { input: "logger.error('{}'.format(1)) or 1 + 1 == 3".to_string(), expected_output: "logger.error('%s', 1) or 1 + 1 == 3".to_string() },
            // Variable
            TestCase { input: "foo=1\nlogger.error('{}'.format(foo))".to_string(), expected_output: "foo=1\nlogger.error('%s', foo)".to_string() },
            TestCase { input: "foo=1\nlogger.error('{foo}'.format(foo=foo))".to_string(), expected_output: "foo=1\nlogger.error('%s', foo)".to_string() },
            // Multi-line
            TestCase { input: "logger.error(\n\t'{}'.format(\n\t\t1\n\t)\n)".to_string(), expected_output: "logger.error(\n\t'%s', 1\n\n)".to_string() },
            // Contained by class
            TestCase { input: "class Foo:\n\tdef bar(self):\n\t\tlogger.error('{}'.format(1))\n".to_string(), expected_output: "class Foo:\n\tdef bar(self):\n\t\tlogger.error('%s', 1)\n".to_string() },
            // Nested properties
            TestCase { input: "logger.error('{}'.format(a.b.c.d))".to_string(), expected_output: "logger.error('%s', a.b.c.d)".to_string() },
            // Call in string
            TestCase { input: "logger.error('foo {}'.format(len(bar)))".to_string(), expected_output: "logger.error('foo %s', len(bar))".to_string() },
            // Binary operation
            TestCase { input: "logger.error('{}'.format(foo + 1))".to_string(), expected_output: "logger.error('%s', foo + 1)".to_string() },
            // Newline character
            TestCase { input: "logger.error('{}\\n{}'.format(foo, bar))".to_string(), expected_output: "logger.error('%s\n%s', foo, bar)".to_string() },

        ]
    }

    #[tokio::test]
    async fn test_fix_content_format() {
        SETTINGS.get_or_init(|| Opts {
            log_level: LogLevel::Error,
            filenames: vec![],
            quotes: Quotes::Single,
        });
        for test_case in format_test_cases() {
            let (content, _changed) = fix_content(test_case.input, "filename").await.unwrap();
            let output = content.join("\n");
            assert_eq!(output, test_case.expected_output);
        }
    }

    #[test]
    fn test_fix_content_format_with_too_many_arguments_panics() {
        assert_panic!(
            tokio_test::block_on(async {
                fix_content("logger.error('{}'.format(1,2))".to_string(), "filename")
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
            TestCase { input: "logger.error(f'{1}')".to_string(), expected_output: "logger.error('%s', 1)".to_string() },
            // With formatting
            TestCase { input: "logger.error(f'{1:02f}')".to_string(), expected_output: "logger.error('%s', 1)".to_string() },
            // Variable
            TestCase { input: "logger.error(f'{foo}')".to_string(), expected_output: "logger.error('%s', foo)".to_string() },
            // Packed single line
            TestCase { input: "logger.error(f'{1}') or 1 + 1 == 3".to_string(), expected_output: "logger.error('%s', 1) or 1 + 1 == 3".to_string() },
            // Log level below default - expect no change
            TestCase { input: "logger.debug(f'{1}')".to_string(), expected_output: "logger.debug(f'{1}')".to_string() },
            TestCase { input: "logger.info(f'{1}')".to_string(), expected_output: "logger.info(f'{1}')".to_string() },
            TestCase { input: "logger.warning(f'{1}')".to_string(), expected_output: "logger.warning(f'{1}')".to_string() },
            TestCase { input: "logger.warn(f'{1}')".to_string(), expected_output: "logger.warn(f'{1}')".to_string() },
            // Nested properties
            TestCase { input: "logger.error(f'{a.b.c.d}')".to_string(), expected_output: "logger.error('%s', a.b.c.d)".to_string() },
            // Call in string
            TestCase { input: "logger.error(f'foo {len(bar)}')".to_string(), expected_output: "logger.error('foo %s', len(bar))".to_string() },
            // Binary operation
            TestCase { input: "logger.error(f'{foo + 1}')".to_string(), expected_output: "logger.error('%s', foo + 1)".to_string() },
            // Newline character
            TestCase { input: "logger.error(f'{foo}\\n{bar}')".to_string(), expected_output: "logger.error('%s\n%s', foo, bar)".to_string() },
        ]
    }

    #[tokio::test]
    async fn test_fix_content_fstring() {
        SETTINGS.get_or_init(|| Opts {
            log_level: LogLevel::Error,
            filenames: vec![],
            quotes: Quotes::Single,
        });
        for test_case in fstring_test_cases() {
            let (content, _changed) = fix_content(test_case.input, "filename").await.unwrap();
            let output = content.join("\n");
            assert_eq!(output, test_case.expected_output);
        }
    }

    #[rustfmt::skip]
    fn regression_cases() -> Vec<TestCase> {
        vec![
            // Normal f-string -- expect no change
            TestCase { input: "f'{1}'".to_string(), expected_output: "f'{1}'".to_string() },
            // Leading argument is not string -- expect no change
            TestCase { input: "messages.error(self.request, '{}'.format(foo))".to_string(), expected_output: "messages.error(self.request, '{}'.format(foo))".to_string() },
            // Line trim
            TestCase { input: "logger.error(\n\tf'{1}'\n\tf'{2}',\n\texc_info=True\n)".to_string(), expected_output: "logger.error(\n\t'%s%s', 1, 2\n,\n\texc_info=True\n)".to_string() },
            TestCase { input: "logger.exception(f'foo {bar}')".to_string(), expected_output: "logger.exception('foo %s', bar)".to_string() },
        ]
    }

    #[tokio::test]
    async fn test_for_regressions() {
        SETTINGS.get_or_init(|| Opts {
            log_level: LogLevel::Error,
            filenames: vec![],
            quotes: Quotes::Single,
        });
        for test_case in regression_cases() {
            let (content, _changed) = fix_content(test_case.input, "filename").await.unwrap();
            let output = content.join("\n");
            assert_eq!(output, test_case.expected_output);
        }
    }
}
