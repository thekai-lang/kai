//! Tokenizer: source text -> tokens + lexical diagnostics.

pub mod cursor;
pub mod keywords;
pub mod lexer;
pub mod token;

pub use lexer::{LexOutput, lex};
pub use token::{Token, TokenKind};
