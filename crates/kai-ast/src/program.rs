use crate::fn_decl::FnDecl;
use crate::type_decl::TypeDecl;

#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub fns: Vec<FnDecl>,
    pub types: Vec<TypeDecl>,
}

impl Program {
    pub fn empty() -> Self {
        Self {
            fns: Vec::new(),
            types: Vec::new(),
        }
    }
}
