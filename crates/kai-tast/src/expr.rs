use crate::symbol::LocalId;
use crate::ty::KaiType;
use kai_diagnostics::Span;

#[derive(Debug, Clone, PartialEq)]
pub struct TypedExpr {
    pub kind: TypedExprKind,
    pub ty: KaiType,
    /// Source span of the whole expression (byte offsets). Runtime panics
    /// (§10.1) report `file:line:col` resolved from it at emission time;
    /// tests and pass-generated wrappers may leave the zero span.
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Lt,
    Gt,
    Le,
    Ge,
    Eq,
    Ne,
    And,
    Or,
}

impl BinaryOp {
    pub fn describe(self) -> &'static str {
        match self {
            BinaryOp::Add => "+",
            BinaryOp::Sub => "-",
            BinaryOp::Mul => "*",
            BinaryOp::Div => "/",
            BinaryOp::Mod => "%",
            BinaryOp::Lt => "<",
            BinaryOp::Gt => ">",
            BinaryOp::Le => "<=",
            BinaryOp::Ge => ">=",
            BinaryOp::Eq => "==",
            BinaryOp::Ne => "!=",
            BinaryOp::And => "&&",
            BinaryOp::Or => "||",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypedExprKind {
    /// Value fits the width named by `ty` (range-checked by the type checker).
    IntLit(i64),
    FloatLit(f64),
    BoolLit(bool),
    LocalRef(LocalId),
    Neg(Box<TypedExpr>),
    Not(Box<TypedExpr>),
    Binary {
        op: BinaryOp,
        lhs: Box<TypedExpr>,
        rhs: Box<TypedExpr>,
        /// Let statements collected from owned-temp hoisting inside the rhs
        /// of `&&`/`||`. These must be emitted inside the short-circuit rhs
        /// basic block (not the outer scope) so the allocation + release only
        /// execute when the rhs branch is actually taken, preserving
        /// short-circuit semantics.
        rhs_hoists: Vec<crate::TypedStmt>,
    },
    /// Poisoned node carried over from parser recovery. Only ever present in
    /// programs that already failed; codegen lowers it to `undef` so every
    /// match stays total and no phase can mistake it for real code.
    Invalid,
    /// `base.field`, resolved to a slot index in the struct layout. The
    /// result type is the field's declared type; loads copy (§9.3).
    FieldAccess {
        base: Box<TypedExpr>,
        struct_id: crate::symbol::StructId,
        /// Position of the field in declaration order.
        field: u16,
    },
    /// `Name { f: e, .. }` with values in FIELD DECLARATION order and
    /// completeness already enforced by the type checker.
    StructLit {
        struct_id: crate::symbol::StructId,
        values: Vec<TypedExpr>,
    },
    /// `"text"` — escapes decoded upstream (§9.7). Owns its heap bytes;
    /// the ownership pass decides retain/release placement around it.
    StrLit {
        value: String,
    },
    /// `[e0, e1, ..]` with every element already unified to one type
    /// (`self.ty` = `Array(elem)`). Empty literals were rejected upstream
    /// unless a context type existed.
    ArrayLit {
        elements: Vec<TypedExpr>,
    },
    /// `base[index]` — a BORROW read (§9.9): the element is copied out,
    /// ownership of the array itself never moves.
    Index {
        base: Box<TypedExpr>,
        index: Box<TypedExpr>,
    },
    /// Direct call to a top-level function; argument types match the
    /// signature exactly, result type is `self.ty`.
    Call {
        func: crate::symbol::FunctionId,
        args: Vec<TypedExpr>,
    },
    /// Ownership marker (§9.5): the inner value is BORROWED (a param, local,
    /// field access, or array element) and is entering an owning slot —
    /// codegen emits a retain and forwards the pointer unchanged. Inserted
    /// only by the ownership pass; never constructed by the type checker.
    Retain(Box<TypedExpr>),
    // -- v0.0.6 (§9.9a/§9.10) + v0.14 Ok/Err --------------------------------
    /// `Some(value)` — payload already unified; `self.ty` = `Optional(t)`.
    SomeLit(Box<TypedExpr>),
    /// Bare `None`. Carries no payload; `self.ty` was fixed by context.
    NoneLit,
    /// `Ok(value)` — payload already unified; `self.ty` = `Result(ok, err)` where `ok` unified, `err` from context.
    OkLit(Box<TypedExpr>),
    /// `Err(value)` — payload already unified; `self.ty` = `Result(ok, err)` where `err` unified, `ok` from context.
    ErrLit(Box<TypedExpr>),
    /// `lhs ?? rhs` — rhs evaluates ONLY when lhs is None (lazy lowering).
    /// Both sides share the payload type; result type is that payload.
    Coalesce { lhs: Box<TypedExpr>, rhs: Box<TypedExpr> },
    /// `receiver.unwrap_or(default)` — the builtin combinator resolved by
    /// the type checker from an ordinary FieldAccess+Call shape (§9.9a).
    /// Receiver is `Optional<T>` or `Result<T, E>`; result is `T`.
    UnwrapOr { receiver: Box<TypedExpr>, default: Box<TypedExpr> },
    /// `base catch |err| { stmts.. tail }` — Result-only (§3.4). The err
    /// binding is a BORROW of the Err payload (never retained/released as
    /// an owner); it lives for the catch block only. Result type = ok type.
    Catch {
        base: Box<TypedExpr>,
        err_binding: crate::symbol::LocalId,
        err_ty: KaiType,
        stmts: Vec<crate::stmt::TypedStmt>,
        tail: Box<TypedExpr>,
        /// Locals declared by the catch block, released after `tail`
        /// evaluates (ownership pass fills this; codegen emits them).
        releases: Vec<(crate::symbol::LocalId, KaiType)>,
    },
    /// Call through a closure VALUE (`f(x)` where `f: Closure{..}`, v0.0.6):
    /// argument/result types already unified against the signature.
    CallIndirect {
        callee: Box<TypedExpr>,
        args: Vec<TypedExpr>,
    },
    /// Closure literal (v0.0.6). The body lowers into its own scope; the
    /// capture list holds every OUTER local referenced inside, in first-use
    /// order. Heap env allocation + fat-pointer ABI are codegen's job.
    ClosureLit(Box<TypedClosure>),
}

/// One captured outer binding of a closure literal (§9.10): retained into
/// the environment at construction, released via the env destructor when
/// the environment's refcount reaches zero.
#[derive(Debug, Clone, PartialEq)]
pub struct TypedCapture {
    pub local: crate::symbol::LocalId,
    pub ty: KaiType,
}

/// A lowered closure literal: params are plain locals of the body scope;
/// captures are the outer bindings it closes over.
#[derive(Debug, Clone, PartialEq)]
pub struct TypedClosure {
    pub param_ids: Vec<crate::symbol::LocalId>,
    pub body: crate::stmt::TypedBlock,
    pub captures: Vec<TypedCapture>,
}

impl TypedExpr {
    /// Untyped-span constructor: tests and ownership-pass wrappers that
    /// carry an existing expression's position implicitly.
    pub fn new(kind: TypedExprKind, ty: KaiType) -> Self {
        Self::new_at(kind, ty, Span::new(0, 0))
    }

    /// Full constructor used by the type checker, which knows the AST span.
    pub fn new_at(kind: TypedExprKind, ty: KaiType, span: Span) -> Self {
        Self { kind, ty, span }
    }

    /// Integer literal constructor that picks the right width.
    pub fn int_lit(value: i64, ty: KaiType) -> Self {
        Self::new(TypedExprKind::IntLit(value), ty)
    }
}
