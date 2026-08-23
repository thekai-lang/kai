/// Concrete, resolved types. This enum is the single source of truth for what
/// a value is; surface names and aliases (`int`) never reach codegen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KaiType {
    Int32,
    Int64,
    Float64,
    Bool,
    Unit,
    /// Heap-allocated UTF-8-ish byte string with a refcount header (§9.7).
    /// Values are POINTERS to `{ rc, len, data }` headers.
    String,
    /// `T[]` (v0.0.5): unconditionally heap-bearing, values are pointers to
    /// `{ rc, len, elems }` headers. Boxed because arrays nest.
    Array(Box<KaiType>),
    /// Nominal struct type; the layout (field order + types) lives with the
    /// type checker and is mirrored into LLVM by codegen.
    Struct(crate::symbol::StructId),
}

impl KaiType {
    pub fn is_integer(&self) -> bool {
        matches!(self, KaiType::Int32 | KaiType::Int64)
    }

    pub fn is_numeric(&self) -> bool {
        self.is_integer() || *self == KaiType::Float64
    }

    pub fn is_struct(self) -> bool {
        matches!(self, KaiType::Struct(_))
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
            KaiType::String => write!(f, "string"),
            KaiType::Array(elem) => write!(f, "{elem}[]"),
            // The struct NAME needs the declaration table, which lives with
            // the type checker; generic display keeps this enum standalone.
            KaiType::Struct(_) => write!(f, "struct"),
        }
    }
}
