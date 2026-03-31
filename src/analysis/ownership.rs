use crate::ast::parser::{AssignTarget, Expr, Statement};
use crate::error::{ErrorKind, ErrorReporter};
use std::collections::{HashMap, HashSet};
use crate::ast::lexer::TextSpan;

pub struct OwnershipChecker {
    scope: HashMap<String, bool>,
    pub errors: ErrorReporter,
    undefined_vars: HashMap<String, Statement>,
}

impl OwnershipChecker {
    pub fn new(file: &str) -> Self {
        Self {
            scope: HashMap::new(),
            errors: ErrorReporter::new(file),
            undefined_vars: HashMap::new(),
        }
    }

    pub fn check(&mut self, statements: &[Statement]) {
        for stmt in statements {
            self.check_statement(stmt);
        }
        self.undefined_vars.clear();
    }

    fn check_statement(&mut self, stmt: &Statement) {
        match stmt {
            Statement::Variable { name, value, span, .. } => {
                match value {
                    Some(value) => {
                        self.check_expr(value);
                        self.scope.insert(name.clone(), true);
                    }
                    None => {
                        self.scope.insert(name.clone(), true);
                        self.undefined_vars.insert(name.clone(), stmt.clone());
                    },
                }

            }
            Statement::Function { args, body, .. } => {
                for (name, _) in args {
                    self.scope.insert(name.clone(), true);
                }
                self.check(body);
            }
            Statement::Assign { target, value, .. } => {
                self.check_expr(value);
                match target {
                    AssignTarget::Variable(name) => {
                        self.undefined_vars.remove(name);
                    }
                    _ => {}
                }
            }
            Statement::Struct { .. } => {}
            Statement::Expr(e) => self.check_expr(e),
            _ => {}
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
                Some(owned) => {
                    if self.undefined_vars.contains_key(name) {
                        self.errors.error(ErrorKind::NeverDefined(name.clone()), span.line, span.start, span.end-span.start+1);
                    }
                    *owned = false
                },
                None => {}
            },

            Expr::Group(inner, _) => self.check_expr(inner),

            Expr::Field { target, .. } => self.check_expr(target),

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
            },
            Expr::Pointer(inner, _) => self.check_expr(inner),
            Expr::List(exprs, _) => {
                for expr in exprs {
                    self.check_expr(expr);
                }
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
