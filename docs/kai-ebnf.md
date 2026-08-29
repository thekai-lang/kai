# Kai — Core Language Grammar (EBNF)

**Scope:** v0.0.1–v0.0.6 core language (unchanged, §1–§8 below), plus §9
covering the v0.0.7 trust-aware layer's syntax (`@local`/`@wallclock`,
locked in whitepaper v0.15 and refined through v0.17's §5.1.7) and §9a
covering the v0.0.8 `require`/`observe` pair — **syntax AND semantics both
locked** (whitepaper v0.20–v0.22, §5.2). The rest of the trust-aware layer (`@override`) remains **excluded**. `dsl sql` (v0.0.10) and `dsl api` (v0.0.11) grammar are locked and provided in §11.

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
               (* A bare single '_' is carved out of this rule as of
                  v0.0.6 — it lexes as a distinct Underscore token, not an
                  Ident, reserved exclusively for DiscardStmt below. Every
                  other underscore-containing name (`_foo`, `my_var`) is
                  an ordinary Ident, unaffected. *)
Letter       ::= 'a'..'z' | 'A'..'Z'
Digit        ::= '0'..'9'

Keyword      ::= 'fn' | 'let' | 'var' | 'type' | 'use' | 'return'
               | 'if' | 'else' | 'for' | 'in' | 'while'
               | 'true' | 'false'
               | 'public' | 'mut'
               | 'Some' | 'None' | 'Ok' | 'Err' | 'Result' | 'Optional'
               | 'catch' | 'as'                    (* 'as' reserved for future module alias, not v0.0.1–5 core *)
                (* v0.0.7 trust-aware layer (§9): temporal modifiers, effects
                   contracts, and the §9a require/observe pair. `local`/
                   `wallclock` lex as keywords but are only meaningful
                   immediately after '@'. *)
                | 'require' | 'observe'             (* v0.0.8 semantics — locked whitepaper v0.20 §5.2 *)
                | 'effects'                         (* EffectsAnnotation, §9 *)
                | 'escapes-local-context'           (* hyphenated: lexer consumes `-local-context`
                                                       as one continuation of `escapes`, a single
                                                       keyword token despite '-' not being an Ident char *)
                | 'local' | 'wallclock'             (* only after '@', see TemporalModifier §9 *)

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
StringSegment ::= RawTextChar | EscapeSeq | Interpolation   (* Interpolation branch: not active in v0.0.5 *)
Interpolation ::= '${' Expr '}'                  (* deferred — see note above *)

EscapeSeq    ::= '\' ('n' | 't' | 'r' | '\' | '"' | '0')
               (* v0.0.5's full escape set — no others. An unrecognized
                  escape (`\q`, etc.) is a lex-phase diagnostic naming the
                  bad sequence, matching the existing precision-first lexer
                  diagnostics (malformed numeric literals get a specific
                  message, not "unexpected character"). `\$` is deliberately
                  absent: `${` isn't special in v0.0.5, so there's nothing
                  for it to escape yet — this can be revisited once
                  interpolation itself lands. *)

(* Operators and punctuation *)
Op           ::= '+' | '-' | '*' | '/' | '%'
               | '==' | '!=' | '<' | '>' | '<=' | '>='
               | '&&' | '||' | '!'
               | '=' | '+=' | '-=' | '*=' | '/='
               | '@'                               (* v0.0.7: temporal modifier prefix, §9 *)
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
               (* Ident here is a real Ident, never the Underscore token —
                  '_' as a "throwaway parameter" is explicitly not
                  supported, consistent with '_' being reserved solely
                  for DiscardStmt (§9.9b) and nowhere else. *)
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
               | DiscardStmt
               | RequireStmt      (* v0.0.8, §9a — syntax only, semantics pending formalization *)
               | ObserveStmt      (* v0.0.8, §9a — syntax only, semantics pending formalization *)
               | ExprStmt

LetStmt      ::= 'let' Ident [ ':' Type ] '=' Expr ';'
VarStmt      ::= 'var' Ident [ ':' Type ] '=' Expr ';'
               (* Both require an initializer — §9.2. No declaration-
                  without-initializer form exists yet. Neither accepts
                  '_' in the Ident position — '_' is not an Ident at all
                  as of v0.0.6 (see the Ident rule note above), so
                  `let _ = expr;` is a parse-level rejection, not a
                  semantic one. *)

ReturnStmt   ::= 'return' [ Expr ] ';'

IfStmt       ::= 'if' Expr Block [ 'else' (IfStmt | Block) ]

ForStmt      ::= 'for' Ident 'in' Expr Block
               (* Iterates and borrows each element per iteration — §9.9.
                  Ident here is a real Ident too, not Underscore — no
                  throwaway loop-variable form, same reasoning as Param
                  above. *)

WhileStmt    ::= 'while' Expr Block
               (* Confirmed present via v0.4.5 reference code — condition
                  loop, standard `while cond { ... }` form, nesting allowed. *)

DiscardStmt  ::= '_' '=' Expr ';'
               (* v0.0.6. The sole explicit-discard form (whitepaper §9.9b)
                  — applies to any expression type, not just Optional/Result,
                  though it's specifically what makes discarding those two
                  legal despite the diagnostic §9.9a otherwise requires.
                  '_' here is the Underscore token, not Ident — this is a
                  dedicated statement shape, not a variable binding; '_' is
                  never introduced as a name anywhere else in the grammar. *)

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
               | 'catch' '|' Ident '|' CatchBlock  (* Result error-branch unwrap — §3.4/§9.9a. NOT
                    ordinary Block: CatchBlock below requires a trailing
                    expression, unlike every other block in the language. *)

CatchBlock   ::= '{' { Stmt } Expr '}'
               (* v0.0.6, narrow and deliberate: this is the ONLY place a
                  block produces a value as a trailing expression (no ';').
                  Ordinary Block ('{' { Stmt } '}') is completely untouched
                  by this — Kai does not have block-expressions generally,
                  only this one special case, because `catch`'s whole
                  purpose is "recover a value of the Ok type." Reusing plain
                  Block would need every block in the language (function
                  bodies, if/while bodies) to gain trailing-expression
                  semantics — deliberately rejected in favor of one narrow
                  rule instead of a language-wide change. The `|err|` binding
                  introduces `err: E` (the Result's error type) into scope
                  for CatchBlock's statements and trailing expression. *)

(* `.unwrap_or(...)` has NO dedicated grammar production (removed from an
   earlier draft, deliberately). `parsed.unwrap_or(0)` parses through the
   ordinary '.' Ident + '(' ArgList ')' composition already covering
   math.sqrt(...) above — the parser doesn't know `unwrap_or` is special.
   Resolution as a builtin operation happens at typecheck, keyed on
   (receiver type, field name) — receiver must be Optional<T>/Result<T,E>
   and the field name must be `unwrap_or` — exactly the same
   parser-meaning-agnostic discipline as the qualified-call note above. *)

PrimaryExpr  ::= IntLit
               | FloatLit
               | BoolLit
               | StringLit
               | Ident
               | 'Some' '(' Expr ')'
               | 'None'
               (* `None`'s payload type is never written at the use site —
                  it comes entirely from context (the enclosing `let`/`var`
                  annotation), same mechanism as the empty-array-literal
                  rule (§3.4, v0.0.5): `let x: string? = None;` is valid,
                  `let x = None;` is a typecheck error requiring an
                  annotation, not an inference attempt. *)
               | 'Ok' '(' Expr ')'
               | 'Err' '(' Expr ')'
               (* `Result<T,E>` constructors, parallel to Some/None —
                  §3.4. Each pins down only ONE of Result's two type
                  parameters from its argument; the other needs context
                  (annotation, or the enclosing fn's declared return type
                  in a `return` statement) — same context-typing mechanism
                  as `None` immediately above, not a separate rule. *)
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
               (* An empty ArrayLit (`[]`) parses fine — this rule doesn't
                  reject it. The requirement for an explicit type annotation
                  on the enclosing `let`/`var` when the literal is empty
                  (whitepaper §3.4) is a typecheck-phase rule, not a parser
                  one, same pattern as everywhere else element type/context
                  decisions have been deferred past parsing in this grammar. *)

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
| `OptionalType`, `ResultType`, `CoalesceExpr`, `unwrap_or` (both `Optional`/`Result`), `catch` (`Result`-only), `ClosureType`, `ClosureLit`, `DiscardStmt`, closure-cycle rejection | v0.0.6 |
| `DurationLit`, `@local`/`@wallclock` type modifiers, `EffectsAnnotation` | v0.0.7 |
| `RequireStmt`, `ObserveStmt` — syntax and semantics both locked (§9a, whitepaper §5.2) | v0.0.8 |

---

## 9. Trust-aware layer grammar — v0.0.7 (`@local`/`@wallclock` only)

Only §5.1 (temporal types), locked in whitepaper v0.15, gets grammar here alongside §9a below. `require`/`observe` (§5.2) is **v0.0.8** scope per roadmap §7 — syntax stable since v0.2, semantics formalized in whitepaper v0.20–v0.22 (§5.2.1–§5.2.2); kept separate so a reader doesn't infer v0.0.7 implements it. `reversible`/`compensate` (§5.3) is **v0.0.9** scope, locked in v0.25. `dsl sql`/`dsl api` (§5.4) are locked in v0.0.10/v0.0.11 and covered in §11.

```ebnf
DurationLit  ::= DecimalInt DurationUnit
DurationUnit ::= 'ms' | 's' | 'm' | 'h' | 'd'
               (* Integer-only for v0.0.7 — 30m, 1h, 500ms valid; 1.5h,
                  1hour, -30m, bare 30 with no unit are not. Lexical
                  validity only: 0ms parses fine, whether it's a legal
                  duration for a given temporal type is a typecheck
                  question, not a lexer one — whitepaper §5.1.6. *)

TemporalModifier ::= '@local' '(' DurationLit ')'
                   | '@wallclock' '(' DurationLit ')'
               (* Postfix type modifier, same position/style as `T?` and
                  `T[]` — attaches to a Type, e.g. `Token @local(30m)`. *)

Type         ::= ... | Type TemporalModifier
               (* Extends the core-language Type rule (§4 above) — a
                  temporal modifier can apply to any type, matching the
                  postfix-modifier pattern already established for
                  Optional (`?`) and Array (`[]`). *)

EffectName   ::= 'escapes-local-context'
               (* Only effect defined at v0.0.7. §5's roadmap (io,
                  blocking, etc.) will extend this set later — EffectName
                  is deliberately an open alternation, not closed, so
                  adding a new effect kind never requires touching this
                  rule's shape, only adding an alternative. *)

EffectSet    ::= EffectName { ',' EffectName }

EffectsAnnotation ::= 'effects' '{' [ EffectSet ] '}'
               (* Optional, appears after the return type and before the
                  function body — whitepaper §5.1.2. `effects {}` is a
                  legal (empty) declaration, distinct from omitting the
                  annotation entirely: omitted = effects are purely
                  inferred, present = a declared contract the inferred
                  set must be a subset of (checked, never trusted). *)

FnDecl       ::= [ 'public' ] 'fn' Ident '(' [ ParamList ] ')' '->' Type
                 [ EffectsAnnotation ] Block
               (* Extends the core FnDecl (§5 above) with the optional
                  effects contract. Placement after the return type and
                  before the body mirrors `reversible`'s position in the
                  worked examples elsewhere in the whitepaper (an effect/
                  capability annotation goes between signature and body). *)
```

**Not grammar, recorded here only as a pointer:** the boundary rule itself (`escapes-local-context` inference, the closure-capture reachability invariant, the SCC/fixpoint call-graph analysis, and §5.1.3a's local-read narrowing — a `T @local(d)` value flowing to a plain-`T` argument position with propagation gated by the callee's effect set) is entirely a type/effect-checker concern, per whitepaper §5.1.1's own layering principle — none of it belongs in this file. This section defines syntax only; consult whitepaper §5.1.1–§5.1.3a (and §5.1.7 for the runtime representation) for what the syntax *means*.

## 9a. `require`/`observe` grammar — syntax & semantics locked (whitepaper §5.2, v0.20)

```ebnf
RequireStmt  ::= 'require' Expr ';'
               (* Correctness Trust — whitepaper §5.2. Violation always
                  panics; no confidence score, no soft form. *)

ObserveStmt  ::= 'observe' Expr ';'
               (* Signal, not Trust (§5.0, §5.2) — never panics, tracked
                  as informational history only, not debt. *)
```

**Status:** ~~syntax only, semantics pending~~ **Resolved.** This shape has been stable since whitepaper v0.2, and §5.2's semantics are now formalized to §5.1's rigor (whitepaper v0.20–v0.22): `require`'s `Trust<C>` lowering table and evaluation timing (v0.20), the pre-ledger `.kai/debt.log` record and sink-location rule (v0.21), and the raw source-text-span condition text (v0.22). Both lower into `Trust<C>` through `kai-effects` (local, no graph traversal — §5.2's scoping decision explicitly excludes only the call-graph *inference* subsystem from §5.1, not `Trust<C>` lowering itself). Implementation may proceed on the same footing as §5.1.


## Open items — grammar-level decisions not yet made anywhere in the whitepaper

Items **struck through** below are now resolved by the actual implementation
and reflected in the rules above. They're kept
visible so the resolution history isn't lost — same spirit as the whitepaper's
own changelog.

1. ~~No loop other than `for...in` has ever appeared.~~ **Resolved: `while` exists** (v0.0.2) and **`for...in` landed at v0.0.5** with arrays/ownership — both rules above are implemented, not spec-only.
2. ~~Boolean logic operators (`&&`, `||`, `!`) have never appeared.~~ **Resolved: all three exist**, confirmed v0.0.2, with `&&` binding tighter than `||` and short-circuit evaluation verified end-to-end.
3. ~~Arrays and `for` loops now have a scheduled version (v0.0.5, Ownership runtime).~~ **Resolved: v0.0.5 landed** — `ForStmt`/`ArrayType`/`ArrayLit`/indexing are implemented and tested (golden fixture `v0005`).
4. ~~`ClosureLit` still uses `fn(...)` while `ClosureType` dropped it.~~ **Resolved: intentional, confirmed at v0.0.6.** `fn` marks "this is an expression" and deliberately does not appear in the type position — this also leaves room for a future function-pointer-vs-closure type distinction. Not symmetry for its own sake; the asymmetry is load-bearing.
5. ~~Assignable place — partially resolved.~~ **Now fully resolved.** Assignment is statement-only (confirmed by implementation), and `Place` includes both field access (`Place '.' Ident`, v0.0.3) and array indexing (`Place '[' Expr ']'`, v0.0.5) under one uniform rule — see the `Place` rule above and whitepaper §9.3's generalized model (root determines writability; every projection inherits it).
6. **Module-qualified calls (`math.sqrt(9.0)`) and struct field access (`user.name`) share the same `.` postfix rule — resolved architecturally, not yet exercised by real code.** No separate AST node for "qualified call" — `math.sqrt(9.0)` parses as ordinary `Call(FieldAccess(Ident, Ident), args)` via the existing `PostfixExpr` composition (see `PostfixOp` note above); disambiguating whether the base identifier is a module or a value is a resolver-phase job. `StructLit`'s head was generalized to `QualifiedName` (dotted, §7 above) for the same reason, so `math.Point { ... }` parses without a parallel node either. This decision is recorded so a "QualifiedCall"/"QualifiedStructLit" special-case node doesn't get reintroduced later out of convenience — it would duplicate what composition already provides and push a semantic (module-vs-value) distinction into the parser, which contradicts the parser/resolver boundary this project has held everywhere else (§8's TAST discipline, the `Trust<C>` IR in §5.0/§8). Still needs real-code testing once modules (v0.0.4) and structs (v0.0.3) have both landed.
   **Downstream consequence, worth being precise about:** the meaning-agnostic parse means `p.add(2, 3)` (a struct field called like a function) is syntactically identical to `math.add(2, 3)` — the parser genuinely cannot and should not tell them apart. But the two cases resolve through **different diagnostic paths, not one shared "direct-call" diagnostic**: if the base resolves to a module (via `use`), an unknown/private member is a **resolver-phase** diagnostic (commit 4's "unknown module member"/"private access"). If the base resolves to an ordinary value, there is no module-lookup step at all — it falls to **typecheck**, with two sub-cases: the named field doesn't exist on the value's type (the same "unknown field" diagnostic an ordinary read like `p.add` would already produce, call or not), or the field exists but its type isn't callable ("value of type X is not callable" — a genuinely new diagnostic case, only reachable once closures exist at v0.0.6, since before that **no struct field is ever callable, unconditionally** — every `p.<field>(...)` where `p` isn't a module resolves as invalid at v0.0.3–v0.0.5, just via whichever of the two typecheck sub-cases the field name happens to hit).
7. ~~`Optional<T>` vs. postfix `T?`.~~ **Resolved at v0.0.6.** `T?` is canonical source-level sugar for `Optional<T>` — desugars before typecheck, never a second semantic form (whitepaper §9.9a). `Result<T,E>` intentionally has no postfix sugar (binary type parameter, no natural unary shorthand).
8. ~~`println` called unqualified in v0.4.5 reference code contradicts §3.6's "always namespace-qualified" rule.~~ **Resolved: `io.println(...)` only, no exception.** The v0.4.5 sample predates the current whitepaper's strict qualification rule and is not carried forward — every stdlib call, including `println`, goes through its namespace with no globally-injected builtins. Now recorded in whitepaper §3.6.
9. ~~Language-level semantics decided in the compiler but not yet written into the whitepaper.~~ **Resolved.** Block scoping/shadowing, definite-return analysis, and integer literal widening are now in whitepaper §3.2a.
10. ~~`mut` parameter semantics for stack types — ABI implications unclear.~~ **Resolved.** `mut` on a stack-type parameter is local-copy-permission only, zero ABI difference from an unannotated parameter, not observable by the caller. One rule ("`mut` grants write access through the binding"), two consequences depending on stack vs. heap — see whitepaper §9.3.
11. ~~Retain rule (§9.5) enforcement version was misattributed to v0.0.3.~~ **Resolved.** v0.0.3 has zero heap-bearing types active (structs are stack-only per §9.1) — nothing there ever triggers a retain. The claim now correctly sits on v0.0.5 (Ownership runtime), where `string`/arrays first exist.
12. ~~Cyclic struct definitions — undefined behavior.~~ **Resolved: compile error**, detected via DFS over the `TypeDecl` dependency graph, diagnostic reports the cycle path. Indirection/boxing to legitimately express self-referential types remains undesigned — tracked as its own open item in the whitepaper's Appendix A, not here (it's a semantic/type-system question, not a grammar one).
13. ~~Discarding a non-`unit` call result.~~ **Resolved.** Allowed silently for v0.0.3–v0.0.5 (no correctness risk for scalars/structs). From v0.0.6, both `Result` **and** `Optional` require a diagnostic when discarded silently — symmetric, not `Result`-only (whitepaper §9.9a, implemented v0.0.6.2, 295 passing tests). `_ = expr;` (§9.9b) is the sole escape hatch. This remains a typecheck-phase rule, not a grammar one — `CallExprStmt ::= Expr ';'` accepts the shape regardless; the checker decides.
14. **NEW — type/function namespace separation.** `type Point = {...}` and a hypothetical `fn Point(...)` don't collide: struct-literal syntax (`Point { ... }`) and call syntax (`Point(...)`) are already unambiguous to the parser via lookahead on the token following the identifier, so type names and function names can be treated as separate namespaces at the resolver level without any grammar ambiguity. Recorded here as the working assumption; not yet exercised by real code.
## 10. Reversible grammar — v0.0.9 (`reversible`/`compensate`)

```ebnf
FnDecl ::= "fn" Ident "(" ParamList ")" [ "->" Type ] [ "reversible" ] Block
Expr   ::= CallExpr [ "compensate" Block ]
```

## 11. External Contracts grammar — v0.0.10 & v0.0.11 (`dsl sql`/`dsl api`)

```ebnf
DslExpr ::= 'dsl' DslKind ( 'raw' )? '(' DslArgs ')' ( '->' Type )? '{' DslBody '}'

DslKind ::= 'sql' | 'api'

DslArgs ::=
    | 'v' DecimalInt                 (* sql: v12 *)
    | StringLit ',' 'v' DecimalInt   (* api: "stripe", v3 *)

DslBody ::=
    | StringLit                      (* when 'raw' *)
    | SqlQuery                       (* when kind='sql' and not 'raw' *)
    | ApiContract                    (* when kind='api' and not 'raw' *)

ApiContract ::= ApiMethod ApiPath ApiWithClause*

ApiMethod ::= 'GET' | 'POST' | 'PUT' | 'DELETE' | 'PATCH'
ApiPath ::= '/' ( Ident | '/' )*

ApiWithClause ::=
    | 'with' 'path' ':' StructLit
    | 'with' 'query' ':' StructLit
    | 'with' 'header' ':' StructLit
    | 'with' 'body' ':' StructLit
    | 'with' 'auth' ':' Expr
```

## 12. Module Behavior & Associated Types — v0.0.12

```ebnf
FnDecl ::= "public"? "fn" Path "(" ParamList ")" "->" Type [ "reversible" ] Block
Type ::= Path { '[' ']' } [ '?' ]
UseDecl ::= "use" Path [ "as" Ident ] ";"
```
*Note: `Path` replacing `Ident` in `FnDecl` allows for associated functions (`fn User.create()`). In `Type`, it allows for module-qualified types (`auth.User`). In `UseDecl`, `Path` can now resolve to a specific symbol for direct import.*
