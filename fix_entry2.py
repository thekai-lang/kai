import re
with open('crates/kai-resolver/src/entry.rs', 'r') as f:
    text = f.read()

text = text.replace('Ty::Named(Ident {', 'Ty::Path(vec![Ident {')
text = text.replace('name: "int32".to_string(),\n            span: Span::new(0, 0),\n        })', 'name: "int32".to_string(),\n            span: Span::new(0, 0),\n        }])')

text = text.replace('name: Ident {', 'path: vec![Ident {')
text = text.replace('name: "main".to_string(),\n                span: Span::new(0, 0),\n            },', 'name: "main".to_string(),\n                span: Span::new(0, 0),\n            }],')

text = text.replace('name: "string".to_string(),\n                span: Span::new(0, 0),\n            })', 'name: "string".to_string(),\n                span: Span::new(0, 0),\n            }])')

text = text.replace('name: "args".to_string(),\n                span: Span::new(0, 0),\n            },', 'name: "args".to_string(),\n                span: Span::new(0, 0),\n            }],')
# wait, args is a Param, wait `Param` has `name: Ident`? Yes! Param has `name`, NOT `path`!
# Let me look at Param in kai_ast
