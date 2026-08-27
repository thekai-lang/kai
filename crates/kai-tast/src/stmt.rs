use crate::expr::{BinaryOp, TypedExpr};
use crate::ty::KaiType;
use crate::symbol::{LocalId, StructId};

#[derive(Debug, Clone, PartialEq)]
pub struct TypedBlock {
    pub stmts: Vec<TypedStmt>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypedStmt {
    Return(Option<TypedExpr>),
    Let(TypedLet),
    Assign(TypedAssign),
    /// `reversible` ledger push (§5.3.1): captures pre-mutation Place value
    /// before the following `Assign`. Inserted by ownership pass; codegen
    /// emits load + conditional retain + ledger push. The `ty` is the Place's
    /// content type (same as the Assign value's type).
    ReversiblePush(ReversiblePush),
    If(TypedIf),
    For(crate::stmt::TypedFor),
    While(crate::stmt::TypedWhile),
    /// Bare nested block; exists purely for scoping semantics.
    Block(TypedBlock),
    /// `require expr;` (v0.0.8, §5.2) — Correctness Trust, always panics. Parsed in v0.0.7 but not yet effect-checked.
    Require(TypedExpr),
    /// `observe expr;` (v0.0.8, §5.2) — Signal, never panics.
    Observe(TypedExpr),
    Expr(TypedExpr),
    /// Ownership marker (§9.4): the local's heap-bearing value leaves scope
    /// here — codegen emits a release of its current content. Inserted by
    /// the ownership pass at scope ends and on return paths; locals are
    /// released innermost-first.
    ReleaseLocal { local: LocalId, ty: KaiType },
    /// A return whose function exit must release live locals — evaluated
    /// value FIRST, then every release, then control leaves (§9.4 ordering:
    /// the return expression may still read locals it borrows).
    ReturnCleanup {
        value: Option<TypedExpr>,
        /// Innermost-first, reverse declaration order (pass-generated).
        releases: Vec<(LocalId, KaiType)>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypedLet {
    pub local: LocalId,
    /// Kept only for LLVM value names / future diagnostics; uniqueness of
    /// `local` is what codegen relies on.
    pub name: String,
    pub init: TypedExpr,
}

/// One hop of a field-place path (`root.a.b` carries two steps), resolved to
/// the declaring struct and the field's position in it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FieldStep {
    pub struct_id: StructId,
    pub field: u16,
}

/// One hop of the generalized v0.0.5 Place: a field projection or an array
/// index projection. The index EXPRESSION rides along — it evaluates fresh
/// at every assignment site (§9.3).
#[derive(Debug, Clone, PartialEq)]
pub enum TypedPlaceStep {
    Field(FieldStep),
    Index(Box<TypedExpr>),
}

/// `while cond { body }` (v0.0.8.1). `cond_releases` lists the hidden
/// per-iteration temporaries hoisted from the condition — released BOTH on
/// the back-edge and at loop exit (dual release points, §5.2-style care:
/// one release per store, never zero).
#[derive(Debug, Clone, PartialEq)]
pub struct TypedWhile {
    /// Hidden per-iteration temporaries (B1-style hoists) — emitted at the
    /// TOP of `while.cond` so the condition evaluates FRESH every iteration.
    pub cond_prelude: Vec<TypedStmt>,
    pub cond: TypedExpr,
    pub body: crate::stmt::TypedBlock,
    /// Released at BOTH the back-edge (before re-evaluating) and loop exit —
    /// one release per evaluation, never zero.
    pub cond_releases: Vec<(LocalId, KaiType)>,
}

/// `op == None` is plain store; `Some(op)` is read-modify-write
/// (`x += e` lowers to load, add, store). The write goes to `root`, or —
/// when `path` is non-empty — through the field chain starting at `root`.
/// Mutability was enforced upstream against the ROOT binding (§9.3).
///
/// `release_old` marks an owning destination slot (§9.4): codegen must load
/// the old value, prepare the replacement, RELEASE the old value, then
/// store — never release before the RHS exists.
#[derive(Debug, Clone, PartialEq)]
pub struct TypedAssign {
    pub root: LocalId,
    pub path: Vec<TypedPlaceStep>,
    pub op: Option<BinaryOp>,
    pub value: TypedExpr,
    pub release_old: bool,
    /// Source extent of the whole assignment; locates runtime guards it
    /// emits (bounds checks on index hops, arithmetic traps).
    pub span: kai_diagnostics::Span,
}

/// `reversible` ledger push (§5.3.1): captures pre-mutation Place value
/// before the following `Assign`. Heap-bearing snapshots retain(old) so
/// unwind can restore ownership-safe. Inserted by ownership pass; codegen
/// emits the load, conditional retain, and ledger push.
#[derive(Debug, Clone, PartialEq)]
pub struct ReversiblePush {
    pub root: LocalId,
    pub path: Vec<TypedPlaceStep>,
    pub ty: KaiType,
}

/// `for name in array { body }`. `binding_local` is declared ONCE per loop
/// (immutable, element-typed) and re-stored each iteration — the loop
/// variable borrows, it never owns (§9.9).
///
/// `iterable_owned` marks an owned temporary iterable (a call result or
/// literal — not a borrowed binding): the loop takes ownership and releases
/// the array at `for.end`.
#[derive(Debug, Clone, PartialEq)]
pub struct TypedFor {
    pub binding_local: LocalId,
    pub binding_name: String,
    pub iterable: TypedExpr,
    pub body: TypedBlock,
    pub iterable_owned: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypedIf {
    pub cond: TypedExpr,
    pub then_block: TypedBlock,
    /// Absent = no else branch (fall-through).
    pub else_block: Option<TypedBlock>,
}
