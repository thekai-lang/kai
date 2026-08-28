use super::*;
use crate::test_support::parse_ok;
use kai_tast::{KaiType, TypedExprKind};

fn check_src(src: impl AsRef<str>) -> Result<TypedProgram, Vec<Diagnostic>> {
    let ast = parse_ok(src.as_ref());
    let resolution = kai_resolver::analyze(&ast).expect("resolution failed");
    check_with(&ast, &resolution, std::collections::HashMap::new())
}

#[test]
fn string_equality_allowed_and_bool_typed() {
    let src = "fn main() -> int32 { let a = \"x\"; let b = \"y\"; let same = a == b; return 0; }";
    let program = check_src(src).unwrap();
    match &program.fns[0].body.stmts[2] {
        kai_tast::TypedStmt::Let(l) => {
            assert_eq!(l.init.ty, KaiType::Bool);
            assert!(matches!(
                l.init.kind,
                TypedExprKind::Binary { op: kai_tast::BinaryOp::Eq, .. }
            ));
        }
        other => panic!("expected let, got {other:?}"),
    }
}
