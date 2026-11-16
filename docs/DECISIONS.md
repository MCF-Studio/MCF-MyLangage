# Décisions d'implémentation

Le cahier des charges interdit les décisions structurantes silencieuses (§24). Chaque arbitrage pris pour avancer est consigné ici avec son statut.

**Statuts :** ✅ validé · 🟡 proposé, à valider par MCF Studio · ⬜ ouvert

| # | Sujet | Décision | Raison | Statut |
|---|---|---|---|---|
| D1 | Édition de référence | L'édition 1.2 (issue de 1.1) prime sur la 1.0 en cas de contradiction (`set`, `action`, `->`, `};`) | Plus récente | 🟡 |
| D2 | Chiffrement | Tout ce qui transite est chiffré, clés propres à l'utilisateur, aucun accès MCF Studio (§26) | Exigence explicite | ✅ |
| D3 | Type de retour | Obligatoire (`: Void` si rien). La grammaire §21 le marquait optionnel | « Explicite plutôt qu'implicite » ; tous les exemples 1.1 l'écrivent | 🟡 |
| D4 | `;` après un bloc | Avertissement MCF0112, erreur en `--strict` | §3 : pas de `;` après `}` ; l'avertissement aide la migration depuis 1.0 | 🟡 |
| D5 | Instructions au niveau module | Interdites (MCF0115) : uniquement des déclarations, comme la grammaire §21 | Le point d'entrée (`main` ?) reste à définir | 🟡 |
| D6 | Contenu d'un `scope` | Des déclarations (globales, func, scopes imbriqués…), pas des instructions | Exemple §7 | 🟡 |
| D7 | `foreach` | Type de la variable de boucle optionnel : `foreach (_p in xs)` ou `foreach (_p: String in xs)` | Exemple §9.4 sans type, grammaire §21 avec | 🟡 |
| D8 | `for` | 3ᵉ partie = affectation ou expression sans `;` | Exemple §9.3 (`_i = _i + 1`) | 🟡 |
| D9 | `else if` | Autorisé : `else if (...) then { }` | Prévu en 1.0 §6.1 | 🟡 |
| D10 | Priorités des opérateurs | Table de [grammaire.md](grammaire/grammaire.md) ; comparaisons non associatives | Décision §24.1 ouverte ; la non-associativité évite les pièges `0 < x < 10` | 🟡 |
| D11 | Types génériques | `Nom<T, U>` dans les annotations de type | `List<T>`, `Map<K, V>` en 1.0 §4.1 | 🟡 |
| D12 | Événements | Syntaxe 1.1 §11 : `event X(_a: T);`, `on X(_a) { }`, `emit X(v);` ; noms qualifiés `User.Connected` autorisés | Repris de 1.0 §8 | 🟡 |
| D13 | Identifiants | ASCII uniquement | Moins d'ambiguïté (humains et IA) | 🟡 |
| D14 | Mots réservés | Liste dans [lexique.md](grammaire/lexique.md) | Permettre d'ajouter `struct`, `async`, `let`… sans casser de code | 🟡 |
| D15 | Taille des numériques | Proposition : `Int` = 64 bits signé, `Float` = 64 bits IEEE 754 | Non tranché par la spec | ⬜ |
| D16 | Littéral `Char`, exposants | Pas encore | Non spécifié | ⬜ |
| D17 | Licence | Aucune pour l'instant (tous droits réservés), code public pour audit | Langage décrit comme propriétaire ; une licence est nécessaire pour accepter des contributions | ⬜ |
