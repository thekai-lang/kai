//! Module loading (v0.0.4): resolves `use` declarations to files under the
//! project root — the entry file's directory, never the process CWD (§3.6) —
//! and detects import cycles as diagnostics, not stack overflows.
//!
//! `use a.b;` loads `<root>/a/b.kai`; its alias is the last path segment.
//! Modules load once each (diamond imports are fine); revisiting a module
//! that is still on the DFS stack IS a cycle and reports the whole chain.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use kai_ast::Program;
use kai_diagnostics::{Diagnostic, Span};

/// One parsed source file. `name` is the dotted import path (`""` for the
/// entry); `file` is the user-facing display path relative to the root.
#[derive(Debug)]
pub struct LoadedModule {
    pub name: String,
    pub file: String,
    pub source: String,
    pub program: Program,
}

/// Load failure plus every file the loader touched (display path -> source),
/// so diagnostics from ANY visited module can render a caret snippet.
#[derive(Debug)]
pub struct LoadFailure {
    pub diagnostics: Vec<Diagnostic>,
    pub sources: Vec<(String, String)>,
}

/// Loads the entry module plus every transitively imported one, in DFS
/// pre-order (entry first). Lex/parse failures are attributed to their own
/// file via `Diagnostic::file`.
pub fn load(entry: &Path) -> Result<Vec<LoadedModule>, LoadFailure> {
    let source = match std::fs::read_to_string(entry) {
        Ok(source) => source,
        Err(err) => {
            return Err(LoadFailure {
                diagnostics: vec![Diagnostic::error(
                    format!("cannot read entry file `{}`: {err}", entry.display()),
                    Span::new(0, 0),
                )],
                sources: Vec::new(),
            })
        }
    };

    let root = entry.parent().unwrap_or_else(|| Path::new("."));
    // The entry lives directly under the root by definition, so its display
    // path is just the trailing portion — same convention as imports.
    let entry_display = display_path(entry.strip_prefix(root).unwrap_or(entry));

    let mut ctx = Loader {
        root: root.to_path_buf(),
        loaded: Vec::new(),
        finished: HashSet::new(),
        on_stack: HashMap::new(), // name -> position in the visit chain
        chain: Vec::new(),
        sources: Vec::new(),
    };

    ctx.load_module("", entry_display, source)
        .map(|()| ctx.loaded)
        .map_err(|diagnostics| LoadFailure {
            diagnostics,
            sources: std::mem::take(&mut ctx.sources),
        })
}

struct Loader {
    root: PathBuf,
    /// Finished modules in DFS pre-order.
    loaded: Vec<LoadedModule>,
    /// Dotted names fully processed (diamond imports reuse these).
    finished: HashSet<String>,
    /// Dotted names currently on the DFS stack -> chain depth.
    on_stack: HashMap<String, usize>,
    /// Names on the current DFS path, for cycle-chain rendering.
    chain: Vec<String>,
    /// (display path, source) for EVERY file entered — recorded before any
    /// parse can fail, so error rendering always finds its snippet.
    sources: Vec<(String, String)>,
}

impl Loader {
    fn load_module(
        &mut self,
        name: &str,
        file: String,
        source: String,
    ) -> Result<(), Vec<Diagnostic>> {
        self.sources.push((file.clone(), source.clone()));
        let program = parse_source(&source, &file)?;
        let uses = program.use_decls.clone();

        // Pre-order: this module lands in the list BEFORE everything it
        // imports, giving later phases a deterministic global id order.
        self.loaded.push(LoadedModule {
            name: name.to_string(),
            file: file.clone(),
            source,
            program,
        });

        let depth = self.chain.len();
        self.on_stack.insert(name.to_string(), depth);
        self.chain.push(name.to_string());
        let result = self.visit_imports(&file, &uses);
        self.chain.pop();
        self.on_stack.remove(name);
        self.finished.insert(name.to_string());
        result
    }

    fn visit_imports(
        &mut self,
        importer_file: &str,
        uses: &[kai_ast::UseDecl],
    ) -> Result<(), Vec<Diagnostic>> {
        for decl in uses {
            let target = decl.dotted_name();
            let expected = format!("{}.kai", target.replace('.', "/"));

            if let Some(&depth) = self.on_stack.get(&target) {
                let mut chain: Vec<String> = self.chain[depth.min(self.chain.len())..].to_vec();
                chain.push(target.clone());
                return Err(vec![
                    Diagnostic::error(format!("cyclic import: {}", chain.join(" -> ")), decl.span)
                        .with_file(importer_file),
                ]);
            }
            if self.finished.contains(&target) {
                continue;
            }

            if target == "std.io" {
                self.load_module(
                    "std.io",
                    "std.io".to_string(),
                    "public fn print(s: string) -> unit {}\npublic fn println(s: string) -> unit {}\n".to_string(),
                )?;
                continue;
            }

            let path = self.root.join(&expected);
            match std::fs::read_to_string(&path) {
                Ok(source) => {
                    self.load_module(&target, expected, source)?;
                }
                Err(_) => {
                    return Err(vec![
                        Diagnostic::error(format!("cannot find module `{target}`"), decl.span)
                            .with_file(importer_file),
                    ]);
                }
            }
        }
        Ok(())
    }
}

/// Lex + parse one file, attributing any diagnostics to it.
fn parse_source(source: &str, file: &str) -> Result<Program, Vec<Diagnostic>> {
    let lexed = kai_lexer::lex(source);
    if !lexed.diagnostics.is_empty() {
        return Err(lexed
            .diagnostics
            .into_iter()
            .map(|d| d.with_file(file))
            .collect());
    }
    kai_parser::parse(&lexed.tokens)
        .map_err(|diags| diags.into_iter().map(|d| d.with_file(file)).collect())
}

/// Forward-slash display path, stable across platforms for messages/tests.
fn display_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    fn temp_root(tag: &str) -> PathBuf {
        static N: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "kai-modules-{}-{}-{}",
            tag,
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp root");
        dir
    }

    fn write(root: &Path, rel: &str, text: &str) -> PathBuf {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdirs");
        std::fs::write(&path, text).expect("write fixture");
        path
    }

    #[test]
    fn loads_entry_then_imports_in_dfs_preorder() {
        let root = temp_root("preorder");
        let entry = write(
            &root,
            "main.kai",
            "use support.math;\nfn main() -> int32 { return 0; }",
        );
        write(
            &root,
            "support/math.kai",
            "public fn add(a: int32, b: int32) -> int32 { return a + b; }",
        );

        let modules = load(&entry).expect("loads");
        assert_eq!(modules.len(), 2);
        assert_eq!(modules[0].name, "");
        assert!(modules[0].file.ends_with("main.kai"));
        assert_eq!(modules[1].name, "support.math");
        assert_eq!(modules[1].file, "support/math.kai");
    }

    #[test]
    fn diamond_import_loads_shared_module_once() {
        let root = temp_root("diamond");
        let entry = write(
            &root,
            "main.kai",
            "use left.base;\nuse right.base;\nfn main() -> int32 { return 0; }",
        );
        write(
            &root,
            "left/base.kai",
            "use shared.core;\npublic fn l() -> int32 { return 1; }",
        );
        write(
            &root,
            "right/base.kai",
            "use shared.core;\npublic fn r() -> int32 { return 2; }",
        );
        write(
            &root,
            "shared/core.kai",
            "public fn c() -> int32 { return 3; }",
        );

        let modules = load(&entry).expect("loads");
        let names: Vec<&str> = modules.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(
            names,
            vec!["", "left.base", "shared.core", "right.base"],
            "shared module must appear exactly once, before its second importer"
        );
    }

    #[test]
    fn circular_import_reports_chain_not_stack_overflow() {
        let root = temp_root("cycle");
        let entry = write(
            &root,
            "main.kai",
            "use mod.a;\nfn main() -> int32 { return 0; }",
        );
        write(
            &root,
            "mod/a.kai",
            "use mod.b;\npublic fn a() -> int32 { return 1; }",
        );
        write(
            &root,
            "mod/b.kai",
            "use mod.a;\npublic fn b() -> int32 { return 2; }",
        );

        let err = load(&entry).unwrap_err();
        let msg = &err.diagnostics[0].message;
        assert!(msg.contains("cyclic import"), "got: {msg}");
        assert!(msg.contains("mod.a"), "chain names the cycle: {msg}");
        // The diagnostic points at the importing file's use statement.
        assert_eq!(
            err.diagnostics[0].file.as_deref(),
            Some("mod/b.kai")
        );
    }

    #[test]
    fn missing_module_points_at_use_site() {
        let root = temp_root("missing");
        let entry = write(
            &root,
            "main.kai",
            "use ghost.thing;\nfn main() -> int32 { return 0; }",
        );

        let err = load(&entry).unwrap_err();
        assert_eq!(
            err.diagnostics[0].message,
            "cannot find module `ghost.thing`"
        );
        assert_eq!(err.diagnostics[0].file.as_deref(), Some("main.kai"));
        assert!(
            err.diagnostics[0].span.end > 0,
            "span covers the use declaration"
        );
        // Failure carries the entry source so reports can render carets.
        assert_eq!(err.sources.len(), 1);
        assert_eq!(err.sources[0].0, "main.kai");
    }

    #[test]
    fn parse_error_inside_import_is_attributed_to_that_file() {
        let root = temp_root("parse");
        let entry = write(
            &root,
            "main.kai",
            "use bad.mod;\nfn main() -> int32 { return 0; }",
        );
        write(&root, "bad/mod.kai", "public fn f( -> int32 { return 1; }");

        let err = load(&entry).unwrap_err();
        assert!(!err.diagnostics.is_empty());
        assert!(err
            .diagnostics
            .iter()
            .all(|d| d.file.as_deref() == Some("bad/mod.kai")));
        // Both files were entered and are available for rendering.
        assert_eq!(err.sources.len(), 2);
    }
}
