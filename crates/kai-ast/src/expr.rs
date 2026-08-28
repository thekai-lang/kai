use crate::ident::Ident;
use crate::param::Param;
use crate::stmt::{Block, Stmt};
use crate::ty::Ty;
use kai_diagnostics::Span;

/// Binary operators. Precedence lives in the parser; this enum only carries
/// which operator was written.
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Neg,
    Not,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IntLit {
    pub value: u64,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FloatLit {
    pub value: f64,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UnaryExpr {
    pub op: UnaryOp,
    pub op_span: Span,
    pub operand: Box<Expr>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BinaryExpr {
    pub op: BinaryOp,
    pub op_span: Span,
    pub lhs: Box<Expr>,
    pub rhs: Box<Expr>,
}

/// `callee(args)` — v0.0.3 restricts valid callees to top-level functions.
#[derive(Debug, Clone, PartialEq)]
pub struct CallExpr {
    pub callee: Box<Expr>,
    pub args: Vec<Expr>,
}

/// `base.field` — struct field read (v0.0.3); chains nest naturally
/// (`line.start.x` is FieldAccess(FieldAccess(Ident, start), x)).
#[derive(Debug, Clone, PartialEq)]
pub struct FieldAccessExpr {
    pub base: Box<Expr>,
    pub field: Ident,
}

/// `Name { field: expr, ... }` — struct literal (v0.0.3).
#[derive(Debug, Clone, PartialEq)]
pub struct StructLitExpr {
    /// Dotted head (`QualifiedName`): len 1 is the plain `Point { .. }`
    /// form; longer paths are module-qualified (`math.Point { .. }`).
    /// Whether the qualifier names a module is decided by the resolver.
    pub path: Vec<Ident>,
    pub fields: Vec<FieldInit>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FieldInit {
    pub name: Ident,
    pub value: Expr,
}

/// `[e0, e1, ..]` — array literal (v0.0.5). An empty form needs a context
/// type to fix its element type (§9.7); the type checker enforces that.
#[derive(Debug, Clone, PartialEq)]
pub struct ArrayLitExpr {
    pub elements: Vec<Expr>,
}

/// `"text"` — string literal with escapes already DECODED by the lexer
/// (v0.0.5, plain literals only; `${...}` is deferred grammar).
#[derive(Debug, Clone, PartialEq)]
pub struct StrLitExpr {
    pub value: String,
}

/// `base[index]` — array element read (v0.0.5). A borrow: it never touches
/// ownership (§9.9).
#[derive(Debug, Clone, PartialEq)]
pub struct IndexExpr {
    pub base: Box<Expr>,
    pub index: Box<Expr>,
    /// Closing bracket span, for diagnostics pointing at the projection.
    pub rbracket: Span,
}

/// `Some(expr)` — Optional construction (v0.0.6, §9.9a).
#[derive(Debug, Clone, PartialEq)]
pub struct SomeLitExpr {
    pub value: Box<Expr>,
}

/// `Ok(expr)` — Result Ok construction (v0.14, §3.4), parallel to `Some`.
#[derive(Debug, Clone, PartialEq)]
pub struct OkLitExpr {
    pub value: Box<Expr>,
}

/// `Err(expr)` — Result Err construction (v0.14, §3.4).
#[derive(Debug, Clone, PartialEq)]
pub struct ErrLitExpr {
    pub value: Box<Expr>,
}

/// `lhs ?? rhs` (v0.0.6, §9.9a) — right-associative; the right side only
/// evaluates when the left side is `None` (lazy lowering).
#[derive(Debug, Clone, PartialEq)]
pub struct CoalesceExpr {
    pub lhs: Box<Expr>,
    pub rhs: Box<Expr>,
}

/// `base catch |err| { stmts.. tail }` (v0.0.6, §3.4) — Result-only error
/// branch. The block is a CatchBlock: ordinary statements, then ONE
/// mandatory trailing value expression without `;`.
#[derive(Debug, Clone, PartialEq)]
pub struct CatchExpr {
    pub base: Box<Expr>,
    pub err_binding: Ident,
    /// Statements before the trailing value expression.
    pub stmts: Vec<Stmt>,
    pub tail: Box<Expr>,
}

/// `base compensate { stmts }` (v0.0.9, §5.3) — an external-effect
/// compensation block attached to a call inside a `reversible` function. Mirrors
/// `Catch`'s postfix-block shape, but the block is statement-only (no trailing
/// value expression): the compensation is executed on unwind, never produces a
/// value at the call site.
#[derive(Debug, Clone, PartialEq)]
pub struct CompensateExpr {
    pub base: Box<Expr>,
    /// Compensation statements, executed on unwind in reverse.
    pub stmts: Vec<Stmt>,
}

/// `fn(params) -> ret { body }` — closure literal (v0.0.6, §3.5). The value
/// syntax keeps its `fn` head; only the closure TYPE dropped it.
#[derive(Debug, Clone, PartialEq)]
pub struct ClosureLitExpr {
    pub params: Vec<Param>,
    pub ret: Ty,
    pub body: Block,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExprKind {
    IntLit(IntLit),
    FloatLit(FloatLit),
    BoolLit {
        value: bool,
        span: Span,
    },
    Ident(Ident),
    Unary(UnaryExpr),
    Binary(BinaryExpr),
    Call(CallExpr),
    FieldAccess(FieldAccessExpr),
    StructLit(StructLitExpr),
    ArrayLit(ArrayLitExpr),
    Index(IndexExpr),
    StrLit(StrLitExpr),
    // v0.0.6 (§9.9a/§3.4/§3.5) + v0.14 Ok/Err
    SomeLit(SomeLitExpr),
    /// Bare `None` — the payload-less Optional constructor. It carries no
    /// type information, so a context type must fix `T` (typecheck rule,
    /// same pattern as the empty array literal).
    NoneLit,
    OkLit(OkLitExpr),
    ErrLit(ErrLitExpr),
    Coalesce(CoalesceExpr),
    Catch(CatchExpr),
    ClosureLit(ClosureLitExpr),
    /// v0.0.9 (§5.3) — postfix compensation block on a call inside a
    /// `reversible` function.
    Compensate(CompensateExpr),
    DslBlock(DslBlockExpr),
    /// Poisoned node produced only by parser error recovery (e.g. an
    /// expression nested past the recursion budget). Downstream phases treat
    /// it as an error marker, never as compilable code.
    Invalid,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DslBlockExpr {
    pub kind: String, // "sql" or "api"
    pub variant: DslVariant,
    pub version: u32,
    pub return_ty: Option<Box<Ty>>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DslVariant {
    StructuredSql(SqlQuery),
    Raw(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct SqlQuery {
    pub select: Vec<SqlSelectExpr>,
    pub from: SqlTableRef,
    pub joins: Vec<SqlJoin>,
    pub where_clause: Option<SqlExpr>,
    pub group_by: Vec<String>,
    pub order_by: Vec<SqlOrderBy>,
    pub limit: Option<u64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SqlTableRef {
    pub name: String,
    pub span: kai_diagnostics::Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SqlSelectExpr {
    pub expr: SqlExpr,
    pub alias: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SqlJoin {
    pub table: SqlTableRef,
    pub on_clause: SqlExpr,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SqlOrderBy {
    pub expr: SqlExpr,
    pub descending: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SqlOp {
    Eq,
    NotEq,
    Lt,
    Gt,
    Le,
    Ge,
    And,
    Or,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SqlExpr {
    Column {
        qualifier: Option<String>,
        name: String,
        span: kai_diagnostics::Span,
    },
    StringLit { value: String, span: kai_diagnostics::Span },
    IntLit { value: i64, span: kai_diagnostics::Span },
    BoolLit { value: bool, span: kai_diagnostics::Span },
    Variable(Ident),
    BinaryOp(Box<SqlExpr>, SqlOp, Box<SqlExpr>),
}
