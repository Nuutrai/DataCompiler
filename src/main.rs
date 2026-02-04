mod ast;

fn main() {
    let input = "data wow()";

    let lexer = ast::lexer::Lexer::new(input.to_string());
    let tokens = lexer.tokenize();
    println!("{:?}", tokens)
}
