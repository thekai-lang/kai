with open('docs/kai-changelog.md', 'r') as f:
    text = f.read()

v12_entry = """## v0.0.12 — Module Behavior, Qualified Types & Associated Functions

- **Direct Symbol Import (`use a.b.C`)**: Introduced direct imports of types and functions. Rather than importing a module alias, developers can now directly import a specific symbol into their namespace, lowering boilerplate for commonly used items.
- **Qualified Types (`a.b.User`)**: The parser and resolver were updated to support paths in type annotations. Qualified types ensure precise resolution for types spread across multiple modules, preventing naming collisions.
- **Associated Functions (`User.create()`)**: Implemented associated functions attached directly to types. Functions prefixed with a type name (`fn User.create()`) belong to the namespace of that type, providing clean, explicit scoping for constructors and behavior without requiring implicit `self` or traditional OOP paradigms.
- **Parser & Resolver Alignments**: Updated the grammar to represent functions, types, and use declarations with `Path` nodes rather than single `Ident` nodes. A firewall in the typechecker prevents namespaces from accidentally leaking as values into the compilation pipeline.

## v0.0.11 — OpenAPI Contracts & External References

- **OpenAPI 3.0 Integration (`dsl api`)**: Extended the external contracts feature with a robust `api` DSL to parse OpenAPI 3.0 specs. Syntactical rules added for `with path`, `with query`, `with header`, `with body`, and `with auth`.
- **JSON Schema offline validation**: Integrated full JSON parsing and validation from OpenAPI specifications. Included resolution of `$ref` pointers mapping nested objects to their appropriate components.
- **External `$ref` Fetching (`kai sync api`)**: Integrated `DocStore` to handle external HTTP schema fetching and relative filesystem resolution when synchronizing the OpenAPI contracts offline.

"""

text = v12_entry + text

with open('docs/kai-changelog.md', 'w') as f:
    f.write(text)
