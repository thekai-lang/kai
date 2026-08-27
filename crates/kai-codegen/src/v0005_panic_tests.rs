use super::*;
use kai_tast::{BinaryOp, KaiType, TypedExpr, TypedExprKind, TypedProgram};

/// `main` returning one expression — the smallest vehicle for a check.
fn returns(expr: TypedExpr) -> TypedProgram {
    use kai_tast::{EffectSet, FunctionId, TypedBlock, TypedFnDecl, TypedStmt};
    TypedProgram {
        structs: Vec::new(),
        fns: vec![TypedFnDecl {
            id: FunctionId(0),
            name: "main".into(),
            module: String::new(),
            params: Vec::new(),
            ret: expr.ty.clone(),
            declared_effects: None,
            inferred_effects: EffectSet::default(),
            is_reversible: false,
            body: TypedBlock {
                stmts: vec![TypedStmt::Return(Some(expr))],
            },
        }],
    }
}

fn bin(op: BinaryOp, lhs: i64, rhs: i64) -> TypedExpr {
    let wide = lhs < i32::MIN as i64 || lhs > i32::MAX as i64;
    let int = if wide {
        KaiType::Int64
    } else {
        KaiType::Int32
    };
    TypedExpr::new(
        TypedExprKind::Binary {
            op,
            lhs: Box::new(TypedExpr::new(TypedExprKind::IntLit(lhs), int.clone())),
            rhs: Box::new(TypedExpr::new(TypedExprKind::IntLit(rhs), int.clone())),
            rhs_hoists: Vec::new(),
        },
        int,
    )
}

#[test]
fn indexed_reads_carry_bounds_guards() {
    // `[10][0]`: in-bounds read still emits the guard; JIT survives it.
    use kai_tast::KaiType as Kt;
    let arr_ty = Kt::Array(Box::new(Kt::Int32));
    let read = TypedExpr::new(
        TypedExprKind::Index {
            base: Box::new(TypedExpr::new(
                TypedExprKind::ArrayLit {
                    elements: vec![TypedExpr::new(
                        TypedExprKind::IntLit(10),
                        Kt::Int32,
                    )],
                },
                arr_ty,
            )),
            index: Box::new(TypedExpr::new(TypedExprKind::IntLit(0), Kt::Int32)),
        },
        Kt::Int32,
    );
    let ir = compile_ir("test", &returns(read.clone())).unwrap();
    assert!(
        ir.contains("array index out of bounds"),
        "no bounds message global:\n{ir}"
    );
    assert!(ir.contains("@kai_panic"), "no panic call:\n{ir}");
    assert_eq!(run_jit(&returns(read)).unwrap(), 10);
}

#[test]
fn division_by_zero_emits_guard() {
    let ir = compile_ir("test", &returns(bin(BinaryOp::Div, 1, 0))).unwrap();
    assert!(ir.contains("division by zero"), "ir:\n{ir}");
}

#[test]
fn modulo_by_zero_emits_guard() {
    let ir = compile_ir("test", &returns(bin(BinaryOp::Mod, 7, 0))).unwrap();
    assert!(ir.contains("modulo by zero"), "ir:\n{ir}");
}

#[test]
fn signed_arithmetic_uses_checked_intrinsics() {
    for (op, intrinsic) in [
        (BinaryOp::Add, "llvm.sadd.with.overflow.i32"),
        (BinaryOp::Sub, "llvm.ssub.with.overflow.i32"),
        (BinaryOp::Mul, "llvm.smul.with.overflow.i32"),
    ] {
        let ir = compile_ir("test", &returns(bin(op, 2, 3))).unwrap();
        assert!(ir.contains(intrinsic), "{intrinsic} missing:\n{ir}");
        assert!(ir.contains("integer overflow"), "{op:?}:\n{ir}");
    }
}

#[test]
fn negation_of_min_uses_checked_subtraction() {
    let expr = TypedExpr::new(
        TypedExprKind::Neg(Box::new(TypedExpr::new(
            TypedExprKind::IntLit(i64::MIN),
            KaiType::Int64,
        ))),
        KaiType::Int64,
    );
    let ir = compile_ir("test", &returns(expr)).unwrap();
    assert!(
        ir.contains("llvm.ssub.with.overflow.i64"),
        "neg must trap on -MIN:\n{ir}"
    );
}

#[test]
fn min_div_minus_one_traps_as_overflow() {
    let ir = compile_ir("test", &returns(bin(BinaryOp::Div, i32::MIN as i64, -1))).unwrap();
    assert!(
        ir.contains("integer overflow"),
        "MIN / -1 must report overflow:\n{ir}"
    );
}

#[test]
fn safe_division_still_computes() {
    assert_eq!(run_jit(&returns(bin(BinaryOp::Div, 9, 2))).unwrap(), 4);
    assert_eq!(run_jit(&returns(bin(BinaryOp::Mod, 9, 2))).unwrap(), 1);
}
