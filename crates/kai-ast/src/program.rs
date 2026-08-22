use crate::fn_decl::FnDecl;

#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub fns: Vec<FnDecl>,
}

impl Program {
    pub fn empty() -> Self {
        Self { fns: Vec::new() }
    }
}
