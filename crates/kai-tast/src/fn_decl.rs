use crate::stmt::{TypedBlock, TypedStmt};
use crate::symbol::FunctionId;
use crate::symbol::LocalId;
use crate::ty::{EffectSet, KaiType};

/// A parameter, already bound to its local slot by the type checker. `mut`
/// is compile-time only (§9.3): it never changes the ABI.
#[derive(Debug, Clone, PartialEq)]
pub struct TypedParam {
    pub local: LocalId,
    /// Kept as the LLVM argument name.
    pub name: String,
    pub ty: KaiType,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypedFnDecl {
    pub id: FunctionId,
    /// Kept as the LLVM symbol name (entry module); all internal references
    /// use `id`.
    pub name: String,
    /// Owning module's dotted path (`""` = entry). Codegen prefixes it onto
    /// the symbol name, so same-named functions in different modules never
    /// collide in one LLVM module.
    pub module: String,
    pub params: Vec<TypedParam>,
    pub ret: KaiType,
    /// `effects { ... }` declared contract (§5.1.2): `None` = omitted (purely inferred), `Some(empty)` = `effects {}`.
    pub declared_effects: Option<EffectSet>,
    /// Inferred effects `effect(f) = direct_effects(f) ∪ ⋃ effect(g)` (§5.1.2), least-fixed-point over SCCs.
    pub inferred_effects: EffectSet,
    /// `reversible` (§5.3): every Place mutation is transactionally reversible;
    /// external-effect calls must be `compensate`-wrapped. `false` = ordinary.
    pub is_reversible: bool,
    pub body: TypedBlock,
}

impl TypedFnDecl {
    pub fn has_return(&self) -> bool {
        self.body
            .stmts
            .iter()
            .any(|stmt| matches!(stmt, TypedStmt::Return(_)))
    }
}
