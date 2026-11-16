# Vérification des types (phase 4)

Implémentée par `compiler/semantic` (`types.rs` + `lib.rs`). Base : cahier des charges 1.2, §5 et §6.

## Principe : aucune conversion implicite

Une valeur ne peut être rangée (déclaration, affectation, argument, `return`, `emit`) que dans un type **identique**, ou dans un DualType qui **contient** son type.

```rust
speed: Float = 180;      // MCF0220 : écrire 180.0
```

## DualType

```rust
playerState: Bool, Int = true;
playerState = 42;          // ok
playerState = "online";    // MCF0220 : troisième type
print(playerState + 10);   // MCF0221 : type actif inconnu
```

- Deux types **distincts** maximum (MCF0106, MCF0203). `Void` interdit (MCF0227).
- Une valeur DualType ne peut ni entrer dans un type simple, ni servir d'opérande, sans *narrowing* (MCF0221).
- `print` accepte une valeur DualType telle quelle.

## Narrowing

Le compilateur connaît le type actif :

| Situation | Type actif connu |
|---|---|
| Dans `if (x is T) then { … }` | `T` |
| Dans le `else` correspondant | l'autre type |
| Dans `a && b`, pendant `b` | ce que `a` garantit |
| Après `x = valeur` ou `_x: A, B = valeur;` | le type de la valeur |

Le narrowing est **annulé** quand il pourrait devenir faux (règle conservatrice : refuser plutôt que se tromper) :

- en entrant dans une boucle dont le corps affecte la variable (deuxième tour) ;
- pour les **globales**, après tout appel de fonction MCF ou `emit` (la fonction ou le gestionnaire peut la modifier) ;
- après la sortie du bloc où une affectation l'a établi.

`x is T` où `T` est impossible → MCF0222 ; toujours vrai → MCF0223 (avertissement).

## Opérateurs

| Opérateur | Opérandes | Résultat |
|---|---|---|
| `&&` `\|\|` `!` | `Bool` | `Bool` |
| `==` `!=` | même type (ou Int/Float) | `Bool` |
| `<` `>` `<=` `>=` | `Int`/`Float` | `Bool` |
| `+ - * / %` | `Int` et `Int` | `Int` |
| `+ - * / %` | `Float` avec `Int` ou `Float` | `Float` (seule promotion admise, D23) |
| `+` | `String` avec `String`, `Int`, `Float`, `Bool` ou `Char` | `String` (concaténation, D22) |
| `-` unaire | `Int` / `Float` | même type |

Autre combinaison → MCF0230.

## Fonctions et retours

- Arguments : nombre (MCF0210) et types (MCF0220) vérifiés ; appeler une non-fonction → MCF0231.
- `return valeur;` dans une fonction `Void` → MCF0225 ; `return;` ailleurs → MCF0224.
- Une fonction non `Void` doit retourner sur **tous** les chemins (MCF0226) : un `if` sans `else` ne suffit pas.

## Collections

- `Array<T>[Int]` → `T` ; `Map<K, V>[K]` → `V`. `Array` / `Map` sans argument : éléments non vérifiés.
- `[a, b, c]` : tous les éléments du même type.
- `foreach (_x in tableau)` : `_x` prend le type des éléments.

## Littéraux

`Int` = entier signé 64 bits (D15) : au-delà → MCF0229.

## Pas encore vérifié

Membres d'une valeur (`x.length`), initialisation avant usage d'une variable déclarée sans valeur, structures et énumérations : phases suivantes.
