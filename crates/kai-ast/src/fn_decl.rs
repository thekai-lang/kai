use crate::ident::Ident;
use crate::param::Param;
use crate::stmt::Block;
use crate::ty::Ty;
use kai_diagnostics::Span;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EffectName {
    EscapesLocalContext,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectSet(pub Vec<EffectName>);

#[derive(Debug, Clone, PartialEq)]
pub struct FnDecl {
    /// `public fn` — visible through an importing module's alias; a plain
    /// `fn` is module-private (§3.6).
    pub is_public: bool,
    pub name: Ident,
    pub params: Vec<Param>,
    pub ret: Ty,
    /// `effects { ... }` verified contract (§5.1.2): `inferred ⊆ declared`, checked, never trusted.
    /// `None` = omitted (purely inferred), `Some(empty)` = `effects {}` declared empty.
    pub effects: Option<EffectSet>,
    /// `reversible` (§5.3): every Place mutation in the body is transactionally
    /// reversible (pre-mutation snapshot) and external-effect calls must be
    /// `compensate`-wrapped. `false` = ordinary transactional-unaware fn.
    pub is_reversible: bool,
    pub body: Block,
    pub span: Span,
}
