use ruff_python_ast::visitor::Visitor;
use rustpython_parser::ast::{Constant, Expr, ExprKind};
use crate::Change;
use crate::format::check_for_format;

pub(crate) struct LoggerVisitor {
    pub(crate) changes: Vec<Change>,
}

impl<'a> Visitor<'a> for LoggerVisitor {
    fn visit_expr(&mut self, expr: &'a Expr) {
        match &expr.node {
            ExprKind::BoolOp { op, values } => {
                for expr in values {
                    self.visit_expr(expr);
                }
            },
            ExprKind::Call {
                func,
                args,
                keywords,
            } => {
                if let Some((new_string_content, new_string_variables)) = check_for_format(func, args, keywords) {
                    self.changes.push(Change {
                        lineno: expr.location.row(),
                        col_offset: expr.location.column(),
                        end_lineno: expr.end_location.unwrap().row(),
                        end_col_offset: expr.end_location.unwrap().column(),
                        new_string_content,
                        new_string_variables,
                    });
                } else {
                    self.visit_expr(func);
                    for expr in args {
                        self.visit_expr(expr);
                    }
                    for keyword in keywords {
                        self.visit_keyword(keyword);
                    }
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
    }
}


pub fn constant_to_string(constant: Constant) -> String {
    match constant {
        Constant::None => "None".to_string(),
        Constant::Bool(value) => value.to_string(),
        Constant::Str(value) => value,
        Constant::Bytes(value) => format!("b\"{}\"", String::from_utf8_lossy(&*value)),
        Constant::Int(value) => value.to_string(),
        Constant::Float(value) => value.to_string(),
        Constant::Ellipsis => "...".to_string(),
        Constant::Complex { real, imag } => format!("{}{}{}j", real, if imag >= 0.0 { "+" } else { "" }, imag),
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
