use super::*;
use kai_ast::{ExprKind, StmtKind};
use kai_lexer::lex;

pub(crate) fn parse_src(src: &str) -> Result<Program, Vec<Diagnostic>> {
    let out = lex(src);
    assert!(
        out.diagnostics.is_empty(),
        "lexing failed: {:?}",
        out.diagnostics
    );
    parse(&out.tokens)
}

#[test]
fn parses_minimal_program() {
    let program = parse_src("fn main() -> int32 { return 0; }").unwrap();
    assert_eq!(program.fns.len(), 1);
    let main = &program.fns[0];
    assert_eq!(main.path.last().unwrap().name, "main");
    match &main.ret {
        kai_ast::Ty::Path(ident) => assert_eq!(ident.last().unwrap().name, "int32"),
        other => panic!("expected named type, got {other:?}"),
    }
    assert_eq!(main.body.stmts.len(), 1);
}

#[test]
fn parses_reversible_fn_marker() {
    let program = parse_src("fn transferMoney() -> int32 reversible { return 0; }").unwrap();
    let f = &program.fns[0];
    assert!(f.is_reversible, "expected is_reversible = true");
}

#[test]
fn parses_non_reversible_fn_marker() {
    let program = parse_src("fn ordinary() -> int32 { return 0; }").unwrap();
    let f = &program.fns[0];
    assert!(!f.is_reversible, "expected is_reversible = false");
}

#[test]
fn parses_compensate_postfix() {
    let program = parse_src(
        "fn f() -> int32 reversible { chargeCard(user, fee) compensate { refundCard(user, fee); }; return 0; }",
    )
    .unwrap();
    let f = &program.fns[0];
    assert!(f.is_reversible);
    let stmts = &f.body.stmts;
    let first = stmts.first().expect("one statement");
    let StmtKind::Expr(expr) = &first.kind else {
        panic!("expected expression statement");
    };
    let ExprKind::Compensate(comp) = &expr.kind else {
        panic!("expected Compensate expr, got {:?}", expr.kind);
    };
    let ExprKind::Call(_) = comp.base.kind else {
        panic!("compensate base must be a call");
    };
    assert_eq!(comp.stmts.len(), 1);
}

#[test]
fn rejects_missing_return_semicolon() {
    let err = parse_src("fn main() -> int32 { return 0 }").unwrap_err();
    assert!(err[0].message.contains("`;`"));
}


// -- recursion budget ---------------------------------------------------

fn deep_parens(depth: usize) -> String {
    let mut src = String::from("fn main() -> int32 { return ");
    src.push_str(&"(".repeat(depth));
    src.push('1');
    src.push_str(&")".repeat(depth));
    src.push_str("; }");
    src
}

#[test]
fn deep_parens_fail_cleanly_without_stack_overflow() {
    // 10k nesting used to overflow the native stack; now it is a budget
    // diagnostic, never a crash.
    let err = parse_src(&deep_parens(10_000)).unwrap_err();
    assert!(err[0].message.contains("nested too deeply"));
}

#[test]
fn moderate_nesting_still_parses() {
    parse_src(&deep_parens(100)).expect("100 levels are within budget");
}

#[test]
fn deep_unary_chain_is_budgeted_too() {
    // Iterative parsing must not become an unbounded-AST loophole:
    // downstream phases recurse over operator chains as well.
    let mut src = String::from("fn main() -> bool { return ");
    src.push_str(&"-".repeat(1_000));
    src.push_str("1; }");
    let err = parse_src(&src).unwrap_err();
    assert!(err[0].message.contains("nested too deeply"));
}

#[test]
fn small_unary_chain_parses() {
    let program = parse_src("fn main() -> int32 { return - - 5; }").unwrap();
    match &program.fns[0].body.stmts[0].kind {
        kai_ast::StmtKind::Return(Some(e)) => {
            assert!(matches!(e.kind, kai_ast::ExprKind::Unary(_)));
        }
        other => panic!("expected unary, got {other:?}"),
    }
}

#[test]
fn depth_budget_reports_each_independent_overflow() {
    // The reported flag resets per recovery region: two separate deep
    // expressions yield two diagnostics, not one.
    let mut src = String::from("fn main() -> int32 { return ");
    src.push_str(&"(".repeat(1_000));
    src.push('1');
    src.push_str(&")".repeat(1_000));
    src.push_str("; return ");
    src.push_str(&"(".repeat(1_000));
    src.push('2');
    src.push_str(&")".repeat(1_000));
    src.push_str("; }");
    let err = parse_src(&src).unwrap_err();
    assert_eq!(err.len(), 2);
    assert!(err.iter().all(|d| d.message.contains("nested too deeply")));
}

#[test]
fn budget_recovery_preserves_surrounding_statements() {
    // After skipping the over-deep group, later statements still parse
    // (internal parser used here so the recovered tree can be inspected;
    // run on the shared big stack like `parse` itself).
    with_big_stack(move || {
        let mut src = String::from("fn main() -> int32 { return ");
        src.push_str(&"(".repeat(1_000));
        src.push('1');
        src.push_str(&")".repeat(1_000));
        src.push_str("; return 42; }");

        let out = kai_lexer::lex(&src);
        let mut p = parser::Parser::new(&out.tokens);
        let program = decl::program(&mut p);

        assert_eq!(p.diagnostics.len(), 1);
        assert!(p.diagnostics[0].message.contains("nested too deeply"));
        assert_eq!(program.fns[0].body.stmts.len(), 2, "both returns survive");
    })
}
