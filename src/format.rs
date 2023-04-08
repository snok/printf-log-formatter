#![allow(unused_variables)]
use crate::ast::constant_to_string;
use log::{debug, info};
use regex::Regex;
use rustpython_parser::ast::{Constant, Expr, ExprKind, Keyword, KeywordData};

#[derive(Debug)]
struct NamedArg {
    key: String,
    value: Constant,
}

#[derive(Debug)]
pub struct Arg {
    value: String,
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

/// Look for logger.error("string {var}.format(var=var)) syntax
pub fn check_for_format(
    func: &Box<Expr>,
    args: &Vec<Expr>,
    keywords: &Vec<Keyword>,
) -> Option<(String, Vec<String>)> {
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
            match &value.node {
                ExprKind::Constant { value, kind } => {
                    if let Some(arg) = arg {
                        info!("Found named argument {} with value {:?}", arg, value);
                        format_named_args.push(NamedArg {
                            key: arg.to_string(),
                            value: value.clone(),
                        })
                    } else {
                        info!("Found unnamed argument with value {:?}", value);
                        format_args.push(Arg {
                            value: constant_to_string(value.clone()),
                        })
                    }
                }
                ExprKind::Name { id, ctx } => {
                    info!("Found named argument with variable name {id}");
                    format_args.push(Arg {
                        value: id.to_string(),
                    })
                }
                _ => {
                    unreachable!()
                }
            }
        }
    }

    for arg in args {
        println!("---\n{:?}\n---", arg);

        if let ExprKind::Constant { value, kind } = &arg.node {
            info!("Found unnamed argument with value {:?}", value);
            format_args.push(Arg {
                value: constant_to_string(value.clone()),
            })
        }
        if let ExprKind::Name { id, ctx } = &arg.node {
            info!("Found unnamed argument with variable name {:?}", id);
            format_args.push(Arg {
                value: id.to_string(),
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

    // Replace all keyword arguments with %s and insert each of their values
    // into the ordered_arguments vector, in the right order. Something to be
    // aware of is that this is valid Python syntax:
    //
    //   "{x:02f} + {x:03f} - {x} == {y}".format(x=2, y=2)
    //
    // so we have to handle the potential of multiple indices for one keyword arg,
    // and we need to separate the variable name from the contents of the curly brace.
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
    let any_curly_brace_re = Regex::new(r"\{[^{}]*(:[^{}]*)?\}").unwrap();
    for arg in format_args {
        let mat = match any_curly_brace_re.find(&new_string) {
            Some(t) => t,
            None => {
                // This will happen for syntax like
                //  logger.info("{}".format(1,2))
                // where there are more arguments passed than mapped to.
                // We could ignore these cases, but if we silently fixed them
                // that might cause other problems for the user ¯\_(ツ)_/¯
                panic!("Found excess argument `{}` in logger. Run with RUST_LOG=debug for verbose logging.", arg.value)
            }
        };
        let start = mat.start();
        let end = mat.end();

        // Replace a {} with %s
        new_string.replace_range(start..end, "%s");

        let index = ordered_arguments.iter().position(|x| x.is_none()).unwrap();
        ordered_arguments[index] = Some(arg.value);
    }

    let string_addon = ordered_arguments
        .iter()
        .map(|s| s.clone().unwrap())
        .collect();

    debug!("After handling args the new string is {}", new_string);
    Some((new_string, string_addon))
}
