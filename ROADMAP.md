# Feuille de route

Ordre tiré du cahier des charges 1.2 (§22). Une phase n'est terminée que lorsque ses tests passent.

| Phase | Objectif | État |
|---|---|---|
| 1 | Grammaire officielle, lexique, diagnostics | 🟡 en cours |
| 2 | Lexer, parser, AST, `mcf check` (syntaxe) | 🟡 en cours |
| 3 | Types, variables, scopes (résolution des symboles) | ⬜ |
| 4 | DualType, type narrowing, analyse sémantique | ⬜ |
| 5 | Fonctions et structures | ⬜ |
| 6 | Conditions et boucles (exécution) | ⬜ |
| 7 | Imports et résolution des symboles | ⬜ |
| 8 | Événements et asynchrone | ⬜ |
| 9 | Configuration `.cfg` | ⬜ |
| 10 | Interfaces `.uimcf` | ⬜ |
| 11 | Backend web | ⬜ |
| 12 | Runtime, backend natif, **module `Secure` et chiffrement réseau** | ⬜ |
| 13 | Language Server et outils IA | ⬜ |
| 14 | IDE unifié (Tauri 2 + Vue 3) | ⬜ |
| 15 | Tests, optimisation, stabilisation, audit sécurité | ⬜ |

Le chiffrement (§26) n'est pas une phase isolée : chaque phase qui fait sortir une donnée d'un processus (IPC IDE ↔ LSP, registre, réseau) doit l'intégrer dès sa conception.
