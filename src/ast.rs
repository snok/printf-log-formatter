use crate::cli::LogLevel;
use crate::gen_visitor::Visitor;
use crate::parse_format::fix_format_call;
use crate::parse_fstring::fix_fstring;
use crate::{Change, SETTINGS};
use rustpython_parser::ast::{Constant, Expr, ExprKind, Operator};

pub(crate) struct LoggerVisitor {
    pub(crate) changes: Vec<Change>,
}

impl<'a> Visitor<'a> for LoggerVisitor {
    /// Look for logger calls.
    /// Initially Here we're only after one type of call:
    ///
    ///    logger.info("{x}".format(x=1)
    ///             |__ this one
    ///
    /// Info here is not actually what we're looking for; instead we'll accept any
    /// valid python log level.
    ///
    /// Beyond that we have to be careful not to process syntax like
    /// `messages.error(self.request, f"{1}")` or other syntax that doesn't
    /// exactly fit our pattern.
    ///
    /// In the future, if needed, we might want to actually look for assignments from
    /// logging.getLogger and use that when deciding which calls to handle.
    fn visit_expr(&mut self, expr: &'a Expr) {
        match &expr.node {
            ExprKind::Call {
                func,
                args,
                keywords: _,
            } => {
                if let ExprKind::Attribute {
                    value: _,
                    attr: top_attr,
                    ..
                } = &func.node
                {
                    if let Some(log_level) = LogLevel::maybe_from_str(top_attr) {
                        if SETTINGS.get().unwrap().log_level <= log_level {
                            // Check that the first argument is an f-string or a str.format() call
                            // f-strings are ExprKind::JoinedStr and str.format() calls are ExprKind::Call
                            // The reason for this is mainly to avoid false positives for similar syntax,
                            // such as `messages.error(self.request, "foo")`, but it does leave us open to
                            // false negatives from things like `logger.error("foo" + f"{bar}").
                            // Doubt it will cause too many issues.
                            if let Some(first_value) = args.get(0) {
                                match &first_value.node {
                                    ExprKind::JoinedStr { values } => {
                                        // Now that we know the first argument is a string, go ahead and process the whole call
                                        for expr in args {
                                            if let Some((
                                                new_string_content,
                                                new_string_variables,
                                            )) = fix_fstring(values)
                                            {
                                                self.changes.push(Change {
                                                    lineno: expr.location.row(),
                                                    col_offset: expr.location.column(),
                                                    end_lineno: expr.end_location.unwrap().row(),
                                                    end_col_offset: expr
                                                        .end_location
                                                        .unwrap()
                                                        .column(),
                                                    new_string_content,
                                                    new_string_variables,
                                                });
                                            }
                                        }
                                    }
                                    ExprKind::Call {
                                        func,
                                        args,
                                        keywords,
                                    } => {
                                        if let ExprKind::Attribute {
                                            value: _,
                                            attr,
                                            ctx: _,
                                        } = &func.node
                                        {
                                            if attr == "format" {
                                                if let Some((
                                                    new_string_content,
                                                    new_string_variables,
                                                )) = fix_format_call(func, args, keywords)
                                                {
                                                    self.changes.push(Change {
                                                        lineno: first_value.location.row(),
                                                        col_offset: first_value.location.column(),
                                                        end_lineno: first_value
                                                            .end_location
                                                            .unwrap()
                                                            .row(),
                                                        end_col_offset: first_value
                                                            .end_location
                                                            .unwrap()
                                                            .column(),
                                                        new_string_content,
                                                        new_string_variables,
                                                    });
                                                }
                                            }
                                        }
                                    }
                                    _ => {
                                        // Skipping, since first argument was not an f-string or str.format call
                                    }
                                }
                            }
                        }
                    }
                }
            }
            ExprKind::BoolOp { op: _, values } => {
                for expr in values {
                    self.visit_expr(expr);
                }
            }
            _ => (),
        }
    }
}

pub fn constant_to_string(constant: Constant) -> String {
    match constant {
        Constant::None => "None".to_string(),
        Constant::Bool(value) => value.to_string(),
        Constant::Str(value) => value,
        Constant::Bytes(value) => format!("b\"{}\"", String::from_utf8_lossy(&value)),
        Constant::Int(value) => value.to_string(),
        Constant::Float(value) => value.to_string(),
        Constant::Ellipsis => "...".to_string(),
        Constant::Complex { real, imag } => {
            format!("{}{}{}j", real, if imag >= 0.0 { "+" } else { "" }, imag)
        }
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

pub fn operator_to_string(operator: &Operator) -> String {
    match operator {
        Operator::Add => "+".to_owned(),
        Operator::Sub => "-".to_owned(),
        Operator::Mult => "*".to_owned(),
        Operator::MatMult => "@".to_owned(),
        Operator::Div => "/".to_owned(),
        Operator::Mod => "%".to_owned(),
        Operator::Pow => "**".to_owned(),
        Operator::LShift => "<<".to_owned(),
        Operator::RShift => ">>".to_owned(),
        Operator::BitOr => "|".to_owned(),
        Operator::BitXor => "^".to_owned(),
        Operator::BitAnd => "&".to_owned(),
        Operator::FloorDiv => "//".to_owned(),
    }
}
