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

## Méthodes

Une structure peut déclarer des fonctions dans son corps, après ou entre ses champs. Elles s'appellent sur une valeur : `valeur.methode(...)`. Dans le corps, **`self`** désigne la valeur qui reçoit l'appel.

```rust
struct Account {
    owner: String;
    balance: Int;

    func deposit(_amount: Int): Void {
        self.balance = self.balance + _amount;
    }

    func describe(): String {
        return self.owner + " : " + self.balance;
    }
}

func main(): Void {
    _a: Account = Account { owner: "Alex", balance: 0 };
    _a.deposit(100);
    print(_a.describe());     // Alex : 100
}
```

- `self` est un mot-clé ; il n'est pas écrit dans les paramètres. Hors d'une méthode → MCF0247. On ne peut pas lui affecter une autre valeur (`self = ...` → MCF0114), seulement modifier ses champs.
- Les champs s'atteignent toujours par `self.champ` : pas d'accès implicite, on voit d'où vient chaque nom.
- Une structure étant partagée (D30), une méthode qui modifie `self` modifie la valeur de l'appelant. Pour travailler sur une copie : `duplicate(valeur).methode()`.
- Une méthode et un champ d'une même structure ne peuvent pas porter le même nom, ni deux méthodes entre elles (MCF0242).
- Nom inconnu → MCF0241 (la liste des champs et méthodes est proposée) ; méthode utilisée sans l'appeler → MCF0232 ; mauvais arguments → MCF0210 / MCF0220.
- Une méthode suit les règles d'une fonction : type de retour obligatoire, `return` sur tous les chemins (MCF0226). Comme une fonction, son appel annule le narrowing des globales (D24).
- Dans une erreur d'exécution, la trace la nomme `Structure.methode`.

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

Méthodes statiques (sans `self`, ex. `User.create(...)`), méthodes d'énumération, variantes d'énumération avec données, valeurs par défaut, structures génériques, `match` : à arbitrer.
