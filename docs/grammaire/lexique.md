# Lexique MCF

Implémenté par `compiler/syntax/src/lexer.rs`. Toute modification doit être reportée ici et accompagnée de tests.

## Encodage et blancs

- Fichiers en UTF-8 (un BOM initial est ignoré par la CLI).
- Espaces, tabulations et retours à la ligne séparent les tokens et n'ont pas d'autre sens.

## Commentaires

| Forme | Règle |
|---|---|
| `// ...` | Jusqu'à la fin de la ligne |
| `/* ... */` | Sur plusieurs lignes, **non imbriquables** ; non fermé → `MCF0003` |

## Identifiants

| Sorte | Forme | Usage |
|---|---|---|
| Global | `[A-Za-z][A-Za-z0-9_]*` | Variables de module, fonctions, scopes, types, événements |
| Local | `_[A-Za-z][A-Za-z0-9_]*` | Variables locales, paramètres, variables de boucle, données d'événement |

- ASCII uniquement pour l'instant (moins d'ambiguïtés pour les humains comme pour les IA).
- `_` seul ou `_` suivi d'un chiffre / d'un autre `_` → `MCF0005`.

## Mots-clés

`func scope struct enum import event on emit if then else while for foreach in do return break continue is self true false`

## Mots réservés

Interdits comme identifiants (`MCF0008`) pour pouvoir les introduire plus tard sans casser de code :

`action as async await config const interface let match null secret set switch`

`ui`, `state`, `view` et `style` ne sont **pas** réservés en `.mcf` (identifiants trop courants) : ce seront des mots-clés contextuels des fichiers `.uimcf`.

`set` et `action` (édition 1.0) reçoivent une suggestion de migration ; `set` est ignoré après le diagnostic pour que la déclaration qui suit soit analysée normalement.

## Littéraux

| Littéral | Forme | Notes |
|---|---|---|
| Entier | `[0-9]+` | Taille de `Int` à fixer (voir DECISIONS) |
| Décimal | `[0-9]+.[0-9]+` | `1.` n'est pas un décimal : `1.x` = accès membre |
| Chaîne | `"..."` | Sur une seule ligne (`MCF0007`) |
| Booléen | `true`, `false` | |

Un nombre collé à une lettre (`12ab`) → `MCF0006`.

Échappements de chaîne : `\n \t \r \0 \\ \"` et `\u{X..XXXXXX}` (1 à 6 chiffres hexadécimaux, point de code valide). Tout autre → `MCF0004`.

Pas encore de littéral `Char` ni d'exposant (`1e3`) : à arbitrer.

## Ponctuation et opérateurs

`{ } ( ) [ ] ; : , .`
`= == != < > <= >= + - * / % && || !`

`&` ou `|` seul → `MCF0001` avec suggestion `&&` / `||` (pas d'opérateurs bit à bit pour l'instant).
