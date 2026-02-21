use std::fs;
use std::env;

use crate::ast::lexer::Lexer;
use crate::ast::parser::Parser;

mod ast;

fn main() {
    let args: Vec<String> = env::args().collect();
    
    if args.len() < 2 {
        eprintln!("Usage: {} <filename>", args[0]);
        std::process::exit(1);
    }
    
    let filename = &args[1];
    
    let input = fs::read_to_string(filename)
        .unwrap_or_else(|err| {
            eprintln!("Error reading file '{}': {}", filename, err);
            std::process::exit(1);
        });
    let lexer = Lexer::new(input);
    let tokens = lexer.tokenize();
    let mut parser = Parser::new(tokens);
    loop {
        parser.skip_newlines();
        if parser.is_at_end() {
            break;
        }
        let expr = parser.parse_expr();
        println!("{:#?}", expr);
        parser.skip_newlines();
    }
}
