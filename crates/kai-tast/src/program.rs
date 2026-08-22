use crate::fn_decl::TypedFnDecl;
use crate::ty::KaiType;

#[derive(Debug, Clone, PartialEq)]
pub struct TypedProgram {
    /// Declared structs in resolution order; `StructId` indexes this list.
    pub structs: Vec<TypedStruct>,
    pub fns: Vec<TypedFnDecl>,
}

impl TypedProgram {
    pub fn find(&self, name: &str) -> Option<&TypedFnDecl> {
        self.fns.iter().find(|decl| decl.name == name)
    }
}

/// A resolved struct layout. Field ORDER is ABI: it drives LLVM struct types
/// and getelementptr indices everywhere downstream.
#[derive(Debug, Clone, PartialEq)]
pub struct TypedStruct {
    pub name: String,
    pub fields: Vec<TypedStructField>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypedStructField {
    pub name: String,
    pub ty: KaiType,
}
