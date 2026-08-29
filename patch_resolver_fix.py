with open('crates/kai-resolver/src/tables.rs', 'r') as f:
    text = f.read()

# First we need to move the imports block down
idx_start = text.find('    // Import aliases first:')
idx_end = text.find('    for (m_idx, module) in modules.iter().enumerate() {')

imports_block = text[idx_start:idx_end]
text = text[:idx_start] + text[idx_end:]

# Now insert it at the end of the loops
idx_insert = text.find('    // §9.10 closure-bearing poisoning')
if idx_insert == -1:
    idx_insert = text.find('    resolution\n}\n')

text = text[:idx_insert] + imports_block + '\n' + text[idx_insert:]

with open('crates/kai-resolver/src/tables.rs', 'w') as f:
    f.write(text)
