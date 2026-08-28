fn main() {
    let source = std::fs::read_to_string("/home/aras/Documents/Tes/main.kai").unwrap();
    let l = kai_lexer::lex(&source);
    let p = kai_parser::parse(&l.tokens).unwrap();
    println!("{:#?}", p.fns[0].body);
}
