use super::*;
use kai_tast::{FunctionId, KaiType, TypedBlock, TypedExpr, TypedExprKind, TypedFnDecl,
    TypedProgram, TypedStmt, TypedStruct};

fn bare(id: u32, name: &str, module: &str, ret: KaiType, value: i64) -> TypedFnDecl {
    let mut decl = tests::fn_decl(
        id,
        name,
        ret.clone(),
        vec![TypedStmt::Return(Some(TypedExpr::new(
            TypedExprKind::IntLit(value),
            ret,
        )))],
    );
    decl.module = module.to_string();
    decl
}

#[test]
fn same_named_fns_in_different_modules_never_collide() {
    let program = TypedProgram {
        structs: Vec::new(),
        fns: vec![
            bare(0, "main", "", KaiType::Int32, 7),
            bare(1, "dup", "lib.a", KaiType::Int32, 1),
            bare(2, "dup", "lib.b", KaiType::Int32, 2),
        ],
    };
    let ir = compile_ir("test", &program).expect("qualified symbols are distinct");
    assert!(ir.contains("@lib.a.dup"), "ir:\n{ir}");
    assert!(ir.contains("@lib.b.dup"), "ir:\n{ir}");
    // Entry names stay bare so JIT can find @main.
    assert!(ir.contains("define i32 @main()"), "ir:\n{ir}");
    assert_eq!(run_jit(&program).unwrap(), 7);
}

#[test]
fn qualified_struct_type_names_follow_the_same_rule() {
    let point = TypedStruct {
        name: "Point".into(),
        module: "support.math".into(),
        fields: vec![
            kai_tast::TypedStructField {
                name: "x".into(),
                ty: KaiType::Int32,
            },
            kai_tast::TypedStructField {
                name: "y".into(),
                ty: KaiType::Int32,
            },
        ],
    };
    // A signature referencing the struct forces it into the printed IR.
    let takes_point = TypedFnDecl {
        id: FunctionId(1),
        name: "peek".into(),
        module: "support.math".into(),
        params: vec![kai_tast::TypedParam {
            local: kai_tast::LocalId(0),
            name: "p".into(),
            ty: KaiType::Struct(kai_tast::StructId(0)),
        }],
        ret: KaiType::Unit,
        body: TypedBlock {
            stmts: vec![TypedStmt::Return(None)],
        },
    };
    let program = TypedProgram {
        structs: vec![point],
        fns: vec![bare(0, "main", "", KaiType::Int32, 0), takes_point],
    };
    let ir = compile_ir("test", &program).expect("compiles");
    assert!(
        ir.contains("%support.math.Point = type"),
        "ir:\n{ir}"
    );
    assert!(ir.contains("@support.math.peek"), "ir:\n{ir}");
}
