# Structures et énumérations (phase 5)

Base : cahier des charges 1.0 §5.2 (structures), 1.2 §24.3 (point ouvert). Syntaxe adaptée aux conventions 1.2 : pas de `;` après l'accolade fermante d'une déclaration.

## Structures

```rust
struct User {
    id: Int;
    name: String;
    level: Bool, Int;          // un champ peut être DualType
    friends: Array<User>;      // récursion via une collection : autorisé
}

admin: User = User { id: 1, name: "Alex", level: 10, friends: [] };

func rename(_user: User, _name: String): Void {
    _user.name = _name;
}
```

- Déclarée au niveau d'un module ou d'un scope, jamais dans un bloc (MCF0116).
- Les champs s'écrivent **sans** `_` (MCF0120) : ce sont des membres, pas des locales.
- Littéral `Nom { champ: valeur, ... }` : **tous** les champs sont obligatoires (MCF0240), pas de champ inconnu (MCF0241), pas de doublon (MCF0242), chaque valeur du bon type (MCF0220). Virgule finale autorisée.
- Accès `valeur.champ` vérifié (MCF0241). Sur une valeur DualType, vérifier d'abord le type actif (MCF0221).
- Une structure qui se contient elle-même directement (ou via un DualType / une autre structure) est refusée (MCF0244) ; passer par `Array<T>`.
- Une structure ne se crée pas comme une fonction : `User(1)` → MCF0231.
- Une structure ou une énumération ne peut pas porter le nom d'un type natif (MCF0201).

## Énumérations

```rust
enum Status { Online, Offline, Away }

status: Status = Status.Online;

if (status == Status.Away) then { ... }
```

- Variantes sans données pour l'instant ; variante inconnue → MCF0243, doublon → MCF0242.
- Seuls `==` et `!=` s'appliquent ; pas d'ordre (`<`) entre variantes (MCF0230).

## Types venant d'ailleurs

Une structure rangée dans un scope ou un autre fichier s'utilise comme le reste : `import Shop;` puis `Shop.Item` (type et littéral `Shop.Item { ... }`). Un nom qui n'est pas un type utilisé comme type → MCF0245 ; un littéral sur autre chose qu'une structure → MCF0246.

## Pas encore

Méthodes associées aux structures, variantes d'énumération avec données, valeurs par défaut, structures génériques, `match` : à arbitrer.
