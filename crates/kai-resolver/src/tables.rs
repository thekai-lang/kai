//! Namespace tables and struct dependency cycles (§9.2).

use std::collections::HashMap;

use kai_ast::{Program, Ty};
use kai_diagnostics::Diagnostic;

/// Name tables produced by resolution. Indices point into `Program.types` /
/// `Program.fns`, so the type checker turns surface names into ids without
/// re-walking declarations.
///
/// Types and functions live in SEPARATE namespaces: `type foo = ...` and
/// `fn foo() -> ...` coexist. Which namespace a name belongs to follows from
/// syntax form (`Foo { .. }` / `: Foo` vs `foo(..)`), so lookups never
/// collide.
#[derive(Debug, Default, Clone)]
pub struct Resolution {
    /// Struct name -> index into `Program.types`.
    pub types: HashMap<String, usize>,
    /// Function name -> index into `Program.fns`.
    pub fns: HashMap<String, usize>,
}

pub(crate) fn build(program: &Program, diagnostics: &mut Vec<Diagnostic>) -> Resolution {
    let mut resolution = Resolution::default();

    for (idx, decl) in program.types.iter().enumerate() {
        // entry() keeps the FIRST declaration; later ones are reported here.
        if resolution
            .types
            .insert(decl.name.name.clone(), idx)
            .is_some()
        {
            diagnostics.push(Diagnostic::error(
                format!("duplicate type `{}`", decl.name.name),
                decl.name.span,
            ));
        }

        check_duplicate_fields(decl, diagnostics);
    }

    for (idx, decl) in program.fns.iter().enumerate() {
        if resolution.fns.insert(decl.name.name.clone(), idx).is_some() {
            diagnostics.push(Diagnostic::error(
                format!("duplicate function `{}`", decl.name.name),
                decl.name.span,
            ));
        }
    }

    resolution
}

fn check_duplicate_fields(decl: &kai_ast::TypeDecl, diagnostics: &mut Vec<Diagnostic>) {
    let mut seen: HashMap<&str, ()> = HashMap::new();
    for field in &decl.fields {
        if seen.insert(field.name.name.as_str(), ()).is_some() {
            diagnostics.push(Diagnostic::error(
                format!(
                    "duplicate field `{}` in type `{}`",
                    field.name.name, decl.name.name
                ),
                field.name.span,
            ));
        }
    }
}

/// DFS over the struct-dependency graph (edges: a field's named type that is
/// itself a declared struct). Cycles are impossible to lay out, so they are a
/// compile error reporting the path: `cyclic type: A -> B -> A`.
///
/// Unknown names are not edges — "unknown type" is reported later, per use.
pub(crate) fn detect_cycles(
    program: &Program,
    type_ids: &HashMap<String, usize>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    #[derive(Clone, Copy, PartialEq)]
    enum Color {
        White,
        Gray,
        Black,
    }

    let n = program.types.len();
    let mut color = vec![Color::White; n];
    let mut stack: Vec<usize> = Vec::new();

    // Edges of one node, resolved once per visit.
    fn edges(program: &Program, type_ids: &HashMap<String, usize>, idx: usize) -> Vec<usize> {
        program.types[idx]
            .fields
            .iter()
            .filter_map(|field| match &field.ty {
                Ty::Named(ident) => type_ids.get(&ident.name).copied(),
            })
            .collect()
    }

    fn visit(
        program: &Program,
        type_ids: &HashMap<String, usize>,
        idx: usize,
        color: &mut [Color],
        stack: &mut Vec<usize>,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        color[idx] = Color::Gray;
        stack.push(idx);

        for next in edges(program, type_ids, idx) {
            match color[next] {
                Color::White => visit(program, type_ids, next, color, stack, diagnostics),
                Color::Gray => report_cycle(program, stack, next, diagnostics),
                Color::Black => {}
            }
        }

        stack.pop();
        color[idx] = Color::Black;
    }

    fn name_of(program: &Program, idx: usize) -> &str {
        &program.types[idx].name.name
    }

    fn report_cycle(
        program: &Program,
        stack: &[usize],
        cycle_start: usize,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        // One diagnostic per compile is enough to explain the problem;
        // deeper back-edges in the same component add no information.
        if diagnostics
            .iter()
            .any(|d| d.message.starts_with("cyclic type"))
        {
            return;
        }

        let start_pos = stack
            .iter()
            .position(|&idx| idx == cycle_start)
            .unwrap_or_default();
        let mut names: Vec<String> = stack[start_pos..]
            .iter()
            .map(|&idx| name_of(program, idx).to_string())
            .collect();
        names.push(name_of(program, cycle_start).to_string());

        let span = program.types[cycle_start].name.span;
        diagnostics.push(Diagnostic::error(
            format!("cyclic type: {}", names.join(" -> ")),
            span,
        ));
    }

    for start in 0..n {
        if color[start] == Color::White {
            visit(
                program,
                type_ids,
                start,
                &mut color,
                &mut stack,
                diagnostics,
            );
        }
    }
}
