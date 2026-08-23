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
