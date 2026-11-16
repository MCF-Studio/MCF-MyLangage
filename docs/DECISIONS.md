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
| D15 | Taille des numériques | `Int` = 64 bits signé (vérifié, MCF0229), `Float` = 64 bits IEEE 754 | Non tranché par la spec | 🟡 |
| D16 | Littéral `Char`, exposants | Pas encore | Non spécifié | ⬜ |
| D17 | Licence | GPL-3.0-only, copyright MCF Studio | Choix de MCF Studio : usage libre, modifications republiées sous la même licence | ✅ |
| D18 | Types et fonctions natifs (phase 3) | Types de §5 + `Bytes` (§11.1) ; fonctions `print(x)` et `sleep(ms)` | Seuls éléments natifs cités par la spec | 🟡 |
| D19 | Portée des événements | Visibles dans tout le projet, sans import ; nom unique dans le projet | Les événements servent à relier des modules entre eux | 🟡 |
| D20 | Projet et imports | Un dossier vérifié = un projet ; `import` cible un élément d'un **autre** fichier ; un nom d'un autre fichier sans import → MCF0200 avec suggestion | §8 : « les imports doivent être représentés clairement dans le code source » | 🟡 |
| D21 | Masquage | Avertissement MCF0204 (erreur en `--strict`) | §7.1 : autorisé mais « le compilateur doit détecter les masquages » | 🟡 |
| D22 | Concaténation | `String + (String, Int, Float, Bool, Char)` → `String` | L'exemple §11 écrit `"Joueur connecté : " + _playerId` ; c'est une règle d'opérateur documentée, pas une conversion générale | 🟡 |
| D23 | Promotion numérique | `Int` avec `Float` dans `+ - * / %` et les comparaisons → `Float` ; **jamais** à l'affectation (`speed: Float = 180;` refusé) | L'exemple §10 écrit `_points * _multiplier` (Int × Float) | 🟡 |
| D24 | Narrowing des globales | Autorisé (exemple §6.1), mais annulé après tout appel de fonction MCF ou `emit` | Une fonction appelée peut modifier la globale : on préfère refuser que se tromper | 🟡 |
| D25 | Membres d'une valeur | Non vérifiés (`Unknown`) tant que les structures n'existent pas | Phase 5 | 🟡 |
