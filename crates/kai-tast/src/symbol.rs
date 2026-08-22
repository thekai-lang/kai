/// Identity of a resolved top-level function. TAST never carries raw strings
/// where an id will do (whitepaper §8, constraint 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunctionId(pub u32);
