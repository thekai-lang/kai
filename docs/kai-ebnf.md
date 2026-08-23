# Kai — Core Language Grammar (EBNF)

**Scope:** v0.0.1–v0.0.6 (whitepaper v0.7's roadmap: v0.0.5 is now the
"Ownership runtime" slot — `string`, arrays, `for..in`, retain/release —
inserted between modules (v0.0.4) and Optional/Result/closures (now v0.0.6,
was v0.0.5). The trust-aware layer (`require`, `observe`, `@local`/
`@wallclock`, `reversible`, `compensate`, `dsl sql`, `dsl api`, `@override`)
is now v0.0.7+ and deliberately excluded — it needs its own grammar
extension once §5's syntax is locked further, and mixing it in now would let
grammar work run ahead of the section that's still being revised.

**Method:** every rule below is derived from an example that already exists in
the whitepaper or the actual v0.0.1/v0.0.2 implementation. Nothing here is
invented beyond that. Constructs implied by "a real language would need this"
but never actually shown are listed as **open
items** at the end, not silently added.

Notation: `::=` defines a rule, `|` alternation, `{ }` zero-or-more, `[ ]`
optional, `( )` grouping, `'...'` literal terminal.

---

## 1. Lexical grammar (tokens)

```ebnf
(* Whitespace and comments are consumed by the lexer, not part of the token stream *)
Whitespace   ::= (' ' | '\t' | '\n' | '\r')+
LineComment  ::= '//' { any-char-except-newline } 

(* Identifiers and keywords *)
Ident        ::= (Letter | '_') { Letter | Digit | '_' }
Letter       ::= 'a'..'z' | 'A'..'Z'
Digit        ::= '0'..'9'

Keyword      ::= 'fn' | 'let' | 'var' | 'type' | 'use' | 'return'
                | 'if' | 'else' | 'for' | 'in'
                  (* 'while' below is RESERVED in this grammar but NOT
                     implemented — see WhileStmt and open item 1. *)
                | 'while'
                | 'true' | 'false'
               | 'public' | 'mut'
               | 'Some' | 'Result' | 'Optional'   (* Optional not yet used as keyword in examples — see open items *)
               | 'catch' | 'as'                    (* 'as' reserved for future module alias, not v0.0.1–5 core *)

(* Literals *)
IntLit       ::= Digit { Digit }
FloatLit     ::= Digit { Digit } '.' Digit { Digit }
BoolLit      ::= 'true' | 'false'

(* v0.0.5 scope: plain literals only — StringLit reduces to '"' { RawTextChar } '"'.
   Interpolation is defined below for completeness/forward-reference but is
   explicitly DEFERRED past v0.0.5 (whitepaper §9.7, Appendix A) — it needs
   its own decisions (evaluation order, value-to-string conversion, temporary
   ownership) that haven't been made yet. Do not implement the lexer's
   segment-splitting behavior until those decisions land; until then the
   lexer should treat `${` inside a string as ordinary literal text, not
   as the start of an embedded expression. *)
StringLit    ::= '"' { StringSegment } '"'
StringSegment ::= RawTextChar | Interpolation   (* Interpolation branch: not active in v0.0.5 *)
Interpolation ::= '${' Expr '}'                  (* deferred — see note above *)
RawTextChar  ::= PlainChar | EscapeSequence
PlainChar    ::= (* any character except '"' and '\' *)
EscapeSequence ::= '\' ( 'n' | 't' | 'r' | '"' | '\' )
               (* v0.0.5 escape set, locked during implementation planning:
                  newline, tab, carriage return, quote, backslash — the
                  minimal standard set. Any other character after '\'
                  is a LEX error ("unknown escape sequence"), not a
                  silent pass-through: an unrecognized escape almost
                  always means the source meant something the language
                  doesn't say. `\` + line-continuation, `\0`, unicode
                  escapes etc. do NOT exist yet. *)


(* Operators and punctuation *)
Op           ::= '+' | '-' | '*' | '/' | '%'
               | '==' | '!=' | '<' | '>' | '<=' | '>='
               | '&&' | '||' | '!'
               | '=' | '+=' | '-=' | '*=' | '/='
               | '??' | '->' | '.' | ',' | ':' | ';'
               | '(' | ')' | '{' | '}' | '[' | ']'
               | '?'                                  (* postfix optional-type marker *)
```

---

## 2. Program structure

```ebnf
Program      ::= { UseDecl } { TopLevelDecl }

TopLevelDecl ::= TypeDecl
               | FnDecl

UseDecl      ::= 'use' ModulePath ';'
ModulePath   ::= Ident { '.' Ident }
               (* '.', '..', '/', '\' as path SEGMENTS are rejected — §3.6.
                  ModulePath here is dot-joined identifiers only. *)
```

---

## 3. Type declarations

```ebnf
TypeDecl     ::= [ 'public' ] 'type' Ident '=' TypeBody
               (* `public type` added — without it, structs could never
                  cross a module boundary at all (a caller could receive
                  a value of the type but never name or read its fields).
                  Same visibility rule as `public fn`, applied uniformly. *)
TypeBody     ::= StructBody
               (* struct is the only TypeBody form shown so far — §3.3.
                  Type aliases like `type int = int32;` (§3.2) are a
                  separate, narrower rule: *)
TypeAliasDecl ::= 'type' Ident '=' Type ';'

StructBody   ::= '{' { FieldDecl } '}'
FieldDecl    ::= Ident ':' Type ';'
```

---

## 4. Types

```ebnf
Type         ::= PrimitiveType
               | Ident                      (* named struct/type-alias reference *)
               | ArrayType
               | OptionalType
               | ResultType
               | ClosureType

PrimitiveType ::= 'int32' | 'int64' | 'float64' | 'bool' | 'unit' | 'string'
               | 'int' | 'float'            (* aliases per §3.2, resolved to int32/float64 *)

ArrayType    ::= Type '[' ']'                (* e.g. int32[] *)
OptionalType ::= Type '?'                    (* e.g. string? *)
ResultType   ::= 'Result' '<' Type ',' Type '>'
ClosureType  ::= '(' [ TypeList ] ')' '->' Type
TypeList     ::= Type { ',' Type }
```

---

## 5. Function declarations

```ebnf
FnDecl       ::= [ 'public' ] 'fn' Ident '(' [ ParamList ] ')' '->' Type Block
ParamList    ::= Param { ',' Param }
Param        ::= [ 'mut' ] Ident ':' Type
               (* 'mut' marks a borrowed-mutable parameter — §9.3.
                  Absence = borrowed-immutable, the default. *)

Block        ::= '{' { Stmt } '}'
```

---

## 6. Statements

```ebnf
Stmt         ::= LetStmt
               | VarStmt
               | ReturnStmt
               | IfStmt
               | ForStmt
               | WhileStmt
               | ExprStmt

LetStmt      ::= 'let' Ident [ ':' Type ] '=' Expr ';'
VarStmt      ::= 'var' Ident [ ':' Type ] '=' Expr ';'
               (* Both require an initializer — §9.2. No declaration-
                  without-initializer form exists yet. *)

ReturnStmt   ::= 'return' [ Expr ] ';'

IfStmt       ::= 'if' Expr Block [ 'else' (IfStmt | Block) ]

ForStmt      ::= 'for' Ident 'in' Expr Block
               (* Iterates and borrows each element per iteration — §9.9. *)

WhileStmt    ::= 'while' Expr Block
               (* SPEC-ONLY — not implemented. No parser or AST support
                  exists in any release so far; the rule below is kept for
                  the grammar's target shape only. An earlier revision of
                  this document claimed `while` was "confirmed v0.0.2" —
                  that was false and is corrected here (open item 1). *)

ExprStmt     ::= AssignStmt | CallExprStmt

AssignStmt   ::= Place ('=' | '+=' | '-=' | '*=' | '/=') Expr ';'
               (* Assignment is STATEMENT-ONLY, never an expression —
                  confirmed by implementation (v0.0.2 changelog): rejects
                  `x = (y = 5)` and similar at parse phase. This differs
                  from the earlier draft of this grammar, which nested
                  assignment inside the Expr precedence chain — that draft
                  is superseded by this rule. *)

Place        ::= Ident
               | Place '.' Ident
               | Place '[' Expr ']'
               (* Field-access assignment target added per v0.0.3 scope
                  decision: struct field writes (`p.x = 5;`) are in scope
                  alongside field reads. Array-index alternative added for
                  v0.0.5: `arr[i] = x;` uses the SAME rule as field writes,
                  not a special case — writability is a property of the
                  root binding (§9.3's Place model: `var`/`mut`-param roots
                  are writable, `let`/plain-param roots are not), and every
                  projection (`.field` or `[index]`, arbitrarily chained)
                  inherits it uniformly. Checked at typecheck/effect-check,
                  not parse time — the parser accepts the shape regardless
                  of root mutability. The root of a `Place` is found by
                  stripping projections down to the base `Ident`. *)

CallExprStmt ::= Expr ';'
               (* Bare calls only in practice (`foo();`), but the grammar
                  doesn't restrict Expr here beyond what typecheck will
                  reject as a statement with no effect — this is a
                  typecheck-phase concern, not a parse-phase one. *)
```

---

## 7. Expressions

Ordered highest-to-lowest precedence (typical for a recursive-descent parser;
adjust table as implementation proceeds, but this is the working assumption
until an example contradicts it):

```ebnf
Expr         ::= CoalesceExpr
               (* Assignment is NOT part of this chain — see AssignStmt
                  above. Expr is a pure, side-effect-free-at-the-syntax-
                  level production; assignment only ever appears as a
                  statement. *)

CoalesceExpr ::= LogicalOrExpr [ '??' CoalesceExpr ]
               (* Optional-only, per §3.4 change from v0.4.5.
                  Right-associative: a ?? b ?? c reads as a ?? (b ?? c). *)

LogicalOrExpr  ::= LogicalAndExpr { '||' LogicalAndExpr }
LogicalAndExpr ::= EqualityExpr { '&&' EqualityExpr }
               (* Confirmed present via v0.4.5 reference code:
                  `bool1 && bool3 || bool2` — && binds tighter than ||,
                  the conventional precedence, matching that example's
                  implied grouping (bool1 && bool3) || bool2. *)

EqualityExpr ::= RelExpr { ('==' | '!=') RelExpr }
RelExpr      ::= AddExpr { ('<' | '>' | '<=' | '>=') AddExpr }
AddExpr      ::= MulExpr { ('+' | '-') MulExpr }
MulExpr      ::= UnaryExpr { ('*' | '/' | '%') UnaryExpr }
UnaryExpr    ::= [ '-' | '!' ] PostfixExpr
               (* Both unary minus and logical NOT confirmed present —
                  v0.0.2 changelog: "unary minus and logical NOT, applied
                  to any expression... `!x` via XOR-fold." Negative literals
                  fold at parse time so codegen never sees `-` applied to
                  an unrepresentable positive constant. *)

PostfixExpr  ::= PrimaryExpr { PostfixOp }
PostfixOp    ::= '.' Ident                   (* field access. Also produces the
                    shape for module-qualified calls, e.g. math.sqrt(9.0) —
                    parses as Call(FieldAccess(Ident("math"), "sqrt"), args)
                    via ordinary composition below. No separate "qualified
                    call" node exists or is needed: the parser only ever
                    produces FieldAccess + Call, and the RESOLVER — not the
                    parser — later decides whether a given FieldAccess base
                    identifier names a module (§3.6) or an ordinary struct
                    value, per open item #6. This is deliberate: keeping the
                    parser meaning-agnostic here mirrors §8's TAST boundary
                    discipline one layer up (typed input, mechanical
                    consumer) — module-vs-value is exactly the kind of
                    semantic distinction the parser must not encode. *)
               | '(' [ ArgList ] ')'          (* call *)
               | '[' Expr ']'                 (* array index *)
               | '.' 'unwrap_or' '(' Expr ')' (* Result unwrap — §3.4 *)
               | 'catch' '|' Ident '|' Block  (* Result error-branch unwrap — §3.4 *)

PrimaryExpr  ::= IntLit
               | FloatLit
               | BoolLit
               | StringLit
               | Ident
               | 'Some' '(' Expr ')'
               | StructLit
               | ArrayLit
               | ClosureLit
               | '(' Expr ')'

ArgList      ::= Expr { ',' Expr }

StructLit    ::= QualifiedName '{' [ FieldInitList ] '}'
QualifiedName ::= Ident { '.' Ident }
               (* Generalized from a bare Ident (earlier draft) to support
                  `math.Point { x: 1, y: 2 }` — a module-qualified struct
                  literal, needed once v0.0.4 modules exist. Reuses the same
                  shape as ModulePath (§2) rather than introducing a
                  parallel "QualifiedStructLit" node: it's the same concept
                  (a dotted name) in both places, and — same reasoning as
                  the PostfixOp note above — whether the qualifier names a
                  module is still a resolver-phase question, not something
                  this rule encodes. A bare Ident is just the len-1 case of
                  QualifiedName, so no separate unqualified form is needed
                  either. *)
FieldInitList ::= FieldInit { ',' FieldInit }
FieldInit    ::= Ident ':' Expr

ArrayLit     ::= '[' [ ArgList ] ']'
               (* Empty form `[]` is legal ONLY where a context type fixes
                  the element type (annotation on the binding, parameter or
                  return position, outer-literal element). A bare
                  `let arr = [];` with no annotation is a COMPILE ERROR
                  ("empty array literal requires a type annotation") — there
                  is no placeholder element type to infer from, and inventing
                  one would violate the no-implicit-conversions stance. *)


ClosureLit   ::= 'fn' '(' [ ParamList ] ')' '->' Type Block
               (* closure VALUE uses 'fn' — only the closure TYPE dropped
                  the leading 'fn' per §3.5's v0.4.5 change. This asymmetry
                  is intentional per the whitepaper example but worth a
                  second look — see open items. *)
```

---

## 8. Cross-reference: which grammar rule needs which version (§7 roadmap)

| Rule | First needed at |
|---|---|
| `FnDecl`, `ReturnStmt`, `IntLit`, minimal `Block` | v0.0.1 |
| `LetStmt`/`VarStmt`, `PrimitiveType` beyond int32, `AddExpr`..`MulExpr`, `IfStmt`, `WhileStmt`, `LogicalAndExpr`/`LogicalOrExpr`, unary `!` | v0.0.2 |
| `TypeDecl`/`StructBody`, `StructLit`, `ArgList`/calls, `Param` with `mut`, field-access `Place` (read and write, write gated by `mut`), cyclic-struct rejection | v0.0.3 |
| `UseDecl`, `ModulePath`, qualified `PostfixOp` (`.` access as module call) | v0.0.4 |
| `ArrayType`, `ArrayLit`, array indexing, `ForStmt`, `string` (`StringLit`, plain literals — no interpolation, deferred), string `==`/`!=` as content comparison | v0.0.5 (Ownership runtime — retain/release actually exercised here for the first time) |
| `OptionalType`, `ResultType`, `CoalesceExpr`, `unwrap_or`/`catch`, `ClosureType`, `ClosureLit` | v0.0.6 |

---

## Open items — grammar-level decisions not yet made anywhere in the whitepaper

Items **struck through** below are now resolved by the actual implementation
(v0.0.1/v0.0.2 changelog) and reflected in the rules above. They're kept
visible so the resolution history isn't lost — same spirit as the whitepaper's
own changelog.

1. **Corrected (v0.0.5.1): this item's earlier resolution was wrong in both directions.** There is no `while` — no parser or AST support has ever existed; the "confirmed v0.0.2" claim above was false, and `WhileStmt` remains spec-only. Conversely, the claim that "`for...in` itself is still not implemented" went stale: `ForStmt` **did land** in v0.0.5 (arrays + iteration) and is implemented as written. `break`/`continue` have never appeared anywhere and are not part of any rule here.
2. ~~Boolean logic operators (`&&`, `||`, `!`) have never appeared.~~ **Resolved: all three exist**, confirmed v0.0.2, with `&&` binding tighter than `||` and short-circuit evaluation verified end-to-end.
3. ~~**Arrays and `for` loops now have a scheduled version (v0.0.5, Ownership runtime)**~~ **Resolved: landed in v0.0.5.** `ForStmt`/`ArrayType`/`ArrayLit` are implemented as written above — array literals, indexing reads/writes, and element-borrowing iteration all ship with the ownership runtime.
4. **`ClosureLit` still uses `fn(...)` while `ClosureType` dropped it** (§3.5's stated change was type-only). Not yet implemented (closures are v0.0.6, not yet reached) — still open.
5. ~~Assignable place — partially resolved.~~ **Now fully resolved.** Assignment is statement-only (confirmed by implementation), and `Place` includes both field access (`Place '.' Ident`, v0.0.3) and array indexing (`Place '[' Expr ']'`, v0.0.5) under one uniform rule — see the `Place` rule above and whitepaper §9.3's generalized model (root determines writability; every projection inherits it).
6. **Module-qualified calls (`math.sqrt(9.0)`) and struct field access (`user.name`) share the same `.` postfix rule — resolved architecturally, not yet exercised by real code.** No separate AST node for "qualified call" — `math.sqrt(9.0)` parses as ordinary `Call(FieldAccess(Ident, Ident), args)` via the existing `PostfixExpr` composition (see `PostfixOp` note above); disambiguating whether the base identifier is a module or a value is a resolver-phase job. `StructLit`'s head was generalized to `QualifiedName` (dotted, §7 above) for the same reason, so `math.Point { ... }` parses without a parallel node either. This decision is recorded so a "QualifiedCall"/"QualifiedStructLit" special-case node doesn't get reintroduced later out of convenience — it would duplicate what composition already provides and push a semantic (module-vs-value) distinction into the parser, which contradicts the parser/resolver boundary this project has held everywhere else (§8's TAST discipline, the `Trust<C>` IR in §5.0/§8). Still needs real-code testing once modules (v0.0.4) and structs (v0.0.3) have both landed.
   **Downstream consequence, worth being precise about:** the meaning-agnostic parse means `p.add(2, 3)` (a struct field called like a function) is syntactically identical to `math.add(2, 3)` — the parser genuinely cannot and should not tell them apart. But the two cases resolve through **different diagnostic paths, not one shared "direct-call" diagnostic**: if the base resolves to a module (via `use`), an unknown/private member is a **resolver-phase** diagnostic (commit 4's "unknown module member"/"private access"). If the base resolves to an ordinary value, there is no module-lookup step at all — it falls to **typecheck**, with two sub-cases: the named field doesn't exist on the value's type (the same "unknown field" diagnostic an ordinary read like `p.add` would already produce, call or not), or the field exists but its type isn't callable ("value of type X is not callable" — a genuinely new diagnostic case, only reachable once closures exist at v0.0.6, since before that **no struct field is ever callable, unconditionally** — every `p.<field>(...)` where `p` isn't a module resolves as invalid at v0.0.3–v0.0.5, just via whichever of the two typecheck sub-cases the field name happens to hit).
7. **`Optional<T>` vs. postfix `T?`** — still open; Optional/Result are now v0.0.6 scope (shifted from v0.0.5), not yet implemented.
8. ~~`println` called unqualified in v0.4.5 reference code contradicts §3.6's "always namespace-qualified" rule.~~ **Resolved: `io.println(...)` only, no exception.** The v0.4.5 sample predates the current whitepaper's strict qualification rule and is not carried forward — every stdlib call, including `println`, goes through its namespace with no globally-injected builtins. Now recorded in whitepaper §3.6.
9. ~~Language-level semantics decided in the compiler but not yet written into the whitepaper.~~ **Resolved.** Block scoping/shadowing, definite-return analysis, and integer literal widening are now in whitepaper §3.2a.
10. ~~`mut` parameter semantics for stack types — ABI implications unclear.~~ **Resolved.** `mut` on a stack-type parameter is local-copy-permission only, zero ABI difference from an unannotated parameter, not observable by the caller. One rule ("`mut` grants write access through the binding"), two consequences depending on stack vs. heap — see whitepaper §9.3.
11. ~~Retain rule (§9.5) enforcement version was misattributed to v0.0.3.~~ **Resolved.** v0.0.3 has zero heap-bearing types active (structs are stack-only per §9.1) — nothing there ever triggers a retain. The claim now correctly sits on v0.0.5 (Ownership runtime), where `string`/arrays first exist.
12. ~~Cyclic struct definitions — undefined behavior.~~ **Resolved: compile error**, detected via DFS over the `TypeDecl` dependency graph, diagnostic reports the cycle path. Indirection/boxing to legitimately express self-referential types remains undesigned — tracked as its own open item in the whitepaper's Appendix A, not here (it's a semantic/type-system question, not a grammar one).
13. **NEW — discarding a non-`unit` call result.** Resolved for v0.0.3–v0.0.5: allowed silently, no diagnostic (no correctness risk for scalars/structs). Revisited at v0.0.6 once `Result` exists — discarding a `Result` will require a diagnostic (§2.3); `Optional`'s discard policy is deliberately left open for that same version, not assumed. This is a typecheck-phase decision, not a grammar one — `CallExprStmt ::= Expr ';'` already accepts the shape regardless of what the checker eventually does with it.
14. **NEW — type/function namespace separation.** `type Point = {...}` and a hypothetical `fn Point(...)` don't collide: struct-literal syntax (`Point { ... }`) and call syntax (`Point(...)`) are already unambiguous to the parser via lookahead on the token following the identifier, so type names and function names can be treated as separate namespaces at the resolver level without any grammar ambiguity. Recorded here as the working assumption; not yet exercised by real code.