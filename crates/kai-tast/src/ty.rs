/// Concrete, resolved types. This enum is the single source of truth for what
/// a value is; surface names and aliases (`int`) never reach codegen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KaiType {
    Int32,
}

impl std::fmt::Display for KaiType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KaiType::Int32 => write!(f, "int32"),
        }
    }
}
