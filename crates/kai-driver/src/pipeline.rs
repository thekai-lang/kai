//! Pipeline orchestration:
//! lex -> parse -> resolve -> typecheck -> ownership -> effects -> codegen.
//! Each phase runs only after the previous one produced no diagnostics; the
//! first failing phase's diagnostics are reported and compilation stops.
//!
//! Two front doors:
//! - `compile`/`jit` take source text (single anonymous module; `use` is a
//!   diagnostic, since imports need a project root)
//! - `compile_file`/`jit_file` take an ENTRY FILE and load its whole module
//!   tree from the project root (§3.6)

use std::path::Path;

use kai_ast::Program;
use kai_codegen::SourceUnit;
use kai_diagnostics::Diagnostic;
use kai_resolver::ModuleInput;
use kai_tast::TypedProgram;

#[derive(Debug, Clone)]
pub struct Failure {
    pub phase: &'static str,
    pub diagnostics: Vec<Diagnostic>,
    /// (display path, source) for every module in play — lets the reporter
    /// render carets for diagnostics attributed to any file. Empty for the
    /// string API, whose caller already holds the single source.
    pub sources: Vec<(String, String)>,
}

/// Full compile to textual LLVM IR (module verified).
pub fn compile(source: &str) -> Result<String, Failure> {
    let source = source.to_string();
    with_big_stack(move || {
        let program = lower(&source)?;
        kai_codegen::compile_ir_with_sources("kai_module", &program, &anon_sources(&source))
            .map_err(internal_failure)
    })
}

/// JIT-compile and execute `main`, returning its `int32` result.
pub fn jit(source: &str) -> Result<i32, Failure> {
    let source = source.to_string();
    with_big_stack(move || {
        let program = lower(&source)?;
        kai_codegen::run_jit_with_sources(&program, &anon_sources(&source))
            .map_err(internal_failure)
    })
}

/// Compiles the module tree rooted at `entry` to textual LLVM IR.
pub fn compile_file(entry: &Path) -> Result<String, Failure> {
    let entry = entry.to_path_buf();
    with_big_stack(move || {
        let modules = load_modules(&entry)?;
        let program = lower_modules(&modules)?;
        let root = entry.parent().map(|p| p.to_path_buf());
        kai_codegen::compile_ir_with_sink(
            "kai_module",
            &program,
            &module_sources(&modules),
            root.as_deref(),
        )
            .map_err(internal_failure)
    })
}

/// [`compile_file`] with an explicit sink root — golden-IR tests use a
/// fixed fake root so the baked `.kai/*.log` path globals stay deterministic
/// across machines (real runs derive the root from the entry's parent).
pub fn compile_file_with_sink(entry: &Path, sink_root: &Path) -> Result<String, Failure> {
    let entry = entry.to_path_buf();
    let sink_root = sink_root.to_path_buf();
    with_big_stack(move || {
        let modules = load_modules(&entry)?;
        let program = lower_modules(&modules)?;
        kai_codegen::compile_ir_with_sink(
            "kai_module",
            &program,
            &module_sources(&modules),
            Some(sink_root.as_path()),
        )
            .map_err(internal_failure)
    })
}

/// JIT-executes the module tree rooted at `entry`.
pub fn jit_file(entry: &Path) -> Result<i32, Failure> {
    let entry = entry.to_path_buf();
    with_big_stack(move || {
        let modules = load_modules(&entry)?;
        let program = lower_modules(&modules)?;
        let root = entry.parent().map(|p| p.to_path_buf());
        kai_codegen::run_jit_with_sink(&program, &module_sources(&modules), root.as_deref())
            .map_err(internal_failure)
    })
}

/// The string API's single anonymous module (`""`), reported as `<stdin>`.
fn anon_sources(source: &str) -> Vec<SourceUnit> {
    vec![SourceUnit {
        module: String::new(),
        file: "<stdin>".to_string(),
        text: source.to_string(),
    }]
}

fn module_sources(modules: &[crate::modules::LoadedModule]) -> Vec<SourceUnit> {
    modules
        .iter()
        .map(|m| SourceUnit {
            module: m.name.clone(),
            file: m.file.clone(),
            text: m.source.clone(),
        })
        .collect()
}

// Deeply nested input recurses through every phase before a budget trips,
// and debug builds burn thousands of bytes of stack per AST level — the
// default 2 MiB test-thread stack is not enough to reach the diagnostic
// (same reason rustc runs its own passes on a big stack). The helper lives
// with the parser, which needs it first in the pipeline.
use kai_parser::with_big_stack;

fn load_modules(
    entry: &Path,
) -> Result<Vec<kai_driver_modules::LoadedModule>, Failure> {
    crate::modules::load(entry).map_err(|load_failure| Failure {
        phase: "resolve",
        diagnostics: load_failure.diagnostics,
        sources: load_failure.sources,
    })
}

// Re-export alias keeps the import list tidy without a circular feel.
use crate::modules as kai_driver_modules;

fn lower_modules(modules: &[kai_driver_modules::LoadedModule]) -> Result<TypedProgram, Failure> {
    let sources: Vec<(String, String)> = modules
        .iter()
        .map(|m| (m.file.clone(), m.source.clone()))
        .collect();
    let fail = |phase| {
        let sources = sources.clone();
        move |diagnostics: Vec<Diagnostic>| Failure {
            phase,
            diagnostics,
            sources,
        }
    };

    // Global id order == module order (DFS pre-order), so merging is a plain
    // concatenation; per-module tables in Resolution hold indices into it.
    let merged = Program {
        use_decls: Vec::new(),
        fns: modules.iter().flat_map(|m| m.program.fns.clone()).collect(),
        types: modules
            .iter()
            .flat_map(|m| m.program.types.clone())
            .collect(),
    };

    let inputs: Vec<ModuleInput> = modules
        .iter()
        .map(|m| ModuleInput {
            name: &m.name,
            file: &m.file,
            program: &m.program,
        })
        .collect();

    let resolution =
        kai_resolver::analyze_modules(&inputs).map_err(fail("resolve"))?;

    let mut program = kai_typecheck::check_with(&merged, &resolution, load_sql_snapshots(modules), load_api_snapshots(modules))
        .map_err(fail("typecheck"))?;
    kai_ownership::resolve(&mut program);
    let effect_diags = kai_effects::analyze(&mut program);
    if !effect_diags.is_empty() {
        return Err(fail("effect")(effect_diags));
    }
    Ok(program)
}

fn lower(source: &str) -> Result<TypedProgram, Failure> {
    let fail = |phase| move |diagnostics: Vec<Diagnostic>| Failure {
        phase,
        diagnostics,
        sources: Vec::new(),
    };

    let lexed = kai_lexer::lex(source);
    if !lexed.diagnostics.is_empty() {
        return Err(fail("lex")(lexed.diagnostics));
    }

    let ast = kai_parser::parse(&lexed.tokens).map_err(fail("parse"))?;

    // The string API cannot resolve imports (they need a project root, i.e.
    // a file entry point). A predictable user-facing situation reports as a
    // diagnostic, never an internal error (§8).
    if let Some(first) = ast.use_decls.first() {
        return Err(fail("resolve")(vec![Diagnostic::error(
            "modules require a file entry point; run kai build or kai run on an entry file",
            first.span,
        )]));
    }

    let resolution = kai_resolver::analyze(&ast).map_err(fail("resolve"))?;

    let mut program =
        kai_typecheck::check_with(&ast, &resolution, std::collections::HashMap::new(), std::collections::HashMap::new()).map_err(fail("typecheck"))?;
    kai_ownership::resolve(&mut program);
    let effect_diags = kai_effects::analyze(&mut program);
    if !effect_diags.is_empty() {
        return Err(fail("effect")(effect_diags));
    }
    Ok(program)
}

fn internal_failure(err: String) -> Failure {
    Failure {
        phase: "codegen",
        diagnostics: vec![internal(err)],
        sources: Vec::new(),
    }
}

fn internal(message: String) -> Diagnostic {
    Diagnostic::error(
        format!("internal codegen error: {message}"),
        kai_diagnostics::Span::new(0, 0),
    )
}


fn load_api_snapshots(modules: &[crate::modules::LoadedModule]) -> std::collections::HashMap<(String, u32), kai_typecheck::api::snapshot::ApiSnapshot> {
    let mut snapshots = std::collections::HashMap::new();
    if let Some(first) = modules.first() {
        let entry_path = std::path::Path::new(&first.file);
        if let Some(parent) = entry_path.parent() {
            let snap_dir = parent.join(".kai").join("snapshots").join("api");
            if snap_dir.exists() && snap_dir.is_dir()
                && let Ok(services) = std::fs::read_dir(snap_dir) {
                    for service_entry in services.flatten() {
                        let svc_path = service_entry.path();
                        if svc_path.is_dir() {
                            let service_name = svc_path.file_name().unwrap().to_str().unwrap().to_string();
                            if let Ok(entries) = std::fs::read_dir(svc_path) {
                                for entry in entries.flatten() {
                                    let path = entry.path();
                                    if path.extension().and_then(|e| e.to_str()) == Some("json")
                                        && let Some(stem) = path.file_stem().and_then(|s| s.to_str())
                                            && stem.starts_with('v')
                                                && let Ok(version) = stem[1..].parse::<u32>()
                                                    && let Ok(content) = std::fs::read_to_string(&path)
                                                        && let Ok(snapshot) = kai_typecheck::api::snapshot::parse_snapshot(&content) {
                                                            snapshots.insert((service_name.clone(), version), snapshot);
                                                        }
                                }
                            }
                        }
                    }
                }
        }
    }
    snapshots
}

fn load_sql_snapshots(modules: &[crate::modules::LoadedModule]) -> std::collections::HashMap<u32, kai_typecheck::sql::snapshot::SqlSnapshot> {
    let mut snapshots = std::collections::HashMap::new();
    
    // We infer the root directory from the first module's path (which is the entry point).
    if let Some(first) = modules.first() {
        let entry_path = std::path::Path::new(&first.file);
        if let Some(parent) = entry_path.parent() {
            let snap_dir = parent.join(".kai").join("snapshots").join("sql");
            if snap_dir.exists() && snap_dir.is_dir()
                && let Ok(entries) = std::fs::read_dir(snap_dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.extension().and_then(|e| e.to_str()) == Some("json") {
                            // Extract version number from filename, e.g. "v12.json" -> 12
                            if let Some(stem) = path.file_stem().and_then(|s| s.to_str())
                                && stem.starts_with('v')
                                    && let Ok(version) = stem[1..].parse::<u32>()
                                        && let Ok(content) = std::fs::read_to_string(&path)
                                            && let Ok(snapshot) = kai_typecheck::sql::snapshot::parse_snapshot(&content) {
                                                snapshots.insert(version, snapshot);
                                            }
                        }
                    }
                }
        }
    }
    
    snapshots
}


/// Validates the module tree up to the typechecker phase without code generation.
/// If `schema_check` is true, it also validates snapshot drift against a current reference.
pub fn check_file(entry: &Path, _schema_check: bool) -> Result<(), Failure> {
    let entry = entry.to_path_buf();
    with_big_stack(move || {
        let modules = load_modules(&entry)?;
        let _program = lower_modules(&modules)?;
        // TODO: invoke schema drift engine if _schema_check is true
        Ok(())
    })
}
