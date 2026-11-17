# Grammaire formelle MCF (implémentée)

Notation EBNF : `{ x }` = zéro ou plus, `[ x ]` = optionnel, `|` = alternative.
Base : cahier des charges 1.2, §21. Les écarts et compléments sont justifiés dans [DECISIONS.md](../DECISIONS.md).

```text
Module          = { Item } ;
Item            = GlobalDecl | FuncDecl | ScopeDecl | ImportDecl | EventDecl | OnHandler
                | StructDecl | EnumDecl ;

GlobalDecl      = Ident ":" TypeDecl [ "=" Expr ] ";" ;
LocalDecl       = LocalIdent ":" TypeDecl [ "=" Expr ] ";" ;

TypeDecl        = TypeRef [ "," TypeRef ] ;                 (* DualType : 2 max, sinon MCF0106 *)
TypeRef         = Path [ "<" TypeRef { "," TypeRef } ">" ] ;
Path            = Ident { "." Ident } ;

FuncDecl        = "func" Ident "(" [ Params ] ")" ":" TypeDecl Block ;
Params          = Param { "," Param } ;
Param           = LocalIdent ":" TypeDecl ;

ScopeDecl       = "scope" Ident "{" { Item } "}" ;
ImportDecl      = "import" Path [ "as" Ident ] ";" ;
StructDecl      = "struct" Ident "{" { Ident ":" TypeDecl ";" | FuncDecl } "}" ;
EnumDecl        = "enum" Ident "{" [ Ident { "," Ident } [ "," ] ] "}" ;
EventDecl       = "event" Path "(" [ Params ] ")" ";" ;
OnHandler       = "on" Path "(" [ LocalIdent { "," LocalIdent } ] ")" Block ;

Block           = "{" { Stmt } "}" ;
Stmt            = LocalDecl
                | IfStmt
                | "while" "(" Expr ")" "do" Block
                | "for" "(" LocalDecl Expr ";" SimpleStmt ")" "do" Block
                | "foreach" "(" LocalIdent [ ":" TypeDecl ] "in" Expr ")" "do" Block
                | "return" [ Expr ] ";"
                | "break" ";"
                | "continue" ";"
                | [ "start" ] "emit" Path "(" [ Args ] ")" ";"
                | SimpleStmt ";" ;
SimpleStmt      = Place "=" Expr | Expr ;
Place           = Name | Postfix "." Ident | Postfix "[" Expr "]" ;
IfStmt          = "if" "(" Expr ")" "then" Block [ "else" ( IfStmt | Block ) ] ;

Expr            = Or ;
Or              = And { "||" And } ;
And             = Equality { "&&" Equality } ;
Equality        = Relational [ ( "==" | "!=" ) Relational ] ;          (* non associatif *)
Relational      = Additive [ ( "<" | ">" | "<=" | ">=" ) Additive
                           | "is" TypeRef ] ;                          (* non associatif *)
Additive        = Multiplicative { ( "+" | "-" ) Multiplicative } ;
Multiplicative  = Unary { ( "*" | "/" | "%" ) Unary } ;
Unary           = ( "!" | "-" | "await" ) Unary
                | "start" Postfix                                      (* un appel : MCF0121 *)
                | Postfix ;
Postfix         = Primary { "(" [ Args ] ")" | "." Ident | "[" Expr "]" | StructLit } ;
StructLit       = "{" [ Ident ":" Expr { "," Ident ":" Expr } [ "," ] ] "}" ;
                  (* seulement après un chemin global `A` / `A.B`, et si `{` est suivi
                     de `}` ou de `Ident :` — sinon c'est un bloc *)
Args            = Expr { "," Expr } ;
Primary         = Int | Float | String | "true" | "false"
                | Ident | LocalIdent
                | "(" Expr ")"
                | "[" [ Args ] "]" ;
```

## Priorités

| Niveau (faible → fort) | Opérateurs | Associativité |
|---|---|---|
| 1 | `\|\|` | gauche |
| 2 | `&&` | gauche |
| 3 | `==` `!=` | **non associatif** (`a == b == c` → MCF0113) |
| 4 | `<` `>` `<=` `>=` `is` | **non associatif** (`0 < x < 10` → MCF0113) |
| 5 | `+` `-` | gauche |
| 6 | `*` `/` `%` | gauche |
| 7 | `!` `-` `await` `start` unaires | droite |
| 8 | appel, `.membre`, `[index]` | gauche |

## Règles contextuelles vérifiées par le parser

| Règle | Diagnostic |
|---|---|
| Déclaration `_x:` au niveau module ou scope | MCF0103 |
| Déclaration `x:` dans un bloc | MCF0104 |
| Paramètre ou donnée d'événement sans `_` | MCF0105 |
| Type de retour absent | MCF0111 |
| `;` après un bloc ou instruction vide | MCF0112 (avertissement, erreur en `--strict`) |
| Instruction exécutable au niveau module | MCF0115 |
| `func`, `scope`, `struct`, `enum`, `event`, `import`, `on` dans un bloc | MCF0116 |
| Champ de structure préfixé `_` | MCF0120 |
| `start` suivi d'autre chose qu'un appel (`start 3`, `start f`) | MCF0121 |
| `then` / `do` manquant | MCF0117 / MCF0118 |
