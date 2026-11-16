# MCF – MyLangage

Langage de programmation de **MCF Studio** : typage strict avec **DualType** (une variable peut porter un ou deux types, jamais plus), scopes nommés, événements natifs, interfaces libres façon HTML/CSS (`.uimcf`), configuration JSON typée (`.cfg`), et un compilateur pensé pour être lu aussi bien par des humains que par des agents IA.

> ## 🔒 MCF Studio ne peut pas vous surveiller
>
> Tout ce qui transite avec MCF est **chiffré**, avec des **clés générées et conservées chez l'utilisateur**.
> MCF Studio ne détient aucune clé, n'impose aucun serveur relais et ne collecte **aucune télémétrie** :
> il n'a **strictement aucun moyen technique** de lire ou de surveiller les échanges réalisés avec son propre langage.
>
> → [Chiffrement et souveraineté des clés](docs/securite/chiffrement-et-cles.md)
>
> *Statut : engagement de conception (spécification). L'implémentation viendra avec les phases réseau/runtime.*

## Aperçu

```rust
scope Player {
    name: String = "Alex";
    state: Bool, Int = true;        // DualType : Bool ou Int, jamais autre chose

    func showState(_bonus: Int): Void {
        if (state is Int) then {
            print(state + _bonus);
        }
        else {
            print(state);
        }
    }
}

event PlayerConnected(_playerId: Int);

on PlayerConnected(_playerId) {
    print(_playerId);
}
```

- Variables globales de module : `nom: Type = valeur;`
- Variables locales et paramètres : préfixe `_` obligatoire
- `if (…) then { }`, `while (…) do { }`, `foreach (_x in xs) do { }`

## État du projet

Side project en construction — **phase 1 : fondations** (lexer, parser, AST, diagnostics, CLI `mcf check`). Voir [ROADMAP.md](ROADMAP.md).

```bash
cargo build --release
./target/release/mcf check examples/            # diagnostics lisibles
./target/release/mcf check examples/ --format json --strict
./target/release/mcf describe errors            # liste des diagnostics possibles
```

## Documentation

| Document | Contenu |
|---|---|
| [Cahier des charges 1.2](docs/cahier-des-charges/edition-1.2.md) | Référence du langage, de l'IDE et de la sécurité |
| [Cahier des charges 1.0](docs/cahier-des-charges/edition-1.0.md) | Historique (remplacé) |
| [Chiffrement et clés](docs/securite/chiffrement-et-cles.md) | Engagement « aucun accès éditeur », menaces, limites |
| [Lexique](docs/grammaire/lexique.md) | Tokens, mots-clés, littéraux |
| [Grammaire](docs/grammaire/grammaire.md) | Grammaire formelle implémentée par le parser |
| [Décisions](docs/DECISIONS.md) | Arbitrages pris pendant l'implémentation, à valider |

## Organisation

```text
compiler/diagnostics   identifiants stables, rendu texte et JSON
compiler/syntax        lexer, AST, parser
cli                    exécutable `mcf`
tests/conformance      programmes valides / invalides avec diagnostic attendu
examples               exemples MCF
docs                   spécifications
```

## Licence

Aucune licence n'est accordée pour l'instant : le code est public pour être lu et audité, tous droits réservés à MCF Studio.
