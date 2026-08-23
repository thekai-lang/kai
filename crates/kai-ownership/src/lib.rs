//! Ownership resolution (§9): the explicit IR-producing phase between
//! typecheck and codegen. Every retain/release/move decision is materialized
//! as a TAST node here — `TypedExprKind::Retain`, `TypedStmt::ReleaseLocal`,
//! `TypedAssign::release_old` — so codegen reads mechanically and never
//! infers ownership itself (§8, constraint 2).
//!
//! The model (§9.4–9.9):
//! - Locals (`let`/`var`) OWN their slot contents; parameters BORROW.
//! - Reading any binding yields a borrowed reference (the source keeps
//!   ownership); only fresh allocations are owned temporaries: string
//!   literals, array literals, struct literals, call results.
//! - Entering an OWNING SLOT with a borrowed value inserts a retain
//!   (co-ownership). Owning slots: `let`/`var` inits, assignment targets,
//!   return values of heap-typed functions, struct-literal fields, array-
//!   literal elements.
//! - Replacing an owning slot's content releases the OLD value after the
//!   replacement is prepared (§9.4 ordering, E4).
//! - Scope exit releases locals innermost-first, reverse declaration order;
//!   return paths release every enclosing frame before leaving.
//! - Owned temporaries in BORROW positions (call arguments, comparison and
//!   projection operands, discarded statement values) are materialized into
//!   hidden `$tmp` locals so scope machinery releases them; without this
//!   they would leak. `&&`/`||` subtrees are exempt — hoisting would defeat
//!   short-circuiting.
//! - `for..in` over an owned temporary binds it to a hidden `$iter` local in
//!   the loop's scope frame: released at loop exit AND on returns from the
//!   body (the old flag-based path could leak on early return). The loop
//!   variable never owns (E7).

use kai_tast::{
    BinaryOp, KaiType, LocalId, TypedAssign, TypedBlock, TypedExpr, TypedExprKind, TypedFnDecl,
    TypedFor, TypedProgram, TypedStmt,
};

/// Runs the pass over a typechecked program, annotating it in place.
pub fn resolve(program: &mut TypedProgram) {
    let heap = HeapBearing::new(&program.structs);
    let mut fresh = FreshIds::seeded_beyond(program);
    for fns in &mut program.fns {
        resolve_fn(&heap, fns, &mut fresh);
    }
}

/// Allocates local ids beyond everything present in the source tree, used
/// for hidden locals introduced by this pass (hoisted temporaries, owned
/// `for` iterables).
struct FreshIds {
    next: u32,
}

impl FreshIds {
    fn seeded_beyond(program: &kai_tast::TypedProgram) -> Self {
        let mut max = 0;
        for decl in &program.fns {
            for p in &decl.params {
                max = max.max(p.local.0);
            }
            seed_stmts(&decl.body.stmts, &mut max);
        }
        Self { next: max + 1 }
    }

    fn alloc(&mut self) -> LocalId {
        let id = LocalId(self.next);
        self.next += 1;
        id
    }
}

fn seed_stmts(stmts: &[TypedStmt], max: &mut u32) {
    for s in stmts {
        match s {
            TypedStmt::Let(b) => {
                *max = (*max).max(b.local.0);
                seed_expr(&b.init, max);
            }
            TypedStmt::Assign(a) => {
                *max = (*max).max(a.root.0);
                for step in &a.path {
                    if let kai_tast::TypedPlaceStep::Index(idx) = step {
                        seed_expr(idx, max);
                    }
                }
                seed_expr(&a.value, max);
            }
            TypedStmt::If(i) => {
                seed_expr(&i.cond, max);
                seed_stmts(&i.then_block.stmts, max);
                if let Some(b) = &i.else_block {
                    seed_stmts(&b.stmts, max);
                }
            }
            TypedStmt::For(f) => {
                *max = (*max).max(f.binding_local.0);
                seed_expr(&f.iterable, max);
                seed_stmts(&f.body.stmts, max);
            }
            TypedStmt::Block(b) => seed_stmts(&b.stmts, max),
            TypedStmt::Expr(e) | TypedStmt::Return(Some(e)) => seed_expr(e, max),
            TypedStmt::Return(None)
            | TypedStmt::ReleaseLocal { .. }
            | TypedStmt::ReturnCleanup { .. } => {}
        }
    }
}

fn seed_expr(expr: &TypedExpr, max: &mut u32) {
    match &expr.kind {
        TypedExprKind::LocalRef(id) => *max = (*max).max(id.0),
        TypedExprKind::Neg(inner) | TypedExprKind::Not(inner) | TypedExprKind::Retain(inner) => {
            seed_expr(inner, max)
        }
        TypedExprKind::Binary { lhs, rhs, .. } => {
            seed_expr(lhs, max);
            seed_expr(rhs, max);
        }
        TypedExprKind::FieldAccess { base, .. } => seed_expr(base, max),
        TypedExprKind::Index { base, index } => {
            seed_expr(base, max);
            seed_expr(index, max);
        }
        TypedExprKind::StructLit { values, .. } | TypedExprKind::ArrayLit { elements: values } => {
            for v in values {
                seed_expr(v, max);
            }
        }
        TypedExprKind::Call { args, .. } => {
            for a in args {
                seed_expr(a, max);
            }
        }
        TypedExprKind::IntLit(_)
        | TypedExprKind::FloatLit(_)
        | TypedExprKind::BoolLit(_)
        | TypedExprKind::StrLit { .. }
        | TypedExprKind::Invalid => {}
    }
}

/// Precomputed "does this type own heap memory" table (§9.1 + §9.5):
/// `string` and arrays always; structs iff any field is heap-bearing,
/// recursively. Cycles are impossible — the resolver rejects them, and
/// arrays break any potential cycle by indirection.
struct HeapBearing {
    struct_heap: Vec<bool>,
}

impl HeapBearing {
    fn new(structs: &[kai_tast::TypedStruct]) -> Self {
        // Fixed-point iteration: assume no struct is heap-bearing, then
        // propagate until stable. Forward field references are legal, so
        // one pass is not enough — iterate to a fixpoint.
        let mut heap = Self {
            struct_heap: vec![false; structs.len()],
        };
        loop {
            let mut changed = false;
            for (idx, ts) in structs.iter().enumerate() {
                if heap.struct_heap[idx] {
                    continue;
                }
                if ts.fields.iter().any(|f| heap.is(&f.ty)) {
                    heap.struct_heap[idx] = true;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        heap
    }

    fn is(&self, ty: &KaiType) -> bool {
        match ty {
            KaiType::String | KaiType::Array(_) => true,
            KaiType::Struct(id) => self.struct_heap[id.0 as usize],
            KaiType::Int32
            | KaiType::Int64
            | KaiType::Float64
            | KaiType::Bool
            | KaiType::Unit => false,
        }
    }
}

/// Ownership class of an evaluated expression (§9.5 summary table).
fn is_owned_temp(expr: &TypedExpr) -> bool {
    match &expr.kind {
        // Fresh allocations / transferred results.
        TypedExprKind::StrLit { .. }
        | TypedExprKind::ArrayLit { .. }
        | TypedExprKind::StructLit { .. }
        | TypedExprKind::Call { .. } => true,
        // Everything else borrows: bindings, projections, scalars, poison.
        _ => false,
    }
}

/// Owning slot fed by a borrowed value (§9.5 row 2): swap the expression
/// for a placeholder, rewrap it in a `Retain` marker that carries the
/// inner span. Shared by every owning-slot site (returns, `let`, plain
/// assignment, literal fields/elements).
fn wrap_retain_if_borrowed(heap: &HeapBearing, e: &mut TypedExpr) {
    if heap.is(&e.ty) && !is_owned_temp(e) {
        let ty = e.ty.clone();
        let span = e.span;
        let inner = std::mem::replace(e, TypedExpr::new(TypedExprKind::Invalid, ty.clone()));
        *e = TypedExpr::new_at(TypedExprKind::Retain(Box::new(inner)), ty, span);
    }
}

fn resolve_fn(heap: &HeapBearing, decl: &mut TypedFnDecl, fresh: &mut FreshIds) {
    // Frame 0: parameters — they borrow, so they are never registered for
    // release (the callee does not release what it does not own, §9.3).
    let mut scopes = Scopes::default();
    scopes.push();
    for param in &decl.params {
        scopes.declare(param.local, param.ty.clone(), false);
    }
    let body = std::mem::replace(&mut decl.body, TypedBlock { stmts: Vec::new() });
    decl.body = walk_block(heap, body, &mut scopes, fresh);
}

#[derive(Default)]
struct Scopes {
    /// Per open block: (local id, type) pairs in declaration order.
    frames: Vec<Vec<(LocalId, KaiType)>>,
}

impl Scopes {
    fn push(&mut self) {
        self.frames.push(Vec::new());
    }

    fn pop(&mut self) -> Vec<(LocalId, KaiType)> {
        self.frames.pop().expect("scope underflow")
    }

    fn declare(&mut self, local: LocalId, ty: KaiType, tracked: bool) {
        if tracked {
            self.frames.last_mut().expect("open scope").push((local, ty));
        }
    }

    /// (local, type) pairs for ALL open frames, innermost first, reverse
    /// declaration order — used on `return` paths where the whole function
    /// unwinds.
    fn releases_all(&self) -> Vec<(LocalId, KaiType)> {
        let mut out = Vec::new();
        for frame in self.frames.iter().rev() {
            for (local, ty) in frame.iter().rev() {
                out.push((*local, ty.clone()));
            }
        }
        out
    }
}

fn push_frame_releases(frame: &[(LocalId, KaiType)], out: &mut Vec<TypedStmt>) {
    for (local, ty) in frame.iter().rev() {
        out.push(TypedStmt::ReleaseLocal {
            local: *local,
            ty: ty.clone(),
        });
    }
}

fn walk_block(
    heap: &HeapBearing,
    mut block: TypedBlock,
    scopes: &mut Scopes,
    fresh: &mut FreshIds,
) -> TypedBlock {
    scopes.push();
    let mut out = Vec::with_capacity(block.stmts.len());
    for stmt in std::mem::take(&mut block.stmts) {
        match stmt {
            // Return paths release every live local before leaving — but
            // AFTER the return value exists (it may still read locals it
            // borrows; the §9.5 retain on the value keeps heap content
            // alive past the releases). One node carries both.
            ret @ TypedStmt::Return(_) => {
                let ret = finish_return(heap, ret);
                let TypedStmt::Return(value) = ret else {
                    unreachable!("finish_return returns a Return");
                };
                out.push(TypedStmt::ReturnCleanup {
                    value,
                    releases: scopes.releases_all(),
                });
                // The block has terminated: nothing after this executes,
                // and emitting anything past a terminator would produce
                // invalid IR. Remaining source statements are dead code.
                break;
            }
            TypedStmt::ReturnCleanup { .. } => unreachable!("pass-generated node"),
            other => out.extend(walk_stmt(heap, other, scopes, fresh)),
        }
    }
    // Normal block end: this frame's locals, reverse declaration order.
    // Skipped when a return already terminated the block.
    if !matches!(out.last(), Some(TypedStmt::ReturnCleanup { .. })) {
        let frame = scopes.pop();
        push_frame_releases(&frame, &mut out);
    } else {
        scopes.pop();
    }
    block.stmts = out;
    block
}

fn finish_return(heap: &HeapBearing, ret: TypedStmt) -> TypedStmt {
    let TypedStmt::Return(value) = ret else {
        unreachable!("finish_return on non-return")
    };
    let value = match value {
        Some(mut e) => {
            // Descend first: nested owning slots (struct-literal fields,
            // array-literal elements) need their own markers before we
            // decide whether the result itself needs retaining.
            walk_expr(heap, &mut e);
            wrap_retain_if_borrowed(heap, &mut e);
            Some(e)
        }
        None => None,
    };
    TypedStmt::Return(value)
}

fn walk_stmt(
    heap: &HeapBearing,
    stmt: TypedStmt,
    scopes: &mut Scopes,
    fresh: &mut FreshIds,
) -> Vec<TypedStmt> {
    match stmt {
        TypedStmt::Let(mut binding) => {
            // Borrow-position temporaries inside the initializer are
            // materialized first (in evaluation order); the root itself is
            // a transfer into the owning slot.
            let mut out = Vec::new();
            hoist_borrow_temps(heap, &mut binding.init, fresh, scopes, &mut out, true);
            walk_expr(heap, &mut binding.init);
            // Owning slot: co-own borrowed sources (§9.4/§9.5 row 3).
            wrap_retain_if_borrowed(heap, &mut binding.init);
            scopes.declare(
                binding.local,
                binding.init.ty.clone(),
                heap.is(&binding.init.ty),
            );
            out.push(TypedStmt::Let(binding));
            out
        }
        TypedStmt::Assign(mut assign) => {
            let mut out = Vec::new();
            // Place steps emit before the value at codegen; hoist their
            // index temporaries in that same order.
            for step in assign.path.iter_mut() {
                if let kai_tast::TypedPlaceStep::Index(idx) = step {
                    hoist_borrow_temps(heap, idx, fresh, scopes, &mut out, false);
                }
            }
            hoist_borrow_temps(heap, &mut assign.value, fresh, scopes, &mut out, true);
            out.push(TypedStmt::Assign(walk_assign(heap, assign)));
            out
        }
        TypedStmt::If(mut if_) => {
            let mut out = Vec::new();
            hoist_borrow_temps(heap, &mut if_.cond, fresh, scopes, &mut out, false);
            walk_expr(heap, &mut if_.cond);
            if_.then_block = walk_block(heap, if_.then_block, scopes, fresh);
            if_.else_block =
                if_.else_block.map(|b| walk_block(heap, b, scopes, fresh));
            out.push(TypedStmt::If(if_));
            out
        }
        TypedStmt::For(f) => {
            let (f, pre, end_releases) = walk_for(heap, f, scopes, fresh);
            let mut out = pre;
            out.push(TypedStmt::For(f));
            push_frame_releases(&end_releases, &mut out);
            out
        }
        TypedStmt::Block(block) => {
            vec![TypedStmt::Block(walk_block(heap, block, scopes, fresh))]
        }
        TypedStmt::Expr(mut e) => {
            let mut out = Vec::new();
            if heap.is(&e.ty) && is_owned_temp(&e) {
                // A heap value computed and thrown away: bind it to a
                // hidden local so scope exit releases it (the statement has
                // no other consumer).
                hoist_borrow_temps(heap, &mut e, fresh, scopes, &mut out, false);
                return out;
            }
            hoist_borrow_temps(heap, &mut e, fresh, scopes, &mut out, false);
            walk_expr(heap, &mut e);
            out.push(TypedStmt::Expr(e));
            out
        }
        // Handled by the caller (return needs surrounding-scope context).
        TypedStmt::Return(_) => unreachable!("returns handled by walk_block"),
        TypedStmt::ReleaseLocal { .. } | TypedStmt::ReturnCleanup { .. } => {
            unreachable!("nodes are pass-generated")
        }
    }
}

/// Materializes owned temporaries sitting in BORROW positions into hidden
/// locals declared in the current scope; the ordinary scope machinery then
/// releases them at block exit and on early returns. Without this, values
/// like a string literal passed to a function leak — nobody owns them after
/// the consuming statement ends.
///
/// `root_is_transfer` marks positions whose top node MOVES instead of
/// borrowing (initializer/assignment RHS roots): there the root stays put
/// and only nested borrow positions are rewritten.
///
/// Skipped deliberately:
/// - `&&`/`||` subtrees — hoisting would evaluate the rhs temp even when
///   short-circuited; needs real materialization nodes (v0.0.6).
/// - Struct/array literal members — those are OWNING slots already handled
///   by the retain wrappers.
fn hoist_borrow_temps(
    heap: &HeapBearing,
    expr: &mut TypedExpr,
    fresh: &mut FreshIds,
    scopes: &mut Scopes,
    out: &mut Vec<TypedStmt>,
    root_is_transfer: bool,
) {
    if !root_is_transfer && heap.is(&expr.ty) && is_owned_temp(expr) {
        let local = fresh.alloc();
        let ty = expr.ty.clone();
        let init =
            std::mem::replace(expr, TypedExpr::new(TypedExprKind::LocalRef(local), ty.clone()));
        scopes.declare(local, ty, true);
        out.push(TypedStmt::Let(kai_tast::TypedLet {
            local,
            name: "$tmp".into(),
            init,
        }));
        return;
    }
    match &mut expr.kind {
        // Guarded by control flow: leave untouched (see doc comment).
        TypedExprKind::Binary { op: BinaryOp::And | BinaryOp::Or, .. } => {}
        TypedExprKind::Binary { lhs, rhs, .. } => {
            hoist_borrow_temps(heap, lhs, fresh, scopes, out, false);
            hoist_borrow_temps(heap, rhs, fresh, scopes, out, false);
        }
        TypedExprKind::Call { args, .. } => {
            for a in args.iter_mut() {
                hoist_borrow_temps(heap, a, fresh, scopes, out, false);
            }
        }
        TypedExprKind::Index { base, index } => {
            hoist_borrow_temps(heap, base, fresh, scopes, out, false);
            hoist_borrow_temps(heap, index, fresh, scopes, out, false);
        }
        TypedExprKind::FieldAccess { base, .. } => {
            hoist_borrow_temps(heap, base, fresh, scopes, out, false)
        }
        // Scalar contexts and pre-wrapped nodes hold nothing to hoist;
        // literals' members are owning slots.
        _ => {}
    }
}

fn walk_assign(heap: &HeapBearing, mut assign: TypedAssign) -> TypedAssign {
    walk_expr(heap, &mut assign.value);

    assign.release_old = assign.op.is_none() && heap.is(&assign.value.ty);

    // Owning slot: retain borrowed replacements (§9.5). Compound ops exist
    // only on numeric (non-heap) slots in v0.0.5, so this is plain stores
    // only.
    if assign.op.is_none() {
        wrap_retain_if_borrowed(heap, &mut assign.value);
    }
    assign
}

/// Rewrites one `for..in`. An OWNED temporary iterable is materialized into
/// a hidden local declared in the loop's own scope frame: normal completion
/// releases it at `for.end` (the frame pops after the body), and a `return`
/// inside the body releases it through the cleanup chain (B3 fix). The old
/// `iterable_owned` flag path is retired — ownership now rides the same
/// machinery as every local.
///
/// Returns (rewritten loop, statements before it, releases for after it).
fn walk_for(
    heap: &HeapBearing,
    mut f: TypedFor,
    scopes: &mut Scopes,
    fresh: &mut FreshIds,
) -> (TypedFor, Vec<TypedStmt>, Vec<(LocalId, KaiType)>) {
    scopes.push();
    let mut pre = Vec::new();
    walk_expr(heap, &mut f.iterable);
    if is_owned_temp(&f.iterable) && heap.is(&f.iterable.ty) {
        let local = fresh.alloc();
        let ty = f.iterable.ty.clone();
        let init = std::mem::replace(
            &mut f.iterable,
            TypedExpr::new(TypedExprKind::LocalRef(local), ty.clone()),
        );
        scopes.declare(local, ty, true);
        pre.push(TypedStmt::Let(kai_tast::TypedLet {
            local,
            name: "$iter".into(),
            init,
        }));
    }
    // The hidden local (or the original owner, for borrowed iterables)
    // carries the release duty; the flag has no further job.
    f.iterable_owned = false;
    f.body = walk_block(heap, f.body, scopes, fresh);
    let frame = scopes.pop();
    (f, pre, frame)
}

fn walk_expr(heap: &HeapBearing, expr: &mut TypedExpr) {
    match &mut expr.kind {
        TypedExprKind::IntLit(_) | TypedExprKind::FloatLit(_) | TypedExprKind::BoolLit(_)
        | TypedExprKind::LocalRef(_) | TypedExprKind::StrLit { .. } | TypedExprKind::Invalid => {}
        TypedExprKind::Neg(inner) | TypedExprKind::Not(inner) | TypedExprKind::Retain(inner) => {
            walk_expr(heap, inner)
        }
        TypedExprKind::Binary { op: _, lhs, rhs } => {
            walk_expr(heap, lhs);
            walk_expr(heap, rhs);
        }
        TypedExprKind::FieldAccess { base, .. } => walk_expr(heap, base),
        TypedExprKind::Index { base, index } => {
            walk_expr(heap, base);
            walk_expr(heap, index);
        }
        // Literal fields/elements ARE owning slots: retain borrowed sources
        // at construction (§9.5 wrap()/pair() examples).
        TypedExprKind::StructLit { values, .. } => {
            for v in values.iter_mut() {
                walk_expr(heap, v);
                wrap_retain_if_borrowed(heap, v);
            }
        }
        TypedExprKind::ArrayLit { elements } => {
            for e in elements.iter_mut() {
                walk_expr(heap, e);
                wrap_retain_if_borrowed(heap, e);
            }
        }
        // Call arguments are BORROWED (§9.6): no retain, but nested
        // expressions inside argument positions still get walked.
        TypedExprKind::Call { args, .. } => {
            for a in args.iter_mut() {
                walk_expr(heap, a);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kai_tast::{BinaryOp, StructId, TypedStruct, TypedStructField};

    // ---------- hand-built TAST helpers ----------

    fn str_lit(s: &str) -> TypedExpr {
        TypedExpr::new(TypedExprKind::StrLit { value: s.into() }, KaiType::String)
    }

    fn int_lit(v: i64) -> TypedExpr {
        TypedExpr::new(TypedExprKind::IntLit(v), KaiType::Int32)
    }

    fn local_ref(id: u32, ty: KaiType) -> TypedExpr {
        TypedExpr::new(TypedExprKind::LocalRef(LocalId(id)), ty)
    }

    fn let_(id: u32, name: &str, init: TypedExpr) -> TypedStmt {
        TypedStmt::Let(kai_tast::TypedLet {
            local: LocalId(id),
            name: name.into(),
            init,
        })
    }

    fn assign(root: u32, path: Vec<kai_tast::TypedPlaceStep>, value: TypedExpr) -> TypedAssign {
        TypedAssign {
            root: LocalId(root),
            path,
            op: None,
            value,
            release_old: false,
            span: kai_diagnostics::Span::new(0, 0),
        }
    }

    fn ret(e: Option<TypedExpr>) -> TypedStmt {
        TypedStmt::Return(e)
    }

    fn block(stmts: Vec<TypedStmt>) -> TypedBlock {
        TypedBlock { stmts }
    }

    fn fn_decl(body: TypedBlock, params: Vec<kai_tast::TypedParam>, ret_ty: KaiType) -> TypedFnDecl {
        TypedFnDecl {
            id: kai_tast::FunctionId(0),
            name: "main".into(),
            module: String::new(),
            params,
            ret: ret_ty,
            body,
        }
    }

    fn param(id: u32, name: &str, ty: KaiType) -> kai_tast::TypedParam {
        kai_tast::TypedParam {
            local: LocalId(id),
            name: name.into(),
            ty,
        }
    }

    fn run(mut program: TypedProgram) -> TypedProgram {
        resolve(&mut program);
        program
    }

    fn unwrap_retain(e: &TypedExpr) -> bool {
        matches!(e.kind, TypedExprKind::Retain(_))
    }

    // ---------- heap-bearing table ----------

    #[test]
    fn heap_bearing_classification() {
        let structs = vec![
            TypedStruct {
                name: "Plain".into(),
                module: String::new(),
                fields: vec![
                    TypedStructField { name: "x".into(), ty: KaiType::Int32 },
                    TypedStructField { name: "y".into(), ty: KaiType::Bool },
                ],
            },
            TypedStruct {
                name: "Bearing".into(),
                module: String::new(),
                fields: vec![TypedStructField { name: "s".into(), ty: KaiType::String }],
            },
            // Forward reference: declared BEFORE the struct it embeds.
            TypedStruct {
                name: "Outer".into(),
                module: String::new(),
                fields: vec![TypedStructField {
                    name: "inner".into(),
                    ty: KaiType::Struct(StructId(4)),
                }],
            },
            TypedStruct {
                name: "Empty".into(),
                module: String::new(),
                fields: vec![],
            },
            TypedStruct {
                name: "Inner".into(),
                module: String::new(),
                fields: vec![TypedStructField { name: "a".into(), ty: KaiType::Array(Box::new(KaiType::Int32)) }],
            },
        ];
        let heap = HeapBearing::new(&structs);
        assert!(!heap.is(&KaiType::Struct(StructId(0)))); // Plain
        assert!(heap.is(&KaiType::Struct(StructId(1))));  // Bearing
        assert!(heap.is(&KaiType::Struct(StructId(2))));  // Outer (forward ref)
        assert!(!heap.is(&KaiType::Struct(StructId(3)))); // Empty
        assert!(!heap.is(&KaiType::Int32));
        assert!(heap.is(&KaiType::Array(Box::new(KaiType::Int32))));
        assert!(heap.is(&KaiType::String));
    }

    // ---------- retain-on-transfer (§9.5 / E8) ----------

    #[test]
    fn returning_param_retains() {
        let program = TypedProgram {
            structs: vec![],
            fns: vec![fn_decl(
                block(vec![ret(Some(local_ref(0, KaiType::String)))]),
                vec![param(0, "s", KaiType::String)],
                KaiType::String,
            )],
        };
        let out = run(program);
        let TypedStmt::ReturnCleanup { value, releases } = &out.fns[0].body.stmts[0]
        else {
            panic!("expected return cleanup");
        };
        let inner = value.as_ref().expect("return keeps its value");
        assert!(unwrap_retain(inner));
        let TypedExprKind::Retain(inner) = &inner.kind else { panic!("expected retain") };
        assert!(matches!(inner.kind, TypedExprKind::LocalRef(_)));
        assert_eq!(inner.ty, KaiType::String);
        // Params borrow — they are never in any release list.
        assert!(releases.is_empty());
    }

    #[test]
    fn returning_literal_moves_free() {
        let program = TypedProgram {
            structs: vec![],
            fns: vec![fn_decl(
                block(vec![ret(Some(str_lit("hi")))]),
                vec![],
                KaiType::String,
            )],
        };
        let out = run(program);
        let TypedStmt::ReturnCleanup { value, .. } = &out.fns[0].body.stmts[0] else { panic!() };
        let e = value.as_ref().expect("literal survives the return");
        assert!(!unwrap_retain(e));
    }

    #[test]
    fn scalar_returns_never_retain() {
        let program = TypedProgram {
            structs: vec![],
            fns: vec![fn_decl(
                block(vec![ret(Some(local_ref(0, KaiType::Int32)))]),
                vec![param(0, "n", KaiType::Int32)],
                KaiType::Int32,
            )],
        };
        let out = run(program);
        let TypedStmt::ReturnCleanup { value, .. } = &out.fns[0].body.stmts[0] else { panic!() };
        let e = value.as_ref().unwrap();
        assert!(!unwrap_retain(e));
    }

    #[test]
    fn let_of_binding_co_owns_via_retain() {
        // let x = "a"; let y = x;
        let body = block(vec![
            let_(0, "x", str_lit("a")),
            let_(1, "y", local_ref(0, KaiType::String)),
            ret(None),
        ]);
        let program = TypedProgram { structs: vec![], fns: vec![fn_decl(body, vec![], KaiType::Unit)] };
        let out = run(program);
        let TypedStmt::Let(y) = &out.fns[0].body.stmts[1] else { panic!() };
        assert!(unwrap_retain(&y.init));
        // x stays unwrapped (owned temp moves free).
        let TypedStmt::Let(x) = &out.fns[0].body.stmts[0] else { panic!() };
        assert!(!unwrap_retain(&x.init));
    }

    #[test]
    fn assignment_retains_borrowed_and_marks_release_old() {
        // var v = "a"; v = w;   (w is another binding)
        let body = block(vec![
            let_(0, "v", str_lit("a")),
            let_(1, "w", str_lit("b")),
            TypedStmt::Assign(assign(0, vec![], local_ref(1, KaiType::String))),
            ret(None),
        ]);
        let program = TypedProgram { structs: vec![], fns: vec![fn_decl(body, vec![], KaiType::Unit)] };
        let out = run(program);
        let TypedStmt::Assign(a) = &out.fns[0].body.stmts[2] else { panic!() };
        assert!(a.release_old, "owning slot replacement releases old (E4)");
        assert!(unwrap_retain(&a.value), "borrowed RHS retains before move");
    }

    #[test]
    fn owned_temp_assignment_moves_free_but_still_releases_old() {
        let body = block(vec![
            let_(0, "v", str_lit("a")),
            TypedStmt::Assign(assign(0, vec![], str_lit("fresh"))),
            ret(None),
        ]);
        let program = TypedProgram { structs: vec![], fns: vec![fn_decl(body, vec![], KaiType::Unit)] };
        let out = run(program);
        let TypedStmt::Assign(a) = &out.fns[0].body.stmts[1] else { panic!() };
        assert!(a.release_old);
        assert!(!unwrap_retain(&a.value));
    }

    #[test]
    fn compound_assign_never_sets_release_old() {
        let a = TypedAssign {
            root: LocalId(0),
            path: vec![],
            op: Some(BinaryOp::Add),
            value: int_lit(1),
            release_old: false,
            span: kai_diagnostics::Span::new(0, 0),
        };
        let body = block(vec![
            let_(0, "n", int_lit(0)),
            TypedStmt::Assign(a),
            ret(None),
        ]);
        let program = TypedProgram { structs: vec![], fns: vec![fn_decl(body, vec![], KaiType::Unit)] };
        let out = run(program);
        let TypedStmt::Assign(a) = &out.fns[0].body.stmts[1] else { panic!() };
        assert!(!a.release_old);
    }

    #[test]
    fn literal_fields_and_elements_are_owning_slots() {
        // wrap(p): User { name: p }  and  [p, p]
        let user_struct = TypedStruct {
            name: "User".into(),
            module: String::new(),
            fields: vec![TypedStructField { name: "name".into(), ty: KaiType::String }],
        };
        let struct_lit = |values| TypedExpr::new(
            TypedExprKind::StructLit { struct_id: StructId(0), values },
            KaiType::Struct(StructId(0)),
        );
        let arr_lit = |elements| TypedExpr::new(
            TypedExprKind::ArrayLit { elements },
            KaiType::Array(Box::new(KaiType::String)),
        );

        let body = block(vec![ret(Some(struct_lit(vec![local_ref(0, KaiType::String)])))]);
        let program = TypedProgram {
            structs: vec![user_struct.clone()],
            fns: vec![fn_decl(body, vec![param(0, "p", KaiType::String)], KaiType::Struct(StructId(0)))],
        };
        let out = run(program);
        let TypedStmt::ReturnCleanup { value, .. } = &out.fns[0].body.stmts[0] else {
            panic!("literal itself moves free")
        };
        let e = value.as_ref().unwrap();
        assert!(!unwrap_retain(e));
        let TypedExprKind::StructLit { values, .. } = &e.kind else { panic!() };
        assert!(unwrap_retain(&values[0]), "field slot retains borrowed source");

        let body = block(vec![ret(Some(arr_lit(vec![local_ref(0, KaiType::String)])))]);
        let program = TypedProgram {
            structs: vec![user_struct],
            fns: vec![fn_decl(
                body,
                vec![param(0, "p", KaiType::String)],
                KaiType::Array(Box::new(KaiType::String)),
            )],
        };
        let out = run(program);
        let TypedStmt::ReturnCleanup { value, .. } = &out.fns[0].body.stmts[0] else { panic!() };
        let e = value.as_ref().unwrap();
        let TypedExprKind::ArrayLit { elements } = &e.kind else { panic!() };
        assert!(unwrap_retain(&elements[0]), "array elements are owning slots");
    }

    #[test]
    fn call_arguments_are_borrowed_never_retained() {
        // callee(p); — argument position borrows (§9.6)
        let callee_call = TypedExpr::new(
            TypedExprKind::Call {
                func: kai_tast::FunctionId(1),
                args: vec![local_ref(0, KaiType::String)],
            },
            KaiType::Unit,
        );
        let body = block(vec![TypedStmt::Expr(callee_call), ret(None)]);
        let program = TypedProgram {
            structs: vec![],
            fns: vec![fn_decl(body, vec![param(0, "p", KaiType::String)], KaiType::Unit)],
        };
        let out = run(program);
        let TypedStmt::Expr(e) = &out.fns[0].body.stmts[0] else { panic!() };
        let TypedExprKind::Call { args, .. } = &e.kind else { panic!() };
        assert!(!unwrap_retain(&args[0]));
    }

    // ---------- scope-exit releases (§9.4) ----------

    #[test]
    fn heap_locals_release_at_block_end_reverse_order() {
        let body = block(vec![
            let_(0, "a", str_lit("a")),
            let_(1, "b", str_lit("b")),
            let_(2, "n", int_lit(0)), // scalar: not tracked
            ret(None),
        ]);
        let program = TypedProgram { structs: vec![], fns: vec![fn_decl(body, vec![], KaiType::Unit)] };
        let out = run(program);
        // stmts: let a, let b, let n, return-with-cleanup(b, a)
        let TypedStmt::ReturnCleanup { releases, .. } = &out.fns[0].body.stmts[3]
        else {
            panic!("expected return cleanup");
        };
        // Reverse declaration order; the scalar `n` and params never appear.
        assert_eq!(
            releases,
            &vec![(LocalId(1), KaiType::String), (LocalId(0), KaiType::String)]
        );
    }

    #[test]
    fn return_inside_nested_block_releases_all_frames() {
        // let a = "..."; if c { return; }   — return must release `a` too.
        let cond = TypedExpr::new(TypedExprKind::BoolLit(true), KaiType::Bool);
        let body = block(vec![
            let_(0, "a", str_lit("a")),
            TypedStmt::If(kai_tast::TypedIf {
                cond,
                then_block: block(vec![ret(None)]),
                else_block: None,
            }),
        ]);
        let program = TypedProgram { structs: vec![], fns: vec![fn_decl(body, vec![], KaiType::Unit)] };
        let out = run(program);
        // Outer block: [let a, If, release a] — If sits at index 1.
        let TypedStmt::If(if_) = &out.fns[0].body.stmts[1] else { panic!() };
        // then-block carries the OUTER frame's locals in its cleanup:
        // returning from inside the branch still unwinds `a`.
        let TypedStmt::ReturnCleanup { releases, .. } = &if_.then_block.stmts[0]
        else {
            panic!("expected cleanup-carrying return");
        };
        assert_eq!(releases, &vec![(LocalId(0), KaiType::String)]);
        // Normal end of the OUTER block also releases `a` (the branch may
        // fall through): [let a, If, release a].
        assert!(matches!(
            out.fns[0].body.stmts[2],
            TypedStmt::ReleaseLocal { local: LocalId(0), .. }
        ));
    }

    // ---------- for..in (§9.9 / E7) ----------

    #[test]
    fn for_over_owned_temp_takes_ownership() {
        let iter = TypedExpr::new(
            TypedExprKind::ArrayLit {
                elements: vec![int_lit(1)],
            },
            KaiType::Array(Box::new(KaiType::Int32)),
        );
        let f = TypedFor {
            binding_local: LocalId(10),
            binding_name: "v".into(),
            iterable: iter,
            body: block(vec![]),
            iterable_owned: false,
        };
        let body = block(vec![TypedStmt::For(f), ret(None)]);
        let program = TypedProgram { structs: vec![], fns: vec![fn_decl(body, vec![], KaiType::Unit)] };
        let out = run(program);
        // The temp is bound to a hidden local before the loop; the loop now
        // iterates the LOCAL and the flag path is retired.
        assert!(matches!(
            &out.fns[0].body.stmts[0],
            TypedStmt::Let(b) if b.name == "$iter"
        ));
        let TypedStmt::For(f) = &out.fns[0].body.stmts[1] else { panic!() };
        assert!(!f.iterable_owned);
        let iter_local = match &f.iterable.kind {
            TypedExprKind::LocalRef(id) => *id,
            other => panic!("iterable not materialized: {other:?}"),
        };
        // Normal completion: the loop frame pops right after the For —
        // the hidden owner is released there; the loop binding never owns.
        let stmts = &out.fns[0].body.stmts;
        assert!(matches!(
            stmts.get(2),
            Some(TypedStmt::ReleaseLocal { local, .. }) if *local == iter_local
        ));
        assert!(stmts
            .iter()
            .all(|s| !matches!(s, TypedStmt::ReleaseLocal { local: LocalId(10), .. })));
    }

    #[test]
    fn return_inside_loop_releases_owned_iterable() {
        // B3: `for x in [1] { return; }` used to skip the loop-end release.
        let iter = TypedExpr::new(
            TypedExprKind::ArrayLit {
                elements: vec![int_lit(1)],
            },
            KaiType::Array(Box::new(KaiType::Int32)),
        );
        let f = TypedFor {
            binding_local: LocalId(10),
            binding_name: "v".into(),
            iterable: iter,
            body: block(vec![ret(None)]),
            iterable_owned: false,
        };
        let body = block(vec![TypedStmt::For(f)]);
        let program = TypedProgram { structs: vec![], fns: vec![fn_decl(body, vec![], KaiType::Unit)] };
        let out = run(program);
        // The return inside the body must carry the hidden iterable's
        // release alongside the (empty) body frame.
        let mut found_iter_release = false;
        for s in &out.fns[0].body.stmts {
            if let TypedStmt::For(f) = s {
                for inner in &f.body.stmts {
                    if let TypedStmt::ReturnCleanup { releases, .. } = inner {
                        found_iter_release =
                            releases.iter().any(|(_, ty)| matches!(ty, KaiType::Array(_)));
                    }
                }
            }
        }
        assert!(found_iter_release, "return skips iterable release:\n{out:#?}");
    }

    #[test]
    fn discarded_heap_temp_is_bound_and_released() {
        // B1: `make();` as a statement must not leak the returned string.
        let call = TypedExpr::new(
            TypedExprKind::Call { func: kai_tast::FunctionId(0), args: vec![] },
            KaiType::String,
        );
        let body = block(vec![TypedStmt::Expr(call), ret(None)]);
        let program = TypedProgram { structs: vec![], fns: vec![fn_decl(body, vec![], KaiType::Unit)] };
        let out = run(program);
        assert!(matches!(
            &out.fns[0].body.stmts[0],
            TypedStmt::Let(b) if b.name == "$tmp" && b.init.ty == KaiType::String
        ));
        // The following `return` carries the hidden local's release.
        assert!(matches!(
            out.fns[0].body.stmts.last(),
            Some(TypedStmt::ReturnCleanup { releases, .. }) if !releases.is_empty()
        ));
    }

    #[test]
    fn call_arg_temp_is_materialized_in_order() {
        // B1: greet("x") — the literal moves into a hidden local BEFORE the
        // call, the argument becomes a plain borrow of that local.
        let greet = TypedExpr::new(
            TypedExprKind::Call {
                func: kai_tast::FunctionId(1),
                args: vec![str_lit("x")],
            },
            KaiType::Unit,
        );
        let body = block(vec![TypedStmt::Expr(greet), ret(None)]);
        let program = TypedProgram { structs: vec![], fns: vec![fn_decl(body, vec![], KaiType::Unit)] };
        let out = run(program);
        let TypedStmt::Let(hidden) = &out.fns[0].body.stmts[0] else { panic!("no hoist:\n{out:#?}") };
        assert_eq!(hidden.name, "$tmp");
        let TypedStmt::Expr(e) = &out.fns[0].body.stmts[1] else { panic!() };
        let TypedExprKind::Call { args, .. } = &e.kind else { panic!() };
        assert!(matches!(args[0].kind, TypedExprKind::LocalRef(_)));
    }

    #[test]
    fn for_over_borrowed_iterable_leaves_ownership_alone() {
        let iter = local_ref(0, KaiType::Array(Box::new(KaiType::Int32)));
        let f = TypedFor {
            binding_local: LocalId(10),
            binding_name: "v".into(),
            iterable: iter,
            body: block(vec![]),
            iterable_owned: false,
        };
        let body = block(vec![
            let_(0, "arr", TypedExpr::new(
                TypedExprKind::ArrayLit { elements: vec![int_lit(1)] },
                KaiType::Array(Box::new(KaiType::Int32)),
            )),
            TypedStmt::For(f),
            ret(None),
        ]);
        let program = TypedProgram { structs: vec![], fns: vec![fn_decl(body, vec![], KaiType::Unit)] };
        let out = run(program);
        let TypedStmt::For(f) = &out.fns[0].body.stmts[1] else { panic!() };
        assert!(!f.iterable_owned);
    }
}
