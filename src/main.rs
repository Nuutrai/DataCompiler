#![allow(warnings)]
use crate::analysis::ownership::OwnershipChecker;
use crate::ast::lexer::Lexer;
use crate::ast::parser::Parser;
use crate::codegen::codegen::Codegen;
use std::env;
use std::fs;
mod analysis;
mod ast;
mod codegen;
mod error;
mod util;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 3 {
        eprintln!("Usage: {} <filename> <target>", args[0]);
        std::process::exit(1);
    }

    let user_input = fs::read_to_string(&args[1]).unwrap_or_else(|err| {
        eprintln!("Error reading file '{}': {}", args[1], err);
        std::process::exit(1);
    });

    let tokens = Lexer::new(user_input).tokenize();
    let mut parser = Parser::new(tokens, &args[1]);
    let ast = parser.parse();
    parser.errors.fatal_if_any();

    let mut checker = OwnershipChecker::new(&args[1]);
    checker.check(&ast);
    checker.errors.fatal_if_any();

    let mut codegen = Codegen::new(&args[1], &args[2]);
    let ir = codegen.generate(&ast);
    codegen.errors.fatal_if_any();
    codegen.compile_to_binary(&ir, "output");
}
