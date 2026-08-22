//! Types in syntactic position (single names in v0.0.1; arrays/optionals land
//! in v0.0.5).

use crate::parser::Parser;
use kai_ast::{Ident, Ty};

pub fn ty(parser: &mut Parser) -> Ty {
    let ident: Ident = parser.expect_ident("a type name");
    Ty::Named(ident)
}
