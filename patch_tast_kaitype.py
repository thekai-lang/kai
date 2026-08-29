with open('crates/kai-tast/src/ty.rs', 'r') as f:
    text = f.read()

text = text.replace('    pub fn is_struct(self) -> bool {\n        matches!(self, KaiType::Struct(_))\n    }\n}', '    pub fn is_struct(self) -> bool {\n        matches!(self, KaiType::Struct(_))\n    }\n}\n\nimpl KaiType {\n    pub const Namespace: Self = KaiType::Unit;\n}')
with open('crates/kai-tast/src/ty.rs', 'w') as f:
    f.write(text)
