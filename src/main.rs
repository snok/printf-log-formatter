use crate::options::Opts;
use anyhow::Result;
use clap::Parser;
use futures::future::join_all;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

mod enums;
mod options;
mod utils;

use crate::utils::{construct_f_string_regex, replace_last_char_with_str};
use regex::Regex;

const CURLY_BRACE_PATTERN: &str = r"\{(?P<name>\w+)\}";

fn fix_f_strings(mut content: String, f_string_regex_pattern: &str) -> Result<(bool, String)> {
    while let Some(f_string_logger_captures) =
        Regex::new(&f_string_regex_pattern).unwrap().captures(&content)
    {
        let logger = f_string_logger_captures
            .get(0)
            .unwrap()
            .as_str()
            .to_string();
        let old_logger = logger.clone();
        let f_string = f_string_logger_captures.get(2).unwrap().as_str();

        let re = Regex::new(CURLY_BRACE_PATTERN).unwrap();

        // Capture the f-string arguments
        let mut names = vec![];
        let result = re.replace_all(&f_string, |caps: &regex::Captures<'_>| {
            // Access the captured name using the name "name" (same as the capture group name)
            names.push(caps["name"].to_owned());
            "%s"
        });

        if names.is_empty() {
            return Ok((false, content));
        }

        println!("{}", logger);

        let logger = logger.replace(
            &format!("f\"{}\"", f_string),
            &format!("\"{}\"", result),
        );
        let logger = replace_last_char_with_str(logger, &format!(", {})", names.join(", ")));

        println!("Fixing f-string");

        // Replace old logger with updated logger in the content
        let updated_content = content.replace(&old_logger, &logger);
        content = updated_content;
    }
    Ok((true, content))
}


async fn fix_file(filename: String, f_string_regex_pattern: String) -> Result<bool> {
    // Read file into string
    let mut content = String::new();
    File::open(&filename)
        .await?
        .read_to_string(&mut content)
        .await?;

    let (f_strings_changed, content) = fix_f_strings(content, &f_string_regex_pattern)?;

    // Write updated content back to file
    File::create(&filename)
        .await?
        .write_all(content.as_bytes())
        .await?;

    Ok(f_strings_changed)  // or ...
}

#[tokio::main]
async fn main() -> Result<()> {
    // Parse CLI arguments
    let opts = Opts::parse();

    // Define regex patterns
    let f_string_regex_pattern = construct_f_string_regex(&opts.log_level);

    // Fix all files concurrently
    let mut tasks = vec![];
    for filename in opts.filenames {
        if filename.ends_with(".py") {
            tasks.push(tokio::task::spawn(fix_file(
                filename,
                f_string_regex_pattern.clone(),
            )));
        }
    }
    let results = join_all(tasks).await;

    let mut something_changed = false;
    for result in results {
        if result?? == true {
            something_changed = true;
            break;
        }
    }

    std::process::exit(if something_changed != false { 1 } else { 0 });
}
