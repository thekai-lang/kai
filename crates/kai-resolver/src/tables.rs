//! Namespace tables across modules, visibility masks, and struct dependency
//! cycles (§9.2, §3.6).

use std::collections::HashMap;

use kai_ast::{Program, Ty};
use kai_diagnostics::Diagnostic;

/// One compilation unit handed to [`analyze_modules`]. The driver loads
/// modules in DFS pre-order (entry first) and mirrors that order here;
/// global declaration ids follow the same order.
pub struct ModuleInput<'a> {
    /// Dotted import path (`""` for the entry module).
    pub name: &'a str,
    /// Display path relative to the project root — stamped onto every
    /// diagnostic originating from this module's declarations.
    pub file: &'a str,
    pub program: &'a Program,
}

/// Name tables produced by resolution. Indices point into the MERGED
/// program (all modules' declarations concatenated in module order), so the
/// type checker turns surface names into global ids without re-walking.
///
/// Unqualified lookups NEVER leave the declaring module (§3.6: imports do
/// not inject into any scope); cross-module access goes through an import
/// alias plus the member's `public` flag. Types/functions live in separate
/// namespaces within a module; import aliases form a third namespace of
/// their own.
#[derive(Debug, Clone)]
pub struct Resolution {
    /// One entry per module: local name -> GLOBAL type index.
    pub module_types: Vec<HashMap<String, usize>>,
    /// One entry per module: local name -> GLOBAL fn index.
    pub module_fns: Vec<HashMap<String, usize>>,
    /// One entry per module: import alias -> target module index.
    pub imports: Vec<HashMap<String, usize>>,
    /// Global id -> visibility flag.
    pub type_is_public: Vec<bool>,
    pub fn_is_public: Vec<bool>,
    /// Global id -> owning module index.
    pub type_module: Vec<usize>,
    pub fn_module: Vec<usize>,
    /// Global id -> display file (diagnostics attribution, §8.6).
    pub type_file: Vec<String>,
    pub fn_file: Vec<String>,
    /// Module index -> dotted name ("" is the entry module).
    pub module_names: Vec<String>,
    /// §9.10 poisoning (v0.0.6): per global type index, true when the
    /// struct transitively contains a closure type through a heap-bearing
    /// member path (field, array ELEMENT, Optional/Result payload). A
    /// closure literal may not capture a value of such a type — that is the
    /// structural precondition for an RC cycle. Computed after cycle
    /// detection; legacy single() callers carry an empty vector.
    pub closure_bearing: Vec<bool>,
}

impl Default for Resolution {
    /// Always at least ONE module — lookups index per-module tables and must
    /// never see an empty list (legacy no-resolution callers included).
    fn default() -> Self {
        Self::single()
    }
}

impl Resolution {
    /// A single anonymous module — the pre-v0.0.4 shape every legacy caller
    /// (and `Default`) expects.
    pub fn single() -> Self {
        Self {
            module_types: vec![HashMap::new()],
            module_fns: vec![HashMap::new()],
            imports: vec![HashMap::new()],
            type_is_public: Vec::new(),
            fn_is_public: Vec::new(),
            type_module: Vec::new(),
            fn_module: Vec::new(),
        type_file: Vec::new(),
        fn_file: Vec::new(),
        module_names: vec![String::new()],
        closure_bearing: Vec::new(),
    }
    }

    /// File that owns global fn `id`.
    pub fn fn_file_of(&self, id: usize) -> Option<&str> {
        self.fn_file.get(id).map(String::as_str)
    }
}

#[cfg(test)]
pub(crate) fn build_single(
    program: &Program,
    diagnostics: &mut Vec<Diagnostic>,
) -> Resolution {
    let input = ModuleInput {
        name: "",
        file: "",
        program,
    };
    build_multi(&[input], diagnostics)
}

pub(crate) fn build_multi(
    modules: &[ModuleInput],
    diagnostics: &mut Vec<Diagnostic>,
) -> Resolution {
    let mut resolution = Resolution {
        module_names: modules.iter().map(|m| m.name.to_string()).collect(),
        ..Resolution::single()
    };
    // Resize the per-module tables beyond the single() seed.
    resolution.module_types.resize_with(modules.len(), HashMap::new);
    resolution.module_fns.resize_with(modules.len(), HashMap::new);
    resolution.imports.resize_with(modules.len(), HashMap::new);

    // Import aliases first: they are validated against the loaded module
    // list, which also catches imports pointing outside the loaded set.
    let name_to_index: HashMap<&str, usize> = modules
        .iter()
        .enumerate()
        .map(|(idx, m)| (m.name, idx))
        .collect();

    for (m_idx, module) in modules.iter().enumerate() {
        for decl in &module.program.use_decls {
            let target = decl.dotted_name();
            let Some(alias) = decl.path.last().cloned() else {
                diagnostics.push(
                    Diagnostic::error(
                        "internal error: empty import path — compiler bug",
                        decl.span,
                    )
                    .with_file(module.file),
                );
                continue;
            };

            let Some(&target_idx) = name_to_index.get(target.as_str()) else {
                diagnostics.push(
                    Diagnostic::error(
                        format!("cannot find module `{target}`"),
                        decl.span,
                    )
                    .with_file(module.file),
                );
                continue;
            };
            if target_idx == m_idx {
                diagnostics.push(
                    Diagnostic::error(
                        format!("cyclic import: {target} -> {target}"),
                        decl.span,
                    )
                    .with_file(module.file),
                );
                continue;
            }

            if resolution.imports[m_idx]
                .insert(alias.name.clone(), target_idx)
                .is_some()
            {
                diagnostics.push(
                    Diagnostic::error(
                        format!("duplicate import alias `{}`", alias.name),
                        alias.span,
                    )
                    .with_file(module.file),
                );
            }
        }
    }

    for (m_idx, module) in modules.iter().enumerate() {
        for decl in &module.program.types {
            let global = resolution.type_module.len();
            if resolution.module_types[m_idx]
                .insert(decl.name.name.clone(), global)
                .is_some()
            {
                diagnostics.push(
                    Diagnostic::error(
                        format!("duplicate type `{}`", decl.name.name),
                        decl.name.span,
                    )
                    .with_file(module.file),
                );
            }
            resolution.type_is_public.push(decl.is_public);
            resolution.type_module.push(m_idx);
            resolution.type_file.push(module.file.to_string());
            check_duplicate_fields(decl, module.file, diagnostics);
        }

        for decl in &module.program.fns {
            let global = resolution.fn_module.len();
            if resolution.module_fns[m_idx]
                .insert(decl.name.name.clone(), global)
                .is_some()
            {
                diagnostics.push(
                    Diagnostic::error(
                        format!("duplicate function `{}`", decl.name.name),
                        decl.name.span,
                    )
                    .with_file(module.file),
                );
            }
            resolution.fn_is_public.push(decl.is_public);
            resolution.fn_module.push(m_idx);
            resolution.fn_file.push(module.file.to_string());
        }
    }

    resolution
}

fn check_duplicate_fields(
    decl: &kai_ast::TypeDecl,
    file: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut seen: HashMap<&str, ()> = HashMap::new();
    for field in &decl.fields {
        if seen.insert(field.name.name.as_str(), ()).is_some() {
            diagnostics.push(
                Diagnostic::error(
                    format!(
                        "duplicate field `{}` in type `{}`",
                        field.name.name, decl.name.name
                    ),
                    field.name.span,
                )
                .with_file(file),
            );
        }
    }
}

/// DFS over the struct-dependency graph (edges: a field's named type that is
/// itself a declared struct IN THE SAME MODULE — unqualified type references
/// cannot cross module boundaries, so cycles cannot either). Cycles are/// impossible to lay out, so they are a compile error reporting the path:
/// `cyclic type: A -> B -> A`.
///
/// Unknown names are not edges — "unknown type" is reported later, per use.
pub(crate) fn detect_cycles(
    program: &Program,
    resolution: &Resolution,
    diagnostics: &mut Vec<Diagnostic>,
) {
    #[derive(Clone, Copy, PartialEq)]
    enum Color {
        White,
        Gray,
        Black,
    }

    // Per-module DFS: field types resolve through the owning module's own
    // table, so the dependency graph decomposes along module lines.
    for (m_idx, table) in resolution.module_types.iter().enumerate() {
        let owned: Vec<usize> = (0..program.types.len())
            .filter(|&g| resolution.type_module[g] == m_idx)
            .collect();
        let mut color = vec![Color::White; program.types.len()];
        let mut stack: Vec<usize> = Vec::new();

        fn edges(program: &Program, table: &HashMap<String, usize>, idx: usize) -> Vec<usize> {
            program.types[idx]
                .fields
                .iter()
                .flat_map(|field| ty_edges(&field.ty, table))
                .collect()
        }

        /// Which declared structs does this field type embed BY VALUE?
        /// Arrays are pointers to heap headers (§9.1) and closure values are
        /// fixed-size fat pointers (`{fn, env}`, v0.0.6) — neither closes a
        /// value-layout cycle. Tagged unions (`Optional`/`Result`, §9.9a)
        /// are inline aggregates: their payload sits inside the containing
        /// value's layout, so they inherit the payload's edges.
        fn ty_edges(ty: &Ty, table: &HashMap<String, usize>) -> Vec<usize> {
            match ty {
                Ty::Named(ident) => table.get(&ident.name).copied().into_iter().collect(),
                Ty::Array(_) | Ty::Closure { .. } => Vec::new(),
                Ty::Optional(inner) => ty_edges(inner, table),
                Ty::Result { ok, err } => {
                    let mut out = ty_edges(ok, table);
                    out.extend(ty_edges(err, table));
                    out
                }
            }
        }

        fn visit(
            program: &Program,
            table: &HashMap<String, usize>,
            idx: usize,
            color: &mut [Color],
            stack: &mut Vec<usize>,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            color[idx] = Color::Gray;
            stack.push(idx);

            for next in edges(program, table, idx) {
                match color[next] {
                    Color::White => visit(program, table, next, color, stack, diagnostics),
                    Color::Gray => report_cycle(program, stack, next, diagnostics),
                    Color::Black => {}
                }
            }

            stack.pop();
            color[idx] = Color::Black;
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
                .map(|&idx| program.types[idx].name.name.clone())
                .collect();
            names.push(program.types[cycle_start].name.name.clone());

            let span = program.types[cycle_start].name.span;
            diagnostics.push(Diagnostic::error(
                format!("cyclic type: {}", names.join(" -> ")),
                span,
            ));
        }

        for &global in &owned {
            if color[global] == Color::White {
                visit(
                    program,
                    table,
                    global,
                    &mut color,
                    &mut stack,
                    diagnostics,
                );
            }
        }
    }
}

/// §9.10 poisoning (v0.0.6): computes `Resolution::closure_bearing`.
///
/// A struct is closure-bearing when any field transitively contains a
/// closure type through a heap-bearing member path — a struct field, an
/// ARRAY ELEMENT, or an Optional/Result payload. Note the deliberate
/// contrast with the layout-cycle DFS above: arrays are pointers there
/// (they never close a *layout* cycle) but propagate here, because an
/// array of closures shares its buffer mutably — exactly the container a
/// `n.action = fn() { n.arr[i] }`-shaped cycle needs. Closure types in fn
/// signatures poison nothing: functions are not capturable containers.
///
/// Runs after `detect_cycles` in the pipeline; cyclic programs never reach
/// the checker, so the Busy guard below is pure defense.
pub(crate) fn compute_closure_bearing(
    modules: &[ModuleInput],
    resolution: &Resolution,
) -> Vec<bool> {
    #[derive(Clone, Copy, PartialEq)]
    enum State {
        Todo,
        Busy,
        Done,
    }

    let total = resolution.type_module.len();
    // Same concatenation order build_multi used to assign global indices.
    let decls: Vec<&kai_ast::TypeDecl> = modules
        .iter()
        .flat_map(|m| m.program.types.iter())
        .collect();

    fn ty_poisons(
        ty: &Ty,
        module_idx: usize,
        resolution: &Resolution,
        decls: &[&kai_ast::TypeDecl],
        state: &mut [State],
        out: &mut [bool],
    ) -> bool {
        match ty {
            Ty::Closure { .. } => true,
            Ty::Array(elem) => ty_poisons(elem, module_idx, resolution, decls, state, out),
            Ty::Optional(inner) => ty_poisons(inner, module_idx, resolution, decls, state, out),
            Ty::Result { ok, err } => {
                ty_poisons(ok, module_idx, resolution, decls, state, out)
                    || ty_poisons(err, module_idx, resolution, decls, state, out)
            }
            // Field type names resolve through the OWNING module's table —
            // unqualified references never cross modules (§3.6).
            Ty::Named(ident) => match resolution.module_types[module_idx].get(&ident.name) {
                Some(&global) => visit(global, resolution, decls, state, out),
                None => false, // unknown type reported later, per use
            },
        }
    }

    fn visit(
        global: usize,
        resolution: &Resolution,
        decls: &[&kai_ast::TypeDecl],
        state: &mut [State],
        out: &mut [bool],
    ) -> bool {
        match state[global] {
            State::Done => return out[global],
            State::Busy => return false, // cycle: rejected by detect_cycles
            State::Todo => {}
        }
        state[global] = State::Busy;
        let module_idx = resolution.type_module[global];
        let mut bearing = false;
        for field in &decls[global].fields {
            if ty_poisons(
                &field.ty,
                module_idx,
                resolution,
                decls,
                state,
                out,
            ) {
                bearing = true;
                break;
            }
        }
        out[global] = bearing;
        state[global] = State::Done;
        bearing
    }

    let mut state = vec![State::Todo; total];
    let mut out = vec![false; total];
    for global in 0..total {
        visit(global, resolution, &decls, &mut state, &mut out);
    }
    out
}
