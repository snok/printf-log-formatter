use std::str::FromStr;
use anyhow::Result;
use tokio::fs::{File};
use tokio::io::{AsyncReadExt};
use crate::options::Opts;
use clap::{Parser};
use futures::future::join_all;
use tokio::join;
use rustpython_parser::ast::{Alias, Constant, ConversionFlag, Expr, ExprContext, ExprKind, Mod, Stmt};
use rustpython_parser::parse_expression;
use crate::enums::LogLevel;
use crate::visitor::{Visitor, walk_alias, walk_body, walk_constant, walk_expr};

mod enums;
mod options;
mod visitor;

const WORD: &str = "logger.";
const WORD_LENGTH: usize = 6;

struct LoggerVisitor {
    buffer: String,
}


impl<'a> Visitor<'a> for LoggerVisitor {
    fn visit_expr(&mut self, expr: &'a Expr) {
        match &expr.node {
            ExprKind::JoinedStr { values } => {
                for value in values {
                    self.unparse_fstring_elem(value);
                }
            }
            _ => ()
        }
    }
}

impl LoggerVisitor {
    fn unparse_fstring_str(&mut self, s: &str) {
        let s = s.replace('{', "{{").replace('}', "}}");
        self.p(&s);
    }
    fn unparse_fstring_elem<U>(&mut self, expr: &Expr<U>) {
        match &expr.node {
            ExprKind::Constant { value, .. } => {
                if let Constant::Str(s) = value {
                    self.unparse_fstring_str(s);
                } else {
                    println!("1");
                    unreachable!()
                }
            }
            ExprKind::JoinedStr { values } => self.unparse_fstring_body(values),
            ExprKind::FormattedValue {
                value,
                conversion,
                format_spec,
            } => self.unparse_formatted(value, *conversion, format_spec.as_deref()),

            _ =>
                {
                    println!("2");
                    unreachable!()
                },
        }
    }
    fn unparse_fstring_body<U>(&mut self, values: &[Expr<U>]) {
        for value in values {
            self.unparse_fstring_elem(value);
        }
    }
    fn p(&mut self, s: &str) {
        self.buffer += s;
    }

    fn unparse_formatted<U>(&mut self, val: &Expr<U>, conversion: usize, spec: Option<&Expr<U>>) {
        let brace = if self.buffer.starts_with('{') {
            // put a space to avoid escaping the bracket
            "{ "
        } else {
            "{"
        };
        self.p(brace);
        self.buffer += &generator.buffer;

        if conversion != ConversionFlag::None as usize {
            self.p("!");
            #[allow(clippy::cast_possible_truncation)]
            self.p(&format!("{}", conversion as u8 as char));
        }

        if let Some(spec) = spec {
            self.p(":");
            self.unparse_fstring_elem(spec);
        }

        self.p("}");
    }
}

async fn fix_file(filename: String) -> Result<()> {
    // Read file - TODO: Use BufReader?
    let mut file = File::open(&filename).await?;
    let mut contents = String::new();
    file.read_to_string(&mut contents).await?;

    // Parse AST
    let python_ast = parse_expression(&contents, &filename).unwrap();

    // Walk AST
    println!("{:?}\n", python_ast);
    let mut visitor = LoggerVisitor {
        buffer: "".to_string(),
    };
    visitor::walk_expr(&mut visitor, &python_ast);
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let opts = Opts::parse();
    println!("{:?}, {:?}", opts.filenames, opts.log_level);


    let mut tasks = vec![];
    for filename in opts.filenames {
        if filename.ends_with(".py") {
            tasks.push(tokio::task::spawn(fix_file(filename)));
        }
    }

    join_all(tasks).await;

    let exit_code = 1;
    std::process::exit(if exit_code != 0 { 1 } else { 0 });
}
