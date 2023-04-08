#![allow(unused_variables)]
use crate::ast::constant_to_string;
use log::debug;
use rustpython_parser::ast::{Expr, ExprKind};

pub fn check_for_fstring(values: &Vec<Expr>) -> Option<(String, Vec<String>)> {
    let mut string = String::new();
    let mut args = vec![];

    for value in values {
        match &value.node {
            ExprKind::Constant { value, kind } => {
                let v = &constant_to_string(value.to_owned());
                debug!("Pushing {v} to string");
                string.push_str(&v);
            }
            ExprKind::FormattedValue {
                value,
                conversion,
                format_spec,
            } => match &value.node {
                ExprKind::Name { id, ctx } => {
                    debug!("Pushing variable {id} to string");
                    args.push(id.to_string());
                    string.push_str("%s");
                }
                ExprKind::Constant { value, kind } => {
                    debug!("Pushing constant {:?} to string", value);
                    args.push(constant_to_string(value.to_owned()));
                    string.push_str("%s");
                }
                _ => {
                    println!("{:?}", &value.node);
                    unreachable!()
                }
            },
            ExprKind::JoinedStr { values } => {
                println!("Skipping joinedstr");
            }
            _ => unreachable!(),
        }
        println!("string: {}, args: {:?}", string, args);
    }

    Some((string, args))
}
