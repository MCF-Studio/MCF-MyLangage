<div align="center">

# MCF – MyLangage

**Un langage strict, lisible d'un coup d'œil, pensé pour les humains *et* les IA — et incapable de vous espionner.**

[![CI](https://github.com/MCF-Studio/MCF-MyLangage/actions/workflows/ci.yml/badge.svg)](https://github.com/MCF-Studio/MCF-MyLangage/actions/workflows/ci.yml)
![Statut](https://img.shields.io/badge/statut-phase%208%20%C2%B7%20t%C3%A2ches-orange)
[![Licence](https://img.shields.io/badge/licence-GPL--3.0-blue)](LICENSE)
![Rust](https://img.shields.io/badge/compilateur-Rust-b7410e)
![Contributions](https://img.shields.io/badge/contributions-bienvenues-brightgreen)

</div>

---

## Pourquoi encore un langage ?

Soyons honnêtes : **MCF est un side project** de [MCF Studio]. Il avance le soir et le week-end, sans roadmap commerciale ni deadline.

Mais l'idée derrière les fichiers `.mcf` mérite qu'on s'y attarde. La plupart des langages vous laissent écrire du code *ambigu* puis vous aident à le déboguer. MCF prend le problème à l'envers : **rendre l'ambiguïté impossible à écrire**, pour que le code se lise sans effort — par vous dans six mois, par un collègue, ou par un agent IA qui doit le modifier sans rien casser.

Concrètement, ça donne quelques choix qu'on ne retrouve pas ailleurs, en tout cas pas ensemble :

### 1. Une variable locale se voit au premier coup d'œil

```rust
playerName: String = "Alex";          // globale du module

func greet(_bonus: Int): Void {        // paramètre : préfixe _
    _total: Int = 10 + _bonus;         // locale : préfixe _
}
```

Le `_` n'est pas une convention de style : c'est **une règle du compilateur**. Une locale sans `_` est une erreur, une globale avec `_` aussi. Plus besoin de remonter 200 lignes pour savoir d'où sort une variable.

### 2. DualType : de la souplesse, mais avec un plafond

```rust
playerState: Bool, Int = true;    // Bool OU Int
playerState = 42;                 // ok
playerState = "online";           // ERREUR : troisième type interdit

if (playerState is Int) then {
    print(playerState + 10);      // ici le compilateur SAIT que c'est un Int
}
```

Entre le typage rigide et le « tout est `any` », MCF choisit une voie étroite : **deux types maximum**, jamais de conversion implicite, et le compilateur suit le type actif dans chaque branche.

### 3. Une syntaxe qui ne laisse pas deviner

`if (…) then { }`, `while (…) do { }`, `;` obligatoire, type de retour obligatoire, `0 < x < 10` refusé (on écrit `0 < x && x < 10`). C'est un peu plus verbeux, et c'est voulu : chaque ligne n'a qu'une seule lecture possible.

### 4. Des erreurs écrites pour être comprises — y compris par une IA

```text
erreur[MCF0104] GLOBAL_DECLARATION_IN_BLOCK: `total` est déclarée dans un bloc sans le préfixe `_`
  --> main.mcf:3:5
  |
3 |     total: Int = 0;
  |     ^^^^^
  = suggestion : une variable locale s'écrit `_total`
```

Chaque erreur a un **identifiant stable**, une position exacte, ce qui était attendu, ce qui a été reçu et une suggestion. La même chose existe en JSON (`--format json`) pour les outils et agents IA. `mcf describe errors` liste toutes les erreurs possibles.

### 5. 🔒 MCF Studio ne peut pas vous surveiller. Par construction.

> Tout ce qui transite avec MCF est **chiffré**, avec des **clés générées et conservées chez l'utilisateur**.
> MCF Studio ne détient aucune clé, n'impose aucun serveur relais et ne collecte **aucune télémétrie** :
> il n'a **strictement aucun moyen technique** de lire ou de surveiller les échanges faits avec son propre langage.

Dans MCF, le chemin le plus simple est le chemin chiffré : la bibliothèque réseau n'a pas d'API en clair par défaut, et un type `Secret<T>` empêche, dès la compilation, qu'un mot de passe finisse dans un `print` ou un log.
→ Tout est détaillé, limites comprises, dans [Chiffrement et souveraineté des clés](docs/securite/chiffrement-et-cles.md).

*Statut : engagement de conception. Il sera implémenté avec les phases réseau/runtime, et le code étant public, chacun pourra le vérifier.*

### Et ensuite

Des interfaces `.uimcf` aussi libres que du HTML/CSS (sans framework imposé), des configurations `.cfg` en JSON validées par le typage, des événements natifs (`event` / `on` / `emit`), un export web autonome, et à terme un IDE unique où code et conception visuelle restent synchronisés.

---

## Essayer

```bash
git clone https://github.com/MCF-Studio/MCF-MyLangage.git
cd MCF-MyLangage
cargo build --release

./target/release/mcf check examples/                    # vérifier un projet
./target/release/mcf run examples/hello                 # l'exécuter (à partir de func main(): Void)
./target/release/mcf check examples/ --format json --strict
./target/release/mcf describe errors                    # toutes les erreurs possibles
./target/release/mcf describe keywords
```

> Aujourd'hui MCF **analyse** le code (syntaxe, noms, scopes, imports entre fichiers, types, DualType, structures et énumérations) et l'**exécute** avec un interpréteur (`mcf run`), tâches concurrentes comprises (`start` / `await`). Il vérifie aussi les configurations `.cfg` et les interfaces `.uimcf`. L'affichage des interfaces (backend web) et le backend natif viendront ensuite : voir la [feuille de route](ROADMAP.md).

## 🤝 Participer

C'est un side project, donc **toute aide fait vraiment avancer les choses**, et pas besoin d'être expert en compilateurs :

- 💡 **Tu as un avis sur la syntaxe ?** Les décisions marquées 🟡 dans [DECISIONS.md](docs/DECISIONS.md) attendent des retours. Une issue suffit.
- 🐛 **Casse le compilateur** : écris du MCF bizarre, lance `mcf check`, signale tout diagnostic faux ou peu clair.
- 🧪 **Ajoute un test** : un fichier `.mcf` dans `tests/conformance/` et c'est fait.
- 🔐 **Challenge la sécurité** : une menace oubliée dans le modèle de chiffrement, c'est une contribution précieuse.
- 🦀 **Code en Rust** : prends une ligne de la [roadmap](ROADMAP.md) et annonce-toi dans une issue.

Tout est expliqué dans [CONTRIBUTING.md](CONTRIBUTING.md). Les ⭐ font aussi plaisir.

## Documentation

| Document | Contenu |
|---|---|
| [Cahier des charges 1.2](docs/cahier-des-charges/edition-1.2.md) | Référence du langage, de l'IDE et de la sécurité |
| [Chiffrement et clés](docs/securite/chiffrement-et-cles.md) | Engagement « aucun accès éditeur », menaces, limites |
| [Lexique](docs/grammaire/lexique.md) · [Grammaire](docs/grammaire/grammaire.md) · [Résolution des noms](docs/grammaire/resolution-des-noms.md) · [Types](docs/grammaire/types.md) · [Structures](docs/grammaire/structures.md) · [Exécution](docs/grammaire/execution.md) · [Configuration](docs/grammaire/configuration.md) · [Interfaces](docs/grammaire/interfaces.md) · [Conseils pour apprendre](docs/grammaire/conseils.md) | Ce que le compilateur vérifie et exécute aujourd'hui |
| [Décisions](docs/DECISIONS.md) | Arbitrages en attente de validation |
| [Cahier des charges 1.0](docs/cahier-des-charges/edition-1.0.md) | Historique |

## Organisation du dépôt

```text
compiler/diagnostics   identifiants d'erreurs stables, rendu texte et JSON
compiler/syntax        lexer, AST, parser
compiler/semantic      résolution des noms, scopes, imports, types, structures
compiler/runtime       interpréteur (`mcf run`)
cli                    l'exécutable `mcf`
tests/conformance      programmes valides / invalides avec l'erreur attendue
tests/execution        programmes exécutés et leur sortie attendue
examples               exemples MCF
docs                   spécifications
```

## Licence

MCF est publié sous **[GNU GPL v3](LICENSE)** (GPL-3.0-only) — © MCF Studio.

En clair :
- ✅ tu peux utiliser MCF, l'étudier, le partager ;
- 🔁 si tu le modifies et que tu distribues ta version, tu dois la publier sous la même licence, avec son code source ;
- 👑 MCF Studio reste l'auteur et le mainteneur du projet : le nom MCF et la direction du langage lui appartiennent.

Le texte de la licence fait foi ; ce résumé n'en est qu'une lecture rapide.

[MCF Studio]: https://mcfstudio.fr
