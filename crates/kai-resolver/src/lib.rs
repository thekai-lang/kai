//! Name/entry-point resolution over the untyped AST.
//! Produces diagnostics; never mutates the AST (that is the type checker's job).

pub mod entry;

pub use entry::check_entry;
