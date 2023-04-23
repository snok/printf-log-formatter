use crate::gen_visitor::walk_stmt;
use crate::visitor::LoggerVisitor;
use crate::{Change, THREAD_LOCAL_STATE};
use anyhow::Result;
use rustpython_parser::parse_program;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

pub(crate) async fn fix_file() -> Result<bool> {
    // Load thread-local state
    let state = THREAD_LOCAL_STATE.with(Clone::clone);

    // Find changes needing to be made
    let changes = get_changes(&state.content, &state.filename);

    // Write changes to string content
    let (content, content_changed) = change_content(&state.content, changes);

    // Write updated content back to file
    if content_changed {
        let mut file = File::create(&state.filename).await?;
        let cleaned_content = content
            .iter()
            .map(|line| line.replace('\n', "\\n"))
            .collect::<Vec<String>>()
            .join("\n");
        file.write_all(cleaned_content.as_bytes()).await?;
    }

    Ok(content_changed)
}

/// Parse the program and find all the changes that need to be made
pub fn get_changes(content: &str, filename: &str) -> Vec<Change> {
    let mut visitor = LoggerVisitor { changes: vec![] };

    if let Ok(program) = parse_program(content, filename) {
        program
            .iter()
            .for_each(|stmt| walk_stmt(&mut visitor, stmt));
    } else {
        // If we're unable to parse a file, we just return no changes
        eprintln!("Failed to parse `{filename}`");
    }

    visitor.changes
}

/// Mutate file content, according to changes found
fn change_content(content: &str, changes: Vec<Change>) -> (Vec<String>, bool) {
    let mut vec_content = content.split('\n').map(str::to_owned).collect::<Vec<_>>();
    let mut popped_rows = 0;

    for change in &changes {
        let mut new_logger = format!(
            "{}{}{}, {}",
            change.quote,
            change.new_string_content,
            change.quote,
            change.new_string_variables.join(", ")
        );

        // If the logger starts and end on the same line, then we can just replace the old line with the new one
        if change.lineno == change.end_lineno {
            vec_content[change.lineno - 1 - popped_rows]
                .replace_range(&change.col_offset..&change.end_col_offset, &new_logger);
        } else {
            let range = change.lineno - popped_rows..change.end_lineno - popped_rows;

            // Replace excess lines - we'll add the new logger on the first line
            let removed_lines = vec_content.drain(range);

            // Add trailing comma if needed
            if let Some(last_item) = removed_lines.last() {
                if last_item.ends_with(',') {
                    new_logger.push(',');
                }
            }

            // Write new logger to file
            vec_content[change.lineno - 1 - popped_rows]
                .replace_range(&change.col_offset.., &new_logger);

            popped_rows += change.end_lineno - change.lineno;
        }
    }

    (vec_content, !changes.is_empty())
}

#[cfg(test)]
mod tests {
    use assert_panic::assert_panic;

    use crate::cli::{LogLevel, Opts};
    use crate::{ThreadLocal, SETTINGS};

    use super::*;

    #[derive(Debug)]
    struct TestCase {
        input: String,
        expected_output: String,
    }

    async fn run(test_case: TestCase) {
        let (content, _changed) = THREAD_LOCAL_STATE
            .scope(
                ThreadLocal {
                    filename: "test.py".to_string(),
                    content: test_case.input,
                },
                async move {
                    let state = THREAD_LOCAL_STATE.with(Clone::clone);
                    let changes = get_changes(&state.content, &state.filename);
                    change_content(&state.content, changes)
                },
            )
            .await;
        let output = content.join("\n");
        assert_eq!(output, test_case.expected_output);
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
            TestCase { input: "logger.error(\n\t'{}'.format(\n\t\t1\n\t)\n)".to_string(), expected_output: "logger.error(\n\t'%s', 1\n)".to_string() },
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
            // Call
            TestCase { input: "logging.error('Error parsing event file: {}'.format(e.errors()))".to_string(), expected_output: "logging.error('Error parsing event file: %s', e.errors())".to_string() },
            // Index
            TestCase { input: "logger.error('{}'.format(ret[\"id\"]))".to_string(), expected_output: "logger.error('%s', ret['id'])".to_string() },
        ]
    }

    #[tokio::test]
    async fn test_change_content_format() {
        SETTINGS.get_or_init(|| Opts {
            log_level: LogLevel::Error,
            filenames: vec![],
        });
        for test_case in format_test_cases() {
            run(test_case).await;
        }
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
            // Multi-line
            TestCase { input: "logger.error(\n\tf'foo {bar} '\n\tf'baz %s',\n\te,\n\texc_info=True\n)".to_string(), expected_output: "logger.error(\n\t'foo %s baz %s', bar,\n\te,\n\texc_info=True\n)".to_string() },
            // Call inside f-string
            TestCase { input: "logging.error(f'Error parsing event file: {e.errors()}')".to_string(), expected_output: "logging.error('Error parsing event file: %s', e.errors())".to_string() },
            // Index inside f-string
            TestCase { input: "logger.error(f'{ret[\"id\"]}')".to_string(), expected_output: "logger.error('%s', ret['id'])".to_string() },
            // List comprehension
            TestCase { input: "logger.error(f'{[str(e) for errors in all_errors for e in errors]}')".to_string(), expected_output: "logger.error('%s', [str(e) for errors in all_errors for e in errors])".to_string() },
            // Dict comprehension
            TestCase { input: "logger.error(f'{ {\"foo\": str(e) for errors in all_errors for e in errors} }')".to_string(), expected_output: "logger.error('%s', {'foo': str(e) for errors in all_errors for e in errors})".to_string() },
            // Call containing list comprehension
            TestCase { input: "logger.error(f'{\", \".join([str(e) for e in errors for errors in all_errors])}')".to_string(), expected_output: "logger.error('%s', ', '.join([str(e) for e in errors for errors in all_errors]))".to_string() },
            // Generator
            TestCase { input: "logger.exception(f'{\", \".join(b for b in bs)}')".to_string(), expected_output: "logger.exception('%s', ', '.join([b for b in bs]))".to_string() },
            // Named args in calls
            TestCase { input: "logger.error(f'{something(1, x=2, y=4)}')".to_string(), expected_output: "logger.error('%s', something(1, x=2, y=4))".to_string() },
        ]
    }

    #[tokio::test]
    async fn test_change_content_fstring() {
        SETTINGS.get_or_init(|| Opts {
            log_level: LogLevel::Error,
            filenames: vec![],
        });
        for test_case in fstring_test_cases() {
            run(test_case).await;
        }
    }

    #[test]
    fn test_change_content_format_with_too_many_arguments_panics() {
        SETTINGS.get_or_init(|| Opts {
            log_level: LogLevel::Error,
            filenames: vec![],
        });
        assert_panic!(
            tokio_test::block_on(
                async {
                    THREAD_LOCAL_STATE.scope(
                        ThreadLocal { filename: "test.py".to_string(), content: "logger.error('{}'.format(1,2))".to_string() },
                        async move {
                            let state = THREAD_LOCAL_STATE.with(Clone::clone);
                            let changes = get_changes(&state.content, &state.filename);
                            change_content(&state.content, changes);
                        }
                    ).await;
                }
            ),
            String,
            "File `test.py` contains a str.format call with too many arguments for the string. Argument is `2`. Please fix before proceeding.",
        );
    }

    #[rustfmt::skip]
    fn regression_cases() -> Vec<TestCase> {
        vec![
            // Normal f-string -- expect no change
            TestCase { input: "f'{1}'".to_string(), expected_output: "f'{1}'".to_string() },
            // Leading argument is not string -- expect no change
            TestCase { input: "messages.error(self.request, '{}'.format(foo))".to_string(), expected_output: "messages.error(self.request, '{}'.format(foo))".to_string() },
            // Line trim
            TestCase { input: "logger.error(\n\tf'{1}'\n\tf'{2}',\n\texc_info=True\n)".to_string(), expected_output: "logger.error(\n\t'%s%s', 1, 2,\n\texc_info=True\n)".to_string() },
            TestCase { input: "logger.exception(f'foo {bar}')".to_string(), expected_output: "logger.exception('foo %s', bar)".to_string() },
            TestCase { input: "warnings.error(f'{1}')".to_string(), expected_output: "warnings.error(f'{1}')".to_string() },
            // Quotes are set correctly
            TestCase { input: "logger.error(f\"{1}\")\nlogger.error(f'{2}')".to_string(), expected_output: "logger.error(\"%s\", 1)\nlogger.error('%s', 2)".to_string() },
        ]
    }

    #[tokio::test]
    async fn test_for_regressions() {
        SETTINGS.get_or_init(|| Opts {
            log_level: LogLevel::Error,
            filenames: vec![],
        });
        for test_case in regression_cases() {
            run(test_case).await;
        }
    }
}
