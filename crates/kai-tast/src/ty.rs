/// Concrete, resolved types. This enum is the single source of truth for what
/// a value is; surface names and aliases (`int`) never reach codegen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KaiType {
    Int32,
    Int64,
    Float64,
    Bool,
    Unit,
}

impl KaiType {
    pub fn is_integer(self) -> bool {
        matches!(self, KaiType::Int32 | KaiType::Int64)
    }

    pub fn is_numeric(self) -> bool {
        self.is_integer() || self == KaiType::Float64
    }
}

impl std::fmt::Display for KaiType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KaiType::Int32 => write!(f, "int32"),
            KaiType::Int64 => write!(f, "int64"),
            KaiType::Float64 => write!(f, "float64"),
            KaiType::Bool => write!(f, "bool"),
            KaiType::Unit => write!(f, "unit"),
        }
    }
}
