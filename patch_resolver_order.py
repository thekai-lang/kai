with open('crates/kai-resolver/src/tables.rs', 'r') as f:
    text = f.read()

idx_imports_start = text.find('    // Import aliases first:')
idx_imports_end = text.find('    for (m_idx, module) in modules.iter().enumerate() {')
if idx_imports_start != -1 and idx_imports_end != -1:
    imports_block = text[idx_imports_start:idx_imports_end]
    # Remove imports block
    text = text[:idx_imports_start] + text[idx_imports_end:]
    # Insert it after fn registration
    idx_insert = text.find('    resolution\n}\n')
    text = text[:idx_insert] + imports_block + text[idx_insert:]
    with open('crates/kai-resolver/src/tables.rs', 'w') as f:
        f.write(text)
