use anyhow::Result;
use tokio::fs::{File};
use tokio::io::{AsyncReadExt};
use crate::options::Opts;
use clap::{Parser};
use futures::future::join_all;
use tokio::join;
use rustpython_parser::{parser, ast};
use crate::enums::LogLevel;

mod enums;
mod options;

const WORD: &str = "logger.";
const WORD_LENGTH: usize = 6;

enum SearchingFor {
    FirstPart,
    LogLevel,
    String,
    Parentheses,
}

enum StringFormat {
    FString,
    Format,
    NamedFormat,
    None,
}

enum Parentheses {
    Left,
    Right
}

impl Parentheses {
    fn ch(&self) -> char {
        match self {
            Self::Left => '(',
            Self::Right => ')'
        }
    }
}


async fn lol(filename: String) -> Result<()> {
    let mut file = File::open(filename).await?;
    let mut contents = String::new();
    file.read_to_string(&mut contents).await?;

    // Start by looking for l, then o, then g, until we get to "logger."
    let mut next = 0;
    let mut searching_for = SearchingFor::FirstPart;
    let mut format = StringFormat::None;
    let mut log_level_chars = String::new();
    let mut parentheses = Parentheses::Left;

    let previous_character_was_escape = false;
    let mut quote_count = 0;

    let mut string_chars = String::new();

    // TODO: Ignore whitespace for all modes except string

    for (index, ch) in contents.chars().enumerate() {
        match searching_for {
            // Keep traversing until we get to `logger.` then switch mode
            SearchingFor::FirstPart => {
                let _next_letter: char = WORD.chars().nth(next).unwrap();
                if ch == _next_letter {
                    // Switch mode
                    if next == WORD_LENGTH {
                        println!("Found the whole word, switching to search for log level");
                        searching_for = SearchingFor::LogLevel;
                        next = 0;
                        continue;
                        // Keep looking
                    } else {
                        println!("Character matched {}", next);
                        next += 1;
                        continue;
                    }
                } else {
                    println!("Reset");
                    next = 0;
                    continue
                };
            }

            // Keep traversing until we finish one of the log levels
            SearchingFor::LogLevel => {
                log_level_chars.push(ch);

                match LogLevel::any_options_starts_with(&log_level_chars) {
                    Err(_) => {
                        println!("Invalid logger name: `{}`. Resetting", log_level_chars);
                        searching_for = SearchingFor::FirstPart;
                        log_level_chars = String::new();
                        continue
                    },
                    Ok(t) => match t {
                        Some(t) => {
                            println!("Valid loger name `{}`. Switching modes", log_level_chars);
                            searching_for = SearchingFor::Parentheses;
                            log_level_chars = String::new();
                            continue
                        },
                        None => continue
                    }
                }
            }
            SearchingFor::Parentheses => {
                match parentheses {
                    Parentheses::Left => {
                        if ch == '(' {
                            println!("Found left paren");
                            searching_for = SearchingFor::String;
                            parentheses = Parentheses::Right;
                            continue
                        } else {
                            println!("Didn't find parentheses. Resetting");
                            searching_for = SearchingFor::FirstPart;
                            continue
                        }
                    }
                    Parentheses::Right => {
                        if ch == ')' {
                            println!("Found right paren");
                            searching_for = SearchingFor::String;
                            parentheses = Parentheses::Left;
                            continue
                        } else {
                            println!("Didn't find parentheses. Resetting");
                            searching_for = SearchingFor::FirstPart;
                            continue
                        }
                    }
                }
            }
            // Keep traversing until we find a { or whatever
            SearchingFor::String => {

                match format {
                    StringFormat::None => {
                        if string_chars.is_empty() && ch == 'f' {
                            println!("It's an f-string!");
                            format = StringFormat::FString;
                            continue
                        }

                        if string_chars.is_empty() && ch == '"' {
                            quote_count += 1;
                        }

                    },
                    StringFormat::FString {

                    }
                }


                string_chars.push(ch);


                // f-string start case
                // start quotes
                // end quotes
                // variables after if not f-string

                println!("string: {}", string_chars);
            }
        }
    };

    // contents.insert_str(0, "\n");
    // file.seek(SeekFrom::Start(0)).await?;
    // file.write_all(contents.as_bytes()).await?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let opts = Opts::parse();
    println!("{:?}, {:?}", opts.filenames, opts.log_level);


    let mut tasks = vec![];
    for filename in opts.filenames {
        if filename.ends_with(".py") {
            tasks.push(tokio::task::spawn(lol(filename)));
        }
    }

    join_all(tasks).await;

    let exit_code = 1;
    std::process::exit(if exit_code != 0 { 1 } else { 0 });
}
