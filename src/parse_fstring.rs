use crate::cli::emit_error;
use crate::parse_format::get_args_and_keywords;
use crate::visitor::{constant_to_string, operator_to_string};
use crate::THREAD_LOCAL_STATE;
use anyhow::bail;
use anyhow::Result;
use rustpython_parser::ast::{Expr, ExprKind};

pub fn parse_formatted_value(
    value: &Expr,
    postfix: String,
    in_call: bool,
    quote: char,
) -> Result<String> {
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
                parse_formatted_value(value, attr.to_string(), false, quote)?
            } else {
                parse_formatted_value(value, format!("{attr}.{postfix}"), false, quote)?
            }
        }
        // A constant is a value like 1 or None.
        // We want these values to be moved out of the string.
        ExprKind::Constant { value, .. } => {
            if in_call {
                format!("{}{}{}", quote, constant_to_string(value.clone()), quote)
            } else {
                constant_to_string(value.clone())
            }
        }
        // Calls are function calls. So for example we might see f"{len(foo)}" in an f-string.
        // Here, we want to move the entire contents of the formatted value out of the string.
        // This requires us to reconstruct the string from AST.
        ExprKind::Call {
            func,
            args: call_args,
            keywords,
        } => {
            let (f_args, f_named_args) = get_args_and_keywords(call_args, keywords, quote)?;
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
                            String::new() + &comma_delimited_named_arguments;
                    }

                    // Finally, push the reconstructed function call to the outside of the string
                    // and just add a %s in the string.
                    if !f_args.is_empty() && !comma_delimited_named_arguments.is_empty() {
                        format!(
                            "{}({}, {})",
                            id,
                            f_args.join(", "),
                            comma_delimited_named_arguments
                        )
                    } else {
                        format!(
                            "{}({}{})",
                            id,
                            f_args.join(", "),
                            comma_delimited_named_arguments
                        )
                    }
                }
                ExprKind::Attribute { value, attr, .. } => {
                    let call = {
                        let mut s = "(".to_string();
                        let mut first_arg = true;

                        for arg in f_args {
                            if first_arg {
                                s.push_str(&arg.to_string());
                                first_arg = false;
                            } else {
                                s.push_str(&format!(", {arg}"));
                            }
                        }
                        for kwarg in f_named_args {
                            if first_arg {
                                s.push_str(&format!(
                                    "{}={}",
                                    kwarg.key,
                                    constant_to_string(kwarg.value)
                                ));
                                first_arg = false;
                            } else {
                                s.push_str(&format!(
                                    ", {}={}",
                                    kwarg.key,
                                    constant_to_string(kwarg.value)
                                ));
                            }
                        }
                        s.push(')');
                        s
                    };

                    format!(
                        "{}.{}{}",
                        parse_formatted_value(value, postfix, true, quote)?,
                        attr,
                        call
                    )
                }
                _ => {
                    let filename = THREAD_LOCAL_STATE.with(|tl| tl.filename.clone());
                    emit_error(&format!(
                        "Failed to parse `{}` line {}",
                        filename,
                        func.location.row()
                    ));
                    bail!("")
                }
            }
        }
        ExprKind::BinOp { left, op, right } => {
            format!(
                "{} {} {}",
                parse_formatted_value(left, postfix.clone(), false, quote)?,
                operator_to_string(op),
                parse_formatted_value(right, postfix, false, quote)?
            )
        }
        ExprKind::Subscript { value, slice, .. } => {
            format!(
                "{}[{}{}{}]",
                parse_formatted_value(value, postfix.clone(), false, quote)?,
                quote,
                parse_formatted_value(slice, postfix, false, quote)?,
                quote
            )
        }
        ExprKind::ListComp { elt, generators } | ExprKind::GeneratorExp { elt, generators } => {
            let mut s = format!(
                "[{}",
                parse_formatted_value(elt, postfix.clone(), true, quote)?,
            );
            for generator in generators {
                s.push_str(&format!(
                    " for {} in {}",
                    parse_formatted_value(&generator.target, postfix.clone(), true, quote)?,
                    parse_formatted_value(&generator.iter, postfix.clone(), true, quote)?
                ));
            }
            s.push(']');
            s
        }
        ExprKind::DictComp {
            key,
            value,
            generators,
        } => {
            let mut s = format!(
                "{{{}: {}",
                parse_formatted_value(key, postfix.clone(), true, quote)?,
                parse_formatted_value(value, postfix.clone(), true, quote)?,
            );
            for generator in generators {
                s.push_str(&format!(
                    " for {} in {}",
                    parse_formatted_value(&generator.target, postfix.clone(), true, quote)?,
                    parse_formatted_value(&generator.iter, postfix.clone(), true, quote)?
                ));
            }
            s.push('}');
            s
        }
        ExprKind::JoinedStr { .. } => {
            bail!("Won't handle f-strings inside f-strings")
        }
        _ => {
            let filename = THREAD_LOCAL_STATE.with(|tl| tl.filename.clone());
            emit_error(&format!(
                "Failed to parse `{}` line {}",
                filename,
                value.location.row()
            ));
            bail!("");
        }
    };
    Ok(string)
}

fn parse_fstring(
    value: &Expr,
    string: &mut String,
    args: &mut Vec<String>,
    quote: char,
) -> Result<()> {
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
            args.push(parse_formatted_value(value, String::new(), false, quote)?);
        }
        _ => {
            let filename = THREAD_LOCAL_STATE.with(|tl| tl.filename.clone());
            emit_error(&format!(
                "Failed to parse `{}` line {}",
                filename,
                value.location.row()
            ));
            bail!("");
        }
    }
    Ok(())
}

pub fn fix_fstring(values: &[Expr], quote: char) -> Option<(String, Vec<String>)> {
    let mut string = String::new();
    let mut args = vec![];

    for value in values {
        match parse_fstring(value, &mut string, &mut args, quote) {
            Ok(_) => (),
            Err(_) => return None,
        }
    }

    Some((string, args))
}
