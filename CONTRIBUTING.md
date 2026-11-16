# Participer à MCF

Merci de passer par ici ! MCF est un side project : il avance par petites touches, et chaque coup de main compte — une idée de syntaxe, un bug, un test, une relecture de la doc.

## Par où commencer

| Envie | Quoi faire |
|---|---|
| Donner ton avis sur la syntaxe | Ouvre une issue « Idée de syntaxe », ou commente les décisions 🟡 de [DECISIONS.md](docs/DECISIONS.md) |
| Casser le compilateur | Écris du MCF tordu, lance `mcf check`, ouvre une issue « Bug » si le diagnostic est faux ou peu clair |
| Ajouter un test | Un fichier dans `tests/conformance/valid/` ou `invalid/` (voir ci-dessous) |
| Coder | Prends une ligne de la [ROADMAP](ROADMAP.md), annonce-toi dans une issue avant de te lancer |
| Relire la sécurité | Challenge [chiffrement-et-cles.md](docs/securite/chiffrement-et-cles.md) : une menace oubliée vaut de l'or |

## Règles du projet (issues du cahier des charges, §20)

- Aucune règle de syntaxe sans documentation (`docs/grammaire/`) ni tests.
- Chaque erreur du compilateur a un identifiant stable `MCFxxxx` ; un identifiant publié ne change jamais de sens.
- Pas de conversion implicite non documentée, pas de type dynamique pour contourner DualType.
- Pas de dépendance inutile ; pas de cryptographie maison.
- Rien qui transite en clair, aucune télémétrie, aucune clé côté MCF Studio (§26). Une PR qui affaiblit ça sera refusée.

## Tests de conformité

```text
tests/conformance/valid/xxx.mcf     → doit être accepté sans aucun diagnostic
tests/conformance/invalid/xxx.mcf   → 1re ligne : // attendu: MCF0104, MCF0101
```

## Avant d'ouvrir une PR

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Licence des contributions

MCF est sous [GPL-3.0-only](LICENSE), © MCF Studio. En proposant une contribution (PR, patch), tu acceptes qu'elle soit publiée sous cette licence, et que MCF Studio reste le mainteneur qui décide de ce qui est intégré au langage.

Ajoute une ligne `Signed-off-by: Ton Nom <email>` à tes commits (`git commit -s`) pour certifier que tu as le droit de proposer ce code ([Developer Certificate of Origin](https://developercertificate.org/)).
