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
- Pas deux collections de même sorte (`Array<Int>, Array<String>`) : `is` ne pourrait pas les distinguer à l'exécution (MCF0203).
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
- pour les **globales**, après tout appel de fonction MCF ou `emit` (la fonction ou le gestionnaire peut la modifier), et après `sleep` ou `await` (les autres tâches s'exécutent pendant ce temps, D41) ;
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
- Une fonction, un scope ou un type n'est pas une valeur : `print(Player.hi)` → MCF0232 (l'appeler : `Player.hi()`).
- `return valeur;` dans une fonction `Void` → MCF0225 ; `return;` ailleurs → MCF0224.
- Une fonction non `Void` doit retourner sur **tous** les chemins (MCF0226) : un `if` sans `else` ne suffit pas.

## Collections

- `Array<T>[Int]` → `T` ; `Map<K, V>[K]` → `V`. `Array` / `Map` sans argument : éléments non vérifiés.
- `[a, b, c]` : tous les éléments du même type.
- `foreach (_x in tableau)` : `_x` prend le type des éléments.

## Méthodes natives

Elles s'appellent avec `.` sur une valeur (D38) ; un nom inconnu → MCF0241 avec la liste des méthodes possibles, mauvais arguments → MCF0210 / MCF0220. Une méthode s'appelle toujours : `_xs.length` sans parenthèses → MCF0232.

| Receveur | Méthode | Retour |
|---|---|---|
| `Array<T>`, `Map<K, V>`, `String` | `length()` | `Int` (nombre de caractères pour une chaîne) |
| `Array<T>` | `push(_v: T)` | `Void` (ajoute à la fin) |
| `Array<T>` | `pop()` | `T` (retire le dernier ; tableau vide → MCF0409) |
| `Array<T>` | `removeAt(_i: Int)` | `T` (retire l'élément ; hors limites → MCF0403) |
| `Array<T>` | `contains(_v: T)` | `Bool` (comparaison `==`) |
| `Map<K, V>` | `has(_k: K)` | `Bool` |
| `Map<K, V>` | `remove(_k: K)` | `Bool` (`false` si la clé n'existait pas) |
| `Map<K, V>` | `keys()` / `values()` | `Array<K>` / `Array<V>`, ordre d'insertion |
| `Task<T>` | `cancel()` | `Void` (voir [tâches](execution.md#tâches)) |
| `Task<T>` | `isDone()` | `Bool` (terminée ou annulée) |
| `Listener` | `stop()` | `Void` (désabonne ; sans effet si déjà arrêté) |
| `Listener` | `isActive()` | `Bool` |

## Tâches

| Expression | Type | Règle |
|---|---|---|
| `start f(x)` | `Task<T>` si `f` retourne `T` (`Task<Void>` pour une fonction `Void`) | l'opérande doit être un appel : fonction, méthode de structure, méthode ou fonction native (MCF0121) |
| `await t` | `T` si `t: Task<T>` | autre chose qu'une tâche → MCF0220 |

- `Task` seul accepte toute tâche (résultat non vérifié), comme `Array` seul.
- `Task<Void>` est permis (seul cas où `Void` sert d'argument de type) ; `await` d'une `Task<Void>` ne produit pas de valeur.
- DualType de deux tâches (`Task<Int>, Task<String>`) → MCF0203, comme deux collections de même sorte.

## `duplicate(valeur)`

Copie complète d'une valeur, du **même type** que l'original (D37) : `_copy: Team = duplicate(_team);`. Voir [exécution](execution.md#valeurs).

## Configuration

`config` et `loadConfig` : types acceptés depuis un `.cfg`, lecture seule (MCF0614) — voir [configuration](configuration.md).

## Littéraux

`Int` = entier signé 64 bits (D15) : au-delà → MCF0229.

## Pas encore vérifié

Initialisation avant usage d'une variable déclarée sans valeur (détectée à l'exécution, MCF0405).
