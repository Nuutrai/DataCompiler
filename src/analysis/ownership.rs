use crate::ast::parser::{Expr, Statement};
use crate::error::{ErrorKind, ErrorReporter};
use std::collections::HashMap;

pub struct OwnershipChecker {
    scope: HashMap<String, bool>,
    pub errors: ErrorReporter,
}

impl OwnershipChecker {
    pub fn new(file: &str) -> Self {
        Self {
            scope: HashMap::new(),
            errors: ErrorReporter::new(file),
        }
    }

    pub fn check(&mut self, statements: &[Statement]) {
        for stmt in statements {
            self.check_statement(stmt);
        }
    }

    fn check_statement(&mut self, stmt: &Statement) {
        match stmt {
            Statement::Variable { name, value, .. } => {
                self.check_expr(value);
                self.scope.insert(name.clone(), true);
            }

            Statement::Function { args, body, .. } => {
                for (name, _) in args {
                    self.scope.insert(name.clone(), true);
                }
                self.check(body);
            }

            Statement::Expr(e) => {
                self.check_expr(e);
            }
            _ => {}

            Statement::Assign { target, value, .. } => {
                self.check_expr(value);
            }
        }
    }

    fn check_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Number(_, _) | Expr::String(_, _) => {}

            Expr::Identifier(name, span) => match self.scope.get_mut(name) {
                Some(false) => self.errors.error(
                    ErrorKind::AlreadyConsumed(name.clone()),
                    span.line,
                    span.start,
                    span.len(),
                ),
                Some(owned) => *owned = false,
                None => {}
            },

            Expr::Group(inner, _) => self.check_expr(inner),

            Expr::Call { callee, args, .. } => {
                self.check_expr(callee);
                for arg in args {
                    self.check_expr(arg);
                }
            }

            Expr::Index { target, index, .. } => {
                self.check_expr_read(target);
                self.check_expr(index);
            }

            Expr::Ternary {
                cond,
                then_branch,
                else_branch,
                ..
            } => {
                self.check_expr_read(cond);
                self.check_expr(then_branch);
                self.check_expr(else_branch);
            }
        }
    }

    fn check_expr_read(&mut self, expr: &Expr) {
        match expr {
            Expr::Identifier(name, span) => match self.scope.get(name) {
                Some(false) => self.errors.error(
                    ErrorKind::AlreadyConsumed(name.clone()),
                    span.line,
                    span.start,
                    span.len(),
                ),
                _ => {}
            },
            other => self.check_expr(other),
        }
    }
}
