use rustpython_parser::ast::{Constant, Expr, ExprKind, Keyword, Operator};

use crate::cli::{get_quotes, LogLevel};
use crate::gen_visitor::Visitor;
use crate::parse_format::fix_format_call;
use crate::parse_fstring::fix_fstring;
use crate::{Change, SETTINGS};

// List of calls we explicitly know are unlikely to be loggers
// for example, warnings.warn() is relatively common syntax
// that we don't want to confuse for logger.warn.
const BLACKLISTED_NAMES: [&str; 2] = ["warnings", "messages"];

pub(crate) struct LoggerVisitor {
    pub(crate) changes: Vec<Change>,
}

impl<'a> Visitor<'a> for LoggerVisitor {
    /// Look for logger calls.
    ///
    /// Initially Here we're only after one type of call:
    ///
    ///    logger.info("{x}".format(x=1)
    ///            |__ this one
    ///
    /// `info` here is not actually what we're looking for;
    /// we'll accept any valid python log level.
    ///
    /// Beyond this, we have to be careful not to process syntax like
    /// `messages.error(self.request, f"{1}")` or other syntax that doesn't
    /// exactly fit our pattern. To negate this particular pattern, we've
    /// added checking to see if the first argument to the call is a string or not.
    ///
    /// In the future, if needed, we might want to actually look for assignments from
    /// logging.getLogger and use that when deciding which calls to handle, but that's
    /// also not a fool-proof solution, as you can import loggers from other files, etc.
    /// Loggers can also be called anything, not just `logger.info`. Many use `log.info`,
    /// `LOG.info`, and more.
    fn visit_expr(&mut self, expr: &'a Expr) {
        match &expr.node {
            ExprKind::Call { func, args, .. } => self.handle_call(func, args),
            ExprKind::BoolOp { op: _, values } => {
                for expr in values {
                    self.visit_expr(expr);
                }
            }
            _ => (),
        }
    }
}

impl LoggerVisitor {
    fn handle_call(&mut self, func: &Expr, args: &Vec<Expr>) {
        if let ExprKind::Attribute {
            value,
            attr: call_attr,
            ..
        } = &func.node
        {
            // Make sure the call made matches a valid log level
            let Some(log_level) = LogLevel::maybe_from_str(call_attr) else { return };

            // Only handle log levels above the settings value
            if SETTINGS.get().unwrap().log_level > log_level {
                return;
            }

            // Make sure we're not handling any expressions like `warnings.warn()`
            if let ExprKind::Name { id, .. } = &value.node {
                if BLACKLISTED_NAMES.contains(&&**id) {
                    return;
                }
            }

            // Make sure the first argument is an f-string or a str.format() call
            //
            // This is mainly done to avoid false positives for similar syntax,
            // such as `messages.error(self.request, "foo")`, but it does leave us open to
            // false negatives from things like `logger.error("foo" + f"{bar}").
            // Doubt it will cause too many issues.
            if let Some(first_value) = args.get(0) {
                match &first_value.node {
                    ExprKind::JoinedStr { values } => {
                        for expr in args {
                            if let ExprKind::JoinedStr { .. } = &expr.node {
                                self.handle_joinedstr(expr, values);
                            }
                        }
                    }
                    ExprKind::Call {
                        func,
                        args,
                        keywords,
                    } => {
                        if let ExprKind::Attribute { attr, .. } = &func.node {
                            if attr == "format" {
                                self.handle_str_format_call(first_value, func, args, keywords);
                            }
                        }
                    }
                    _ => (),
                }
            }
        }
    }

    fn capture_changes<F>(&mut self, expr: &Expr, values: &[Expr], conversion_fn: F)
    where
        F: FnOnce(&[Expr], char) -> Option<(String, Vec<String>)>,
    {
        let Ok(quote) = get_quotes(expr.location.row(), expr.location.column()) else { return };

        if let Some((new_string_content, new_string_variables)) = conversion_fn(values, quote) {
            if !new_string_content.is_empty() {
                self.changes.push(Change {
                    lineno: expr.location.row(),
                    col_offset: expr.location.column(),
                    end_lineno: expr.end_location.unwrap().row(),
                    end_col_offset: expr.end_location.unwrap().column(),
                    new_string_content,
                    new_string_variables,
                    quote,
                });
            }
        }
    }

    /// Handle f-string AST node
    fn handle_joinedstr(&mut self, expr: &Expr, values: &[Expr]) {
        self.capture_changes(expr, values, fix_fstring);
    }

    /// Handle str.format() call AST node
    fn handle_str_format_call(
        &mut self,
        first_value: &Expr,
        func: &Expr,
        args: &[Expr],
        keywords: &[Keyword],
    ) {
        self.capture_changes(first_value, args, |args, quote| {
            fix_format_call(func, args, keywords, quote).ok().flatten()
        });
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
            format!("({items})")
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
