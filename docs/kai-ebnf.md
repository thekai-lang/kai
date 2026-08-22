# Kai — Core Language Grammar (EBNF)

**Scope:** v0.0.1–v0.0.5 only (whitepaper §3). The trust-aware layer (`require`,
`observe`, `@local`/`@wallclock`, `reversible`, `compensate`, `dsl sql`, `dsl api`,
`@override`) is v0.0.6+ and deliberately excluded — it needs its own grammar
extension once §5's syntax is locked further, and mixing it in now would let
grammar work run ahead of the section that's still being revised.

**Method:** every rule below is derived from an example that already exists in
the whitepaper. Nothing here is invented beyond that. Constructs implied by
"a real language would need this" but never actually shown (`while`, `match`,
ranges, `break`/`continue`, operators like `&&`/`||`) are listed as **open
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
               | 'true' | 'false'
               | 'public' | 'mut'
               | 'Some' | 'Result' | 'Optional'   (* Optional not yet used as keyword in examples — see open items *)
               | 'catch' | 'as'                    (* 'as' reserved for future module alias, not v0.0.1–5 core *)

(* Literals *)
IntLit       ::= Digit { Digit }
FloatLit     ::= Digit { Digit } '.' Digit { Digit }
BoolLit      ::= 'true' | 'false'

(* Strings support interpolation: ${ expr } inside the literal.
   Lexer must tokenize the literal as a sequence of raw-text and
   embedded-expression segments, not a single opaque string token. *)
StringLit    ::= '"' { StringSegment } '"'
StringSegment ::= RawTextChar | Interpolation
Interpolation ::= '${' Expr '}'

(* Operators and punctuation *)
Op           ::= '+' | '-' | '*' | '/' | '%'
                | '==' | '!=' | '<' | '>' | '<=' | '>='
                | '&&' | '||' | '!'
                | '=' | '+=' | '-=' | '*=' | '/='
                | '??' | '->' | '.' | ',' | ':' | ';'
                | '(' | ')' | '{' | '}' | '[' | ']'
                | '?'                                  (* postfix optional-type marker *)

(* Lexing decisions locked in v0.0.2:
   - Maximal munch for multi-char operators: '==' before '=', '+=' before
     '+', '-=' and '->' distinguished by the second char, etc.
   - A lone '&' or '|' is a lexical error (only '&&' / '||' exist).
   - FloatLit requires at least one digit after '.'; "1." is not a float. *)
```

**Decisions recorded from v0.0.2 implementation discussion:**

1. **`&&`/`||` exist**, with `&&` binding tighter than `||` (confirmed via the
   `bool1 && bool3 || bool2` reading). Both **short-circuit**.
2. **Unary minus is general**: `UnaryExpr ::= ('-' | '!') UnaryExpr | PostfixExpr`
   — it covers `-42` and applies to any operand the type checker accepts
   (ints, floats; `!` on bools). The old `[ '-' ] PostfixExpr` rule was too
   narrow to state this.
3. **Assignment is statement-only.** `AssignExpr` no longer sits inside `Expr`;
   see §6. This forbids `a = b = c` chains by construction instead of by rule.
4. **String is deferred past v0.0.5's current grammar work until ownership
   runtime (§9) exists** — the lexer still has no StringLit support.

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
TypeDecl     ::= 'type' Ident '=' TypeBody
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
                | AssignStmt
                | Block
                | ExprStmt
                | ForStmt

Block        ::= '{' { Stmt } '}'

LetStmt      ::= 'let' Ident [ ':' Type ] '=' Expr ';'
VarStmt      ::= 'var' Ident [ ':' Type ] '=' Expr ';'
                (* Both require an initializer — §9.2. No declaration-
                   without-initializer form exists. *)

ReturnStmt   ::= 'return' [ Expr ] ';'

IfStmt       ::= 'if' Expr Block [ 'else' (IfStmt | Block) ]

AssignStmt   ::= PostfixExpr AssignOp Expr ';'
AssignOp     ::= '=' | '+=' | '-=' | '*=' | '/='
                (* Left side must be an assignable place (identifier in
                   v0.0.2; field/index from v0.0.3). Parser rejects any other
                   expression shape as "invalid assignment target". *)

ForStmt      ::= 'for' Ident 'in' Expr Block
                (* Iterates and borrows each element per iteration — §9.9.
                   No `while` form has appeared in any example — open item.
                   Not implemented before arrays exist (v0.0.3+). *)

ExprStmt     ::= Expr ';'
                (* bare expressions; calls become meaningful here in v0.0.3 *)
```

---

## 7. Expressions

Ordered highest-to-lowest precedence (locked for v0.0.2; `&&` > `||` per the
`bool1 && bool3 || bool2` reading; `??` sits between `&&` and equality —
tighter than the logic operators, looser than comparison):

```ebnf
Expr         ::= LogicOrExpr

LogicOrExpr  ::= LogicAndExpr { '||' LogicAndExpr }
LogicAndExpr ::= CoalesceExpr { '&&' CoalesceExpr }
CoalesceExpr ::= EqualityExpr [ '??' CoalesceExpr ]
                (* Optional-only, per §3.4 change from v0.4.5.
                   Right-associative: a ?? b ?? c reads as a ?? (b ?? c). *)

EqualityExpr ::= RelExpr { ('==' | '!=') RelExpr }
RelExpr      ::= AddExpr { ('<' | '>' | '<=' | '>=') AddExpr }
AddExpr      ::= MulExpr { ('+' | '-') MulExpr }
MulExpr      ::= UnaryExpr { ('*' | '/' | '%') UnaryExpr }
UnaryExpr    ::= ('-' | '!') UnaryExpr | PostfixExpr
PostfixExpr  ::= PrimaryExpr { PostfixOp }
PostfixOp    ::= '.' Ident                   (* field access / module-qualified call, e.g. math.sqrt *)
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

StructLit    ::= Ident '{' [ FieldInitList ] '}'
FieldInitList ::= FieldInit { ',' FieldInit }
FieldInit    ::= Ident ':' Expr

ArrayLit     ::= '[' [ ArgList ] ']'

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
| `LetStmt`/`VarStmt`, `PrimitiveType` beyond int32, `AddExpr`..`MulExpr`, `IfStmt` | v0.0.2 |
| `TypeDecl`/`StructBody`, `StructLit`, `ArgList`/calls, `Param` with `mut` | v0.0.3 |
| `UseDecl`, `ModulePath`, qualified `PostfixOp` (`.` access as module call) | v0.0.4 |
| `OptionalType`, `ResultType`, `CoalesceExpr`, `unwrap_or`/`catch`, `ClosureType`, `ClosureLit` | v0.0.5 |
| `ForStmt`, `ArrayType`, `ArrayLit`, array indexing | needed by v0.0.5 too — §9.9 examples assume arrays/loops exist; roadmap doesn't currently name an explicit version for them (see open items) |

---

## Open items — grammar-level decisions not yet made anywhere in the whitepaper

These need a decision (in discussion here, then promoted into the whitepaper — same amendment rule as always) before the parser reaches them. Listed so they don't get invented silently mid-implementation.

1. **No loop other than `for...in` has ever appeared.** Is `while` in scope for v0.0.1–v0.0.5 at all, or deliberately excluded from the core language? If included, needs its own roadmap line and grammar rule.
2. **RESOLVED (v0.0.2):** `&&`, `||`, `!` are in. `&&` binds tighter than `||`; both short-circuit; see §1 decisions and §7 precedence chain.
3. **Arrays and `for` loops aren't explicitly placed on the §7 roadmap** — they're used in §9's ownership examples (which assume they already exist) but §7 never assigns them a version. Needs a version slot, likely folded into v0.0.2 or v0.0.3.
4. **`ClosureLit` still uses `fn(...)` while `ClosureType` dropped it** (§3.5's stated change was type-only). Worth confirming this asymmetry is intentional and not an oversight carried over from the "avoid two `fn` tokens" rationale, which was about the *type* position specifically, not the value position.
5. **Partially resolved (v0.0.2):** assignment is statement-only with an explicit assignable-place rule — identifier only for now; field/index places arrive with structs/arrays (v0.0.3+). The old "anything on the left of `=`" hole (`1 + 1 = x;`) is closed by construction.
6. **Module-qualified calls (`math.sqrt(9.0)`) and struct field access (`user.name`) share the same `.` postfix rule** in this grammar, which is syntactically correct (they look identical to the parser) but means disambiguating "is this a module or a value" is a resolver-phase job, not a parser-phase one — worth confirming that's the intended split before writing the resolver.
7. **`Optional<T>` vs. postfix `T?`** — §3.4 writes `string?` (postfix) but §9's category list writes `Optional<T>` (generic form) when describing heap-bearing rules. Grammar above treats `T?` as the only surface syntax and treats `Optional`/`Result` as internal type-family names, not user-written generic syntax for Optional specifically (Result *is* user-written as `Result<T, E>`). Confirm this asymmetry (`T?` sugar for Optional, but explicit `Result<T,E>` with no sugar) is intentional.
8. **NEW (v0.0.2): integer literal width without annotation.** An unannotated integer literal defaults to `int32`. A literal wider than `int32` only type-checks when the surrounding context demands `int64` (annotation, return type, other operand). Two bare wide literals (`10000000000 * 2`) are rejected until a suffix syntax exists — needs a whitepaper decision eventually.