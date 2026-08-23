//! Ownership resolution (§9): the explicit IR-producing phase between
//! typecheck and codegen. Every retain/release/move decision is materialized
//! as a TAST node here — `TypedExprKind::Retain`, `TypedStmt::ReleaseLocal`,
//! `TypedAssign::release_old`, `TypedFor::iterable_owned` — so codegen reads
//! mechanically and never infers ownership itself (§8, constraint 2).
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
//! - `for..in` iterables that are owned temporaries transfer into the loop;
//!   borrowed iterables stay put. The loop variable never owns (E7).

use kai_tast::{
    KaiType, LocalId, TypedAssign, TypedBlock, TypedExpr, TypedExprKind, TypedFnDecl, TypedFor,
    TypedProgram, TypedStmt,
};

/// Runs the pass over a typechecked program, annotating it in place.
pub fn resolve(program: &mut TypedProgram) {
    let heap = HeapBearing::new(&program.structs);
    for fns in &mut program.fns {
        resolve_fn(&heap, fns);
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

fn resolve_fn(heap: &HeapBearing, decl: &mut TypedFnDecl) {
    // Frame 0: parameters — they borrow, so they are never registered for
    // release (the callee does not release what it does not own, §9.3).
    let mut scopes = Scopes::default();
    scopes.push();
    for param in &decl.params {
        scopes.declare(param.local, param.ty.clone(), false);
    }
    let body = std::mem::replace(&mut decl.body, TypedBlock { stmts: Vec::new() });
    decl.body = walk_block(heap, body, &mut scopes);
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

fn walk_block(heap: &HeapBearing, mut block: TypedBlock, scopes: &mut Scopes) -> TypedBlock {
    scopes.push();
    let mut out = Vec::with_capacity(block.stmts.len());
    for stmt in std::mem::take(&mut block.stmts) {
        match stmt {
            // Return paths release every live local before leaving — but
            // AFTER the return value exists (it may still read locals it
            // borrows; the §9.5 retain on the value keeps heap content
            // alive past the releases). One node carries both.
            ret @ TypedStmt::Return(_) => {
                let ret = finish_return(heap, ret, scopes);
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
            other => out.extend(walk_stmt(heap, other, scopes)),
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

fn finish_return(heap: &HeapBearing, ret: TypedStmt, _scopes: &Scopes) -> TypedStmt {
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

fn walk_stmt(heap: &HeapBearing, stmt: TypedStmt, scopes: &mut Scopes) -> Vec<TypedStmt> {
    match stmt {
        TypedStmt::Let(mut binding) => {
            walk_expr(heap, &mut binding.init);
            // Owning slot: co-own borrowed sources (§9.4/§9.5 row 3).
            wrap_retain_if_borrowed(heap, &mut binding.init);
            scopes.declare(
                binding.local,
                binding.init.ty.clone(),
                heap.is(&binding.init.ty),
            );
            vec![TypedStmt::Let(binding)]
        }
        TypedStmt::Assign(assign) => vec![TypedStmt::Assign(walk_assign(heap, assign))],
        TypedStmt::If(if_) => {
            let mut if_ = if_;
            walk_expr(heap, &mut if_.cond);
            if_.then_block = walk_block(heap, if_.then_block, scopes);
            if_.else_block = if_
                .else_block
                .map(|b| walk_block(heap, b, scopes));
            vec![TypedStmt::If(if_)]
        }
        TypedStmt::For(f) => vec![TypedStmt::For(walk_for(heap, f, scopes))],
        TypedStmt::Block(block) => vec![TypedStmt::Block(walk_block(heap, block, scopes))],
        TypedStmt::Expr(mut e) => {
            walk_expr(heap, &mut e);
            vec![TypedStmt::Expr(e)]
        }
        // Handled by the caller (return needs surrounding-scope context).
        TypedStmt::Return(_) => unreachable!("returns handled by walk_block"),
        TypedStmt::ReleaseLocal { .. } | TypedStmt::ReturnCleanup { .. } => {
            unreachable!("nodes are pass-generated")
        }
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

fn walk_for(heap: &HeapBearing, mut f: TypedFor, scopes: &mut Scopes) -> TypedFor {
    walk_expr(heap, &mut f.iterable);
    // Owned temporaries transfer into the loop machinery and are released at
    // loop end; borrowed iterables remain owned where they were (§9.9).
    f.iterable_owned = is_owned_temp(&f.iterable);
    f.body = walk_block(heap, f.body, scopes);
    f
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
        let TypedStmt::For(f) = &out.fns[0].body.stmts[0] else { panic!() };
        assert!(f.iterable_owned);
        // Loop binding never owns: no ReleaseLocal for LocalId(10).
        assert!(out.fns[0]
            .body
            .stmts
            .iter()
            .all(|s| !matches!(s, TypedStmt::ReleaseLocal { local: LocalId(10), .. })));
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
