//! Name resolution over the untyped AST, across modules.
//!
//! v0.0.4 scope:
//! - per-module namespace tables: unqualified names resolve ONLY inside the
//!   declaring module (§3.6 — imports never inject into any scope)
//! - import aliases map to loaded modules; duplicates are errors
//! - `public` flags travel with declarations as visibility masks; the type
//!   checker enforces them at each qualified use site
//! - separate namespaces for types and functions (Rust-style) within a
//!   module, plus a third namespace for import aliases
//! - cyclic struct definitions are a compile error reported as a cycle path
//!   (cycles cannot span modules: field types are always unqualified)
//! - the entry-point contract (`main`, no params, returns int32), scoped to
//!   the ENTRY module

pub mod entry;
pub mod tables;

use kai_ast::Program;
use kai_diagnostics::Diagnostic;

pub use entry::check_entry;
pub use tables::{ModuleInput, Resolution};

/// Legacy single-program entry point: resolves as one anonymous module.
pub fn analyze(program: &Program) -> Result<Resolution, Vec<Diagnostic>> {
    analyze_modules(&[ModuleInput {
        name: "",
        file: "",
        program,
    }])
}

/// Resolves names across all loaded modules. On success the returned
/// `Resolution` feeds the type checker together with the merged program; on
/// failure the diagnostic list is complete for this phase.
pub fn analyze_modules(modules: &[ModuleInput]) -> Result<Resolution, Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();

    let mut resolution = tables::build_multi(modules, &mut diagnostics);

    // Cycle detection and entry validation run over the merged view. The
    // entry contract consults Resolution.fn_module, so imported `main`s
    // don't count — the program's main lives in the ENTRY module.
    if let Some(entry) = modules.first() {
        tables::detect_cycles(entry.program, &resolution, &mut diagnostics);
        check_entry(entry.program, &resolution, &mut diagnostics);
    }

    // §9.10 closure-bearing poisoning: after cycle detection (cyclic
    // programs abort before the checker ever consults this table).
    resolution.closure_bearing = tables::compute_closure_bearing(modules, &resolution);

    if diagnostics.is_empty() {
        Ok(resolution)
    } else {
        Err(diagnostics)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_src(src: &str) -> kai_ast::Program {
        let lexed = kai_lexer::lex(src);
        kai_parser::parse(&lexed.tokens).expect("parse failed")
    }

    fn single(src: &str) -> Result<Resolution, Vec<Diagnostic>> {
        let program = parse_src(src);
        analyze(&program)
    }

    #[test]
    fn accepts_valid_program() {
        let resolution =
            single(
                "fn add(a: int32, b: int32) -> int32 { return a + b; } \
                 fn main() -> int32 { return 0; }",
            )
            .unwrap();
        assert_eq!(resolution.module_fns[0].len(), 2);
        assert!(!resolution.fn_is_public[0]);
    }

    #[test]
    fn records_public_flag() {
        let resolution = single(
            "public fn add(a: int32, b: int32) -> int32 { return a + b; } \
             fn main() -> int32 { return 0; }",
        )
        .unwrap();
        assert!(resolution.fn_is_public[0]);
    }

    #[test]
    fn rejects_duplicate_fn_within_module() {
        let diags = single(
            "fn f() -> int32 { return 0; } fn f() -> int32 { return 0; }",
        )
        .unwrap_err();
        assert!(diags.iter().any(|d| d.message == "duplicate function `f`"));
    }

    #[test]
    fn allows_same_name_across_modules() {
        let entry = parse_src(
            "use support.math; \
             fn main() -> int32 { let x = math.five(); return x; }",
        );
        let math = parse_src("public fn five() -> int32 { return 5; }");
        let resolution = analyze_modules(&[
            ModuleInput {
                name: "",
                file: "main.kai",
                program: &entry,
            },
            ModuleInput {
                name: "support.math",
                file: "support/math.kai",
                program: &math,
            },
        ])
        .unwrap();
        // Two modules may both expose `five`; tables stay per-module.
        assert_eq!(resolution.module_fns[1].get("five"), Some(&1));
    }

    #[test]
    fn rejects_duplicate_import_alias() {
        let diags = single(
            "use support.math; use other.math; fn main() -> int32 { return 0; }",
        )
        .unwrap_err();
        // Only one module is loaded, so the second import is either an
        // unknown target or a duplicate alias depending on order of checks.
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("cannot find module")
                    || d.message.contains("duplicate import alias"))
        );
    }

    #[test]
    fn rejects_unknown_import_target() {
        let diags =
            single("use support.nope; fn main() -> int32 { return 0; }").unwrap_err();
        assert_eq!(
            diags[0].message,
            "cannot find module `support.nope`".to_string()
        );
    }

    #[test]
    fn rejects_self_import() {
        let program = parse_src("use self.main; fn main() -> int32 { return 0; }")
            ;
        // The import path "self.main" is not a loaded module name; loading
        // would never produce it, so this surfaces as cannot-find rather
        // than a cycle. Cycles are caught by the loader (§3.6).
        let diags = analyze(&program).unwrap_err();
        assert!(!diags.is_empty());
    }

    #[test]
    fn imported_public_main_does_not_satisfy_entry_contract() {
        let entry = parse_src("use lib.run;");
        let lib = parse_src("public fn main() -> int32 { return 0; }");
        let diags = analyze_modules(&[
            ModuleInput {
                name: "",
                file: "main.kai",
                program: &entry,
            },
            ModuleInput {
                name: "lib.run",
                file: "lib/run.kai",
                program: &lib,
            },
        ])
        .unwrap_err();
        assert!(diags.iter().any(|d| d.message.contains("no `main`")));
    }

    #[test]
    fn detects_cyclic_structs_within_module() {
        let diags = single(
            "type A = { b: B; } type B = { a: A; } fn main() -> int32 { return 0; }",
        )
        .unwrap_err();
        assert!(diags
            .iter()
            .any(|d| d.message.starts_with("cyclic type") && d.message.contains("A")));
    }

    #[test]
    fn accepts_acyclic_structs() {
        let resolution = single(
            "type Point = { x: int32; y: int32; } \
             type Line = { a: Point; b: Point; } \
             fn main() -> int32 { return 0; }",
        )
        .unwrap();
        assert_eq!(resolution.module_types[0].len(), 2);
    }

    #[test]
    fn self_referential_struct_via_boxing_placeholder_is_still_a_cycle() {
        let diags = single(
            "type Node = { next: Node; } fn main() -> int32 { return 0; }",
        )
        .unwrap_err();
        assert!(diags.iter().any(|d| d.message.starts_with("cyclic type")));
    }
}

// -- v0.0.6: §9.10 closure-bearing poisoning -----------------------------------

#[cfg(test)]
mod v0006_tests {
    use super::*;

    fn parse_src(src: &str) -> kai_ast::Program {
        let lexed = kai_lexer::lex(src);
        kai_parser::parse(&lexed.tokens).expect("parse failed")
    }

    fn bearing_of(src: &str) -> Vec<bool> {
        let program = parse_src(src);
        let resolution = analyze(&program).unwrap();
        resolution.closure_bearing
    }

    #[test]
    fn direct_closure_field_poisons() {
        let flags = bearing_of(
            "type Node = { action: (unit) -> unit; } \
             type Plain = { x: int32; } \
             fn main() -> int32 { return 0; }",
        );
        assert_eq!(flags, vec![true, false]);
    }

    #[test]
    fn transitive_field_chain_poisons() {
        // Inner holds the closure; Outer holds Inner — both poisoned.
        let flags = bearing_of(
            "type Inner = { cb: (unit) -> unit; } \
             type Outer = { inner: Inner; } \
             type Unrelated = { n: int32; } \
             fn main() -> int32 { return 0; }",
        );
        assert_eq!(flags, vec![true, true, false]);
    }

    #[test]
    fn array_element_propagates_even_though_layout_does_not() {
        // Arrays are pointers for LAYOUT cycles but share their buffer
        // mutably — §9.10 poisoning follows the semantics, not the layout.
        let flags = bearing_of(
            "type Holder = { actions: (unit) -> unit[]; } \
             type ScalarArr = { xs: int32[]; } \
             fn main() -> int32 { return 0; }",
        );
        assert_eq!(flags, vec![true, false]);
    }

    #[test]
    fn tagged_union_payloads_propagate() {
        let flags = bearing_of(
            "type Opt = { o: (unit) -> unit?; } \
             type Res = { r: Result<int32, (unit) -> unit>; } \
             type Clean = { o: int32?; } \
             fn main() -> int32 { return 0; }",
        );
        assert_eq!(flags, vec![true, true, false]);
    }

    #[test]
    fn unknown_field_type_is_not_an_edge_here() {
        // `Mystery` resolves nowhere; "unknown type" is reported later per
        // use — poisoning must not crash on it.
        let flags = bearing_of(
            "type H = { m: Mystery; } fn main() -> int32 { return 0; }",
        );
        assert_eq!(flags.len(), 1);
        assert!(!flags[0]);
    }
}
