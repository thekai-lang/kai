with open('crates/kai-resolver/src/tables.rs', 'r') as f:
    text = f.read()

# First we need to extract the imports loop from the main loop
imports_loop_start = text.find('        for decl in &module.program.use_decls {')
imports_loop_end = text.find('        for decl in &module.program.types {')
imports_loop = text[imports_loop_start:imports_loop_end]

# Remove the imports loop from the main loop
text = text[:imports_loop_start] + text[imports_loop_end:]

# Now let's find the `name_to_index` which was moved to the bottom
name_to_index_start = text.find('    let name_to_index: HashMap<&str, usize> = modules')
name_to_index_end = text.find('        .collect();') + 19
name_to_index_block = text[name_to_index_start:name_to_index_end]

# Remove it from the bottom
text = text[:name_to_index_start] + text[name_to_index_end:]

# Insert BOTH after the main loop finishes, right before cycle detection
idx_insert = text.find('    // §9.10 closure-bearing poisoning')
if idx_insert == -1:
    idx_insert = text.find('    resolution\n}\n')

imports_block = name_to_index_block + '\n    for (m_idx, module) in modules.iter().enumerate() {\n' + imports_loop + '    }\n'
text = text[:idx_insert] + imports_block + '\n' + text[idx_insert:]

with open('crates/kai-resolver/src/tables.rs', 'w') as f:
    f.write(text)
