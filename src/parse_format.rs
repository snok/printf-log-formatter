use crate::ast::constant_to_string;
use crate::parse_fstring::parse_formatted_value;
use crate::FILENAME;
use anyhow::bail;
use anyhow::Result;
use regex::Regex;
use rustpython_parser::ast::{Constant, Expr, ExprKind, Keyword, KeywordData};

#[derive(Debug)]
pub struct NamedArg {
    pub(crate) key: String,
    pub(crate) value: Constant,
}

fn get_named_arg_index_start_end(re: &Regex, string: &str, key: &str) -> Result<(usize, usize)> {
    for cap in re.captures_iter(string) {
        let capture = cap.get(0).unwrap();
        if cap.get(1).unwrap().as_str() == key {
            return Ok((capture.start(), capture.end()));
        }
    }
    bail!("Failed to capture named args for string '{string}'. Please submit a ticket to https://github.com/sondrelg/printf-log-formatter/issues")
}

fn get_named_arg_indexes(re: &Regex, string: &str, key: &str) -> Vec<usize> {
    let mut matches = vec![];
    for (i, cap) in re.captures_iter(string).enumerate() {
        if cap.get(1).unwrap().as_str() == key {
            matches.push(i);
        }
    }
    matches
}

pub fn get_args_and_keywords(
    args: &Vec<Expr>,
    keywords: &Vec<Keyword>,
) -> Result<(Vec<String>, Vec<NamedArg>)> {
    let mut f_named_args: Vec<NamedArg> = vec![];
    let mut f_args: Vec<String> = vec![];

    for keyword in keywords {
        let KeywordData { arg, value } = &keyword.node;
        match &value.node {
            ExprKind::Constant { value, .. } => {
                if let Some(arg) = arg {
                    f_named_args.push(NamedArg {
                        key: arg.to_string(),
                        value: value.clone(),
                    });
                } else {
                    f_args.push(constant_to_string(value.clone()));
                }
            }
            ExprKind::Name { id, .. } => f_args.push(id.to_string()),
            _ => {
                let filename = FILENAME.with(std::clone::Clone::clone);
                let error_message = format!("Failed to parse `{}` line {}. Please open an issue at https://github.com/sondrelg/printf-log-formatter/issues/new :)", filename, value.location.row());
                eprintln!("{error_message}");
                bail!("");
            }
        }
    }

    for arg in args {
        f_args.push(parse_formatted_value(arg, String::new())?);
    }

    Ok((f_args, f_named_args))
}

// Captures any {} in a string
const FORMATTED_VALUE_REGEX: &str = r"\{.*?\}";

// Captures any {} in a string, but creates a group for
// {first:second} where second is optional. This lets us separate
// the variable from the formatting in `{foo:02f}`
// TODO: Can't we just use AST?
const FORMATTED_VALUE_GROUP_REGEX: &str = r"\{([^{}:]*)(?::[^{}]*)?\}";

// TODO: Try replacing with FORMATTED_VALUE_REGEX
const FORMATTED_VALUE_GROUP_REGEX_COLON_CHARACTERS: &str = r"\{[^{}]*(:[^{}]*)?\}";

/// Replace all keyword arguments with %s and insert each of their values
/// into the `ordered_arguments` vector, in the right order. Something to be
/// aware of is that this is valid Python syntax:
///
///   "{x:02f} + {x:03f} - {x} == {y}".format(x=2, y=2)
///
/// so we have to handle the potential of multiple indices for one keyword arg,
/// and we need to separate the variable name from the contents of the curly brace.
fn order_keyword_arguments(
    string: &mut str,
    new_string: &mut String,
    f_named_args: Vec<NamedArg>,
    ordered_arguments: &mut [Option<String>],
) -> Result<()> {
    let group_regex = Regex::new(FORMATTED_VALUE_GROUP_REGEX).unwrap();
    for keyword_arg in f_named_args {
        // Get all indexes for the given keyword argument key
        let indexes = get_named_arg_indexes(&group_regex, string, &keyword_arg.key);

        // Convert Rust type to a string value
        let str_value = constant_to_string(keyword_arg.value);

        // Push each string value to the right index
        // We might push index 1, then 3; not 0,1,2.
        for index in indexes {
            let (start, end) =
                get_named_arg_index_start_end(&group_regex, new_string, &keyword_arg.key)?;

            // Insert value into the right index for printf-style formatting later
            ordered_arguments[index] = Some(str_value.clone());

            // Replace the curly brace from the string
            new_string.replace_range(start..end, "%s");
        }
    }
    Ok(())
}

// Args are captured in order, so we should be able to just fill in the missing ordered arguments.
// One nice assumption we can make here is that each arg is unique and only appears once.
fn order_arguments(
    new_string: &mut String,
    f_args: Vec<String>,
    ordered_arguments: &mut [Option<String>],
) {
    let any_curly_brace_re = Regex::new(FORMATTED_VALUE_GROUP_REGEX_COLON_CHARACTERS).unwrap();
    for arg in f_args {
        let Some(mat) = any_curly_brace_re.find(new_string) else {
            // This will happen for syntax like
            //  logger.info("{}".format(1,2))
            // where there are more arguments passed than mapped to.
            // We could ignore these cases, but if we silently fixed them
            // that might cause other problems for the user ¯\_(ツ)_/¯
            panic!("Found excess argument `{arg}` in logger. Run with RUST_LOG=debug for verbose logging.")
        };
        let start = mat.start();
        let end = mat.end();

        // Replace a {} with %s
        new_string.replace_range(start..end, "%s");

        // Find the first `None` in the ordered arguments vector and fill it with
        // our argument value. This relies on keyword arguments being populated first.
        let index = ordered_arguments
            .iter()
            .position(std::option::Option::is_none)
            .unwrap();
        ordered_arguments[index] = Some(arg);
    }
}

fn order(
    string: &mut str,
    new_string: &mut String,
    f_args: Vec<String>,
    f_named_args: Vec<NamedArg>,
    ordered_arguments: &mut [Option<String>],
) -> Result<()> {
    // Keyword arguments need to be handled first, or the ordered_arguments logic breaks
    order_keyword_arguments(string, new_string, f_named_args, ordered_arguments)?;
    order_arguments(new_string, f_args, ordered_arguments);
    Ok(())
}

/// Parse str.format() AST
///
/// First we need to map all the args and keyword args that exist; then we need to figure
/// out which order they appear in, in the string itself.
pub fn fix_format_call(
    func: &Expr,
    args: &Vec<Expr>,
    keywords: &Vec<Keyword>,
) -> Result<Option<(String, Vec<String>)>> {
    // Get all arguments and named arguments from the str.format(...) call
    let (f_args, f_named_args) = get_args_and_keywords(args, keywords)?;

    // Copy the string from the str.format() call
    let mut string = String::new();
    if let ExprKind::Attribute { value, .. } = &func.node {
        if let ExprKind::Constant {
            value: Constant::Str(s),
            kind: _,
        } = &value.node
        {
            string.push_str(s);
        }
    }
    // Make a copy of the string for later
    let mut new_string = string.clone();

    // Initialize an empty vector which will hold our arguments
    // The str.format() syntax is a little trickier to handle than f-strings, since the
    // call can contain both named an unnamed arguments, and they while the unnamed arguments
    // are inserted in an ordered manner, the named arguments could belong to any of the
    // curly brace pairs. A named argument can also appear multiple times.
    let mut ordered_arguments: Vec<Option<String>> = vec![
        None;
        Regex::new(FORMATTED_VALUE_REGEX)
            .unwrap()
            .find_iter(&string)
            .count()
    ];

    order(
        &mut string,
        &mut new_string,
        f_args,
        f_named_args,
        &mut ordered_arguments,
    )?;

    let string_addon = ordered_arguments
        .iter()
        .map(|s| s.clone().unwrap())
        .collect();

    Ok(Some((new_string, string_addon)))
}
