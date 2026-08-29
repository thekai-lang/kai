import re
with open('crates/kai-typecheck/src/ty.rs', 'r') as f:
    text = f.read()

text = text.replace('checker.ctx.module_id', 'checker.current_module')
text = text.replace('checker.ctx.resolution', 'checker.resolution')
text = text.replace('error::custom(format!("type `{type_name}` is private"), path[1].span)', 'kai_diagnostics::Diagnostic::error(format!("type `{type_name}` is private"), path[1].span).with_file(&checker.cur_file)')
text = text.replace('error::custom(format!("module `{alias}` has no type `{type_name}`"), path[1].span)', 'kai_diagnostics::Diagnostic::error(format!("module `{alias}` has no type `{type_name}`"), path[1].span).with_file(&checker.cur_file)')
text = text.replace('error::custom(format!("unknown module alias `{alias}`"), path[0].span)', 'kai_diagnostics::Diagnostic::error(format!("unknown module alias `{alias}`"), path[0].span).with_file(&checker.cur_file)')
text = text.replace('error::custom("invalid type path length", span)', 'kai_diagnostics::Diagnostic::error("invalid type path length".to_string(), span).with_file(&checker.cur_file)')
text = text.replace('checker.error(kai_diagnostics::Diagnostic::error', 'checker.diagnostics.push(kai_diagnostics::Diagnostic::error')

with open('crates/kai-typecheck/src/ty.rs', 'w') as f:
    f.write(text)
