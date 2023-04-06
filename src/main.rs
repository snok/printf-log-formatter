use crate::options::Opts;
use crate::visitor::Visitor;
use anyhow::Result;
use clap::Parser;
use futures::future::join_all;
use log::{debug, info};
use regex::Regex;
use rustpython_parser::ast::{Constant, Location};
use rustpython_parser::ast::{Expr, ExprKind, Keyword, KeywordData};
use rustpython_parser::parse_expression;
use tokio::fs::File;
use tokio::io::AsyncReadExt;

mod enums;
mod options;
mod visitor;

struct LoggerVisitor;

#[derive(Debug)]
struct Change {
    start_row: usize,
    start_col: usize,
    end_row: usize,
    end_col: usize,
    new_logger: String,
}

#[derive(Debug)]
struct NamedArg {
    key: String,
    value: Constant,
}

#[derive(Debug)]
struct Arg {
    value: Constant,
}

fn constant_to_string(constant: Constant) -> String {
    match constant {
        Constant::None => "None".to_string(),
        Constant::Bool(value) => value.to_string(),
        Constant::Str(value) => value,
        Constant::Bytes(value) => format!("b\"{}\"", String::from_utf8_lossy(&*value)),
        Constant::Int(value) => value.to_string(),
        Constant::Float(value) => value.to_string(),
        Constant::Ellipsis => "...".to_string(),
        Constant::Complex { real, imag } => todo!(),
        Constant::Tuple(value) => {
            let items = value
                .iter()
                .map(|item| constant_to_string(item.clone()))
                .collect::<Vec<_>>()
                .join(", ");
            format!("({})", items)
        }
    }
}

fn get_named_arg_index_start_end(re: &Regex, string: &str, key: &str) -> (usize, usize) {
    for (i, cap) in re.captures_iter(string).enumerate() {
        let capture = cap.get(0).unwrap();
        if cap.get(1).unwrap().as_str() == key {
            return (capture.start(), capture.end());
        }
    }
    unreachable!()
}

fn get_named_arg_indexes(re: &Regex, string: &str, key: &str) -> Vec<usize> {
    let mut matches = vec![];
    for (i, cap) in re.captures_iter(string).enumerate() {
        if cap.get(1).unwrap().as_str() == key {
            matches.push(i)
        }
    }
    matches
}

// Look for logger.error("string {var}.format(var=var)) syntax
fn check_for_format(func: &Box<Expr>, args: &Vec<Expr>, keywords: &Vec<Keyword>) -> Option<String> {
    let mut format_named_args: Vec<NamedArg> = vec![]; // .format(var=var) or .format(var)
    let mut format_args: Vec<Arg> = vec![]; // .format(var=var) or .format(var)
    let mut string = String::new();

    if let ExprKind::Attribute { value, attr, ctx } = &func.node {
        if attr != "format" {
            info!("Function call was not for .format; returning early");
            return None;
        }
        if let ExprKind::Constant { value, kind } = &value.node {
            if let Constant::Str(s) = value {
                info!("Found string {}", s);
                string.push_str(s);
            }
        }
    }

    for keyword in keywords {
        if let KeywordData { arg, value } = &keyword.node {
            if let ExprKind::Constant { value, kind } = &value.node {
                if let Some(arg) = arg {
                    info!("Found named argument {} with value {:?}", arg, value);
                    format_named_args.push(NamedArg {
                        key: arg.to_string(),
                        value: value.clone(),
                    })
                } else {
                    info!("Found unnamed argument with value {:?}", value);
                    format_args.push(Arg {
                        value: value.clone(),
                    })
                }
            }
        }
    }

    for arg in args {
        if let ExprKind::Constant { value, kind } = &arg.node {
            info!("Found unnamed argument with value {:?}", value);
            format_args.push(Arg {
                value: value.clone(),
            })
        }
    }

    debug!("We have string '{}'", string);
    debug!("We have the named arguments '{:?}'", format_named_args);
    debug!("And the unnamed arguments '{:?}'", format_args);

    let curly_brace_re = Regex::new(r"\{.*?\}").unwrap();
    let total_number_of_arguments = curly_brace_re.find_iter(&string).count();

    debug!("count: {}", total_number_of_arguments);

    let mut new_string = string.clone();
    let mut ordered_arguments: Vec<Option<String>> = vec![None; total_number_of_arguments];

    let re = Regex::new(r"\{([^{}:]*)(?::[^{}]*)?\}").unwrap();

    /// Replace all keyword arguments with %s and insert each of their values
    /// into the ordered_arguments vector, in the right order. Something to be
    /// aware of is that this is valid Python syntax:
    ///
    ///   "{x:02f} + {x:03f} - {x} == {y}".format(x=2, y=2)
    ///
    /// so we have to handle the potential of multiple indices for one keyword arg,
    /// and we need to separate the variable name from the contents of the curly brace.
    for keyword_arg in format_named_args {
        // Get all indexes for the given keyword argument key
        let indexes = get_named_arg_indexes(&re, &string, &keyword_arg.key);

        // Convert Rust type to a string value
        let str_value = constant_to_string(keyword_arg.value);

        // Push each string value to the right index
        // We might push index 1, then 3; not 0,1,2.
        for index in indexes {
            let (start, end) = get_named_arg_index_start_end(&re, &new_string, &keyword_arg.key);

            // Insert value into the right index for printf-style formatting later
            ordered_arguments[index] = Some(str_value.clone());

            // Replace the curly brace from the string
            new_string.replace_range(start..end, "%s");
        }
    }

    debug!("After handling kwargs the new string is {}", new_string);

    // Args are captured in order, so we should be able to just fill in the missing ordered arguments.
    // One nice assumption we can make here is that each arg is unique and only appears once.

    for arg in format_args {
        // Replace a {} with %s
        new_string = new_string.replacen("{}", "%s", 1);

        // Convert Rust type to a string value
        let str_value = constant_to_string(arg.value);

        match ordered_arguments.iter().position(|x| x.is_none()) {
            Some(index) => ordered_arguments[index] = Some(str_value),
            None => {
                // This will happen for syntax like
                //  logger.info("{}".format(1,2))
                // where there are more arguments passed than mapped to.
                // We could ignore these cases, but if we silently fixed them
                // that might cause other problems for the user ¯\_(ツ)_/¯
                panic!("Found excess argument `{str_value}` in logger. Run with RUST_LOG=debug for verbose logging.")
            }
        }
    }
    let string_addon = ordered_arguments
        .iter()
        .map(|s| s.clone().unwrap())
        .collect::<Vec<_>>()
        .join(", ");

    debug!("After handling args the new string is {}", new_string);
    Some(format!("(\"{}\", {})", new_string, string_addon))
}

impl<'a> Visitor<'a> for LoggerVisitor {}

// fn go_for_a_walkie<'a, V: Visitor<'a> + ?Sized>(visitor: &mut V, expr: &'a Expr) -> Vec<Change> {
//     let mut changes = vec![];
//     match &expr.node {
//         ExprKind::Call {
//             func,
//             args,
//             keywords,
//         } => {
//             if let Some(new_logger) = check_for_format(func, args, keywords) {
//                 changes.push(Change {
//                     start_row: expr.location.row(),
//                     start_col: expr.location.column(),
//                     end_row: expr.end_location.unwrap().row(),
//                     end_col: expr.end_location.unwrap().column(),
//                     new_logger,
//                 });
//             }
//         }
//         ExprKind::JoinedStr { values } => {
//             for value in values {
//                 match &value.node {
//                     ExprKind::Constant { value, kind } => {
//                         println!("constant: {:?} of kind {:?}", value, kind);
//                     }
//                     ExprKind::FormattedValue {
//                         value,
//                         conversion,
//                         format_spec,
//                     } => {
//                         println!(
//                             "fmtval: {:?} conversion {:?} and spec {:?}",
//                             value, conversion, format_spec
//                         );
//                     }
//                     _ => unreachable!(),
//                 }
//             }
//         }
//         _ => (),
//     }
//     changes
// }

impl<'a> Visitor<'a> for LoggerVisitor {
    fn visit_expr(&mut self, expr: &'a Expr) -> Vec<Change> {
        let mut changes = vec![];
        match &expr.node {
            ExprKind::Call {
                func,
                args,
                keywords,
            } => {
                if let Some(new_logger) = check_for_format(func, args, keywords) {
                    changes.push(Change {
                        start_row: expr.location.row(),
                        start_col: expr.location.column(),
                        end_row: expr.end_location.unwrap().row(),
                        end_col: expr.end_location.unwrap().column(),
                        new_logger,
                    });
                }
            }
            ExprKind::JoinedStr { values } => {
                for value in values {
                    match &value.node {
                        ExprKind::Constant { value, kind } => {
                            println!("constant: {:?} of kind {:?}", value, kind);
                        }
                        ExprKind::FormattedValue {
                            value,
                            conversion,
                            format_spec,
                        } => {
                            println!(
                                "fmtval: {:?} conversion {:?} and spec {:?}",
                                value, conversion, format_spec
                            );
                        }
                        _ => unreachable!(),
                    }
                }
            }
            _ => (),
        }
        changes
    }
}

async fn fix_file(filename: String) -> Result<bool> {
    info!("Processing file {filename}");

    // Read file into string
    let mut content = String::new();
    File::open(&filename)
        .await?
        .read_to_string(&mut content)
        .await?;

    // Parse AST
    let python_ast = parse_expression(&content, &filename).unwrap();

    // Walk AST
    // println!("{:?}\n", python_ast);
    let mut visitor = LoggerVisitor {};
    let changes = go_for_a_walkie(&mut visitor, &python_ast);

    println!("Found changes: {:?}", changes);

    Ok(true) // TODO: only return true if we've made changes
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    // Parse CLI arguments
    let opts = Opts::parse();

    // Fix all files concurrently
    let mut tasks = vec![];
    for filename in opts.filenames {
        if filename.ends_with(".py") {
            tasks.push(tokio::task::spawn(fix_file(filename)));
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
