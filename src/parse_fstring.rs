use crate::ast::{constant_to_string, operator_to_string};
use crate::parse_format::get_args_and_keywords;
use crate::{FILENAME, SETTINGS};
use anyhow::bail;
use anyhow::Result;
use rustpython_parser::ast::{Expr, ExprKind};

/// Parse `FormattedValue` AST ({something})
pub fn parse_formatted_value(value: &Expr, postfix: String) -> Result<String> {
    let string = match &value.node {
        // When we see a Name node we're typically handling a variable.
        // In this case, we want variables to be referenced with %s, and
        // for the variable definition to be placed after our string.
        ExprKind::Name { id, .. } => {
            if postfix.is_empty() {
                id.to_string()
            } else {
                format!("{id}.{postfix}")
            }
        }
        // An attribute node is typically an intermediate node
        // We pass down the a reference to the `attr` value to be able
        // to reconstruct the entire chain of attributes + names in the end.
        ExprKind::Attribute { value, attr, .. } => {
            if postfix.is_empty() {
                parse_formatted_value(value, attr.to_string())?
            } else {
                parse_formatted_value(value, format!("{attr}.{postfix}"))?
            }
        }
        // A constant is a value like 1 or None.
        // We want these values to be moved out of the string.
        ExprKind::Constant { value, .. } => constant_to_string(value.clone()),
        // Calls are function calls. So for example we might see f"{len(foo)}" in an f-string.
        // Here, we want to move the entire contents of the formatted value out of the string.
        // This requires us to reconstruct the string from AST.
        ExprKind::Call {
            func,
            args: call_args,
            keywords,
        } => {
            let (f_args, f_named_args) = get_args_and_keywords(call_args, keywords)?;

            match &func.node {
                ExprKind::Name { id, .. } => {
                    // Create a string with `x=y` for all named arguments and prefix it
                    // with a comma unless the string ends up being empty.
                    let mut comma_delimited_named_arguments = f_named_args
                        .into_iter()
                        .map(|arg| format!("{}={}", arg.key, constant_to_string(arg.value)))
                        .collect::<Vec<String>>()
                        .join(", ");
                    if !comma_delimited_named_arguments.is_empty() {
                        comma_delimited_named_arguments =
                            ", ".to_string() + &comma_delimited_named_arguments;
                    }

                    // Finally, push the reconstructed function call to the outside of the string
                    // and just add a %s in the string.
                    format!(
                        "{}({}{})",
                        id,
                        f_args.join(", "),
                        comma_delimited_named_arguments
                    )
                }
                ExprKind::Attribute { .. } => {
                    format!("{}()", parse_formatted_value(func, postfix)?)
                }
                _ => {
                    let filename = FILENAME.with(std::clone::Clone::clone);
                    let error_message = format!("Failed to parse `{}` line {}. Please open an issue at https://github.com/sondrelg/printf-log-formatter/issues/new :)", filename, func.location.row());
                    eprintln!("{error_message}");
                    bail!("")
                }
            }
        }
        ExprKind::BinOp { left, op, right } => {
            format!(
                "{} {} {}",
                parse_formatted_value(left, postfix.clone())?,
                operator_to_string(op),
                parse_formatted_value(right, postfix)?
            )
        }
        ExprKind::Subscript { value, slice, .. } => {
            let quotes = SETTINGS.get().unwrap().quotes.clone();
            format!(
                "{}[{}{}{}]",
                parse_formatted_value(value, postfix.clone())?,
                quotes.char(),
                parse_formatted_value(slice, postfix)?,
                quotes.char()
            )
        }
        _ => {
            let filename = FILENAME.with(std::clone::Clone::clone);
            let error_message = format!("Failed to parse `{}` line {}. Please open an issue at https://github.com/sondrelg/printf-log-formatter/issues/new :)", filename, value.location.row());
            eprintln!("{error_message}");
            bail!("");
        }
    };
    Ok(string)
}

/// Parse f-string AST
fn parse_fstring(value: &Expr, string: &mut String, args: &mut Vec<String>) -> Result<()> {
    match &value.node {
        // When we see a constant, we can just add it back to our new string directly
        ExprKind::Constant { value, .. } => {
            string.push_str(&constant_to_string(value.clone()));
        }
        // A FormattedValue is the {} in an f-string.
        // Since a formatted value can contain constants, and we want to recursively
        // handle the structure, we'll handle the parsing of the formatted value in
        // a dedicated function.
        ExprKind::FormattedValue { value, .. } => {
            string.push_str("%s");
            args.push(parse_formatted_value(value, String::new())?);
        }
        _ => {
            let filename = FILENAME.with(std::clone::Clone::clone);
            let error_message = format!("Failed to parse `{}` line {}. Please open an issue at https://github.com/sondrelg/printf-log-formatter/issues/new :)", filename, value.location.row());
            eprintln!("{error_message}");
            bail!("");
        }
    }
    Ok(())
}

pub fn fix_fstring(values: &[Expr]) -> Option<(String, Vec<String>)> {
    let mut string = String::new();
    let mut args = vec![];

    for value in values {
        match parse_fstring(value, &mut string, &mut args) {
            Ok(_) => (),
            Err(_) => return None,
        }
    }

    Some((string, args))
}
