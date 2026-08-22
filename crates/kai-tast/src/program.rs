use crate::fn_decl::TypedFnDecl;

#[derive(Debug, Clone, PartialEq)]
pub struct TypedProgram {
    pub fns: Vec<TypedFnDecl>,
}

impl TypedProgram {
    pub fn find(&self, name: &str) -> Option<&TypedFnDecl> {
        self.fns.iter().find(|decl| decl.name == name)
    }
}
