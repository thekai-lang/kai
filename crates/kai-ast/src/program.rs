use crate::fn_decl::FnDecl;
use crate::type_decl::TypeDecl;
use crate::use_decl::UseDecl;

#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub use_decls: Vec<UseDecl>,
    pub fns: Vec<FnDecl>,
    pub types: Vec<TypeDecl>,
}

impl Program {
    pub fn empty() -> Self {
        Self {
            use_decls: Vec::new(),
            fns: Vec::new(),
            types: Vec::new(),
        }
    }
}
