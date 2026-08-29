#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DurationUnit {
    Ms,
    S,
    M,
    H,
    D,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DurationLit {
    pub value: u64,
    pub unit: DurationUnit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemporalOrigin {
    Local,
    Wallclock,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    EscapesLocalContext,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EffectSet(pub Vec<Effect>);

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
    /// `Optional<T>` / `T?` (v0.0.6, §9.9a): tagged inline aggregate
    /// `{ tag, T payload }`. Heap-bearing iff the instantiated payload is.
    /// One semantic form — `T?` desugared away at parse time.
    Optional(Box<KaiType>),
    /// `Result<T, E>` (v0.0.6, §9.9a): `{ tag, T ok, E err }`, no sugar.
    /// Heap-bearing iff either instantiated payload is.
    Result { ok: Box<KaiType>, err: Box<KaiType> },
    /// Closure type (v0.0.6, §9.10): values are fat pointers
    /// `{ fn_ptr, env_ptr }` and UNCONDITIONALLY heap-bearing regardless of
    /// what they capture (mirrors array's rule).
    Closure { params: Vec<KaiType>, ret: Box<KaiType> },
    /// `T @local(d)` / `T @wallclock(d)` (v0.0.7, §5.1): temporal wrapper.
    /// The inner type's heap/stack semantics are unchanged; the wrapper adds
    /// the `Origin` and `Duration` for effect checking. `T @wallclock` is
    /// always heap-bearing due to embedded timestamp (RFC3339 string).
    Temporal {
        inner: Box<KaiType>,
        origin: TemporalOrigin,
        duration: DurationLit,
    },
}

impl KaiType {
    /// Is this one of the two builtin parameterized types or their closures?
    /// Used by diagnostics to describe receivers (`unwrap_or`, `catch`, ...).
    pub fn is_tagged_union(&self) -> bool {
        matches!(self, KaiType::Optional(_) | KaiType::Result { .. })
    }
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

impl KaiType {
    pub const NAMESPACE: Self = KaiType::Unit;
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
            KaiType::Optional(inner) => write!(f, "{inner}?"),
            KaiType::Result { ok, err } => write!(f, "Result<{ok}, {err}>"),
            KaiType::Closure { params, ret } => {
                let names: Vec<String> = params.iter().map(|p| p.to_string()).collect();
                write!(f, "({}) -> {ret}", names.join(", "))
            }
            KaiType::Temporal { inner, origin, duration } => {
                let origin_str = match origin {
                    TemporalOrigin::Local => "local",
                    TemporalOrigin::Wallclock => "wallclock",
                };
                let unit_str = match duration.unit {
                    DurationUnit::Ms => "ms",
                    DurationUnit::S => "s",
                    DurationUnit::M => "m",
                    DurationUnit::H => "h",
                    DurationUnit::D => "d",
                };
                write!(f, "{inner} @{origin_str}({}{unit_str})", duration.value)
            }
            // The struct NAME needs the declaration table, which lives with
            // the type checker; generic display keeps this enum standalone.
            KaiType::Struct(_) => write!(f, "struct"),
        }
    }
}

impl std::fmt::Display for Effect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Effect::EscapesLocalContext => write!(f, "escapes-local-context"),
        }
    }
}

impl std::fmt::Display for EffectSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let parts: Vec<String> = self.0.iter().map(|e| e.to_string()).collect();
        write!(f, "{{{}}}", parts.join(", "))
    }
}
