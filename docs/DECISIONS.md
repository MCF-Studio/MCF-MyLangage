# Décisions d'implémentation

Le cahier des charges interdit les décisions structurantes silencieuses (§24). Chaque arbitrage pris pour avancer est consigné ici avec son statut.

**Statuts :** ✅ validé · 🟡 proposé, à valider par MCF Studio · ⬜ ouvert

| # | Sujet | Décision | Raison | Statut |
|---|---|---|---|---|
| D1 | Édition de référence | L'édition 1.2 (issue de 1.1) prime sur la 1.0 en cas de contradiction (`set`, `action`, `->`, `};`) | Plus récente | 🟡 |
| D2 | Chiffrement | Tout ce qui transite est chiffré, clés propres à l'utilisateur, aucun accès MCF Studio (§26) | Exigence explicite | ✅ |
| D3 | Type de retour | Obligatoire (`: Void` si rien). La grammaire §21 le marquait optionnel | « Explicite plutôt qu'implicite » ; tous les exemples 1.1 l'écrivent | 🟡 |
| D4 | `;` après un bloc | Avertissement MCF0112, erreur en `--strict` | §3 : pas de `;` après `}` ; l'avertissement aide la migration depuis 1.0 | 🟡 |
| D5 | Instructions au niveau module | Interdites (MCF0115) : uniquement des déclarations, comme la grammaire §21 | Point d'entrée : `func main(): Void` (D29) | 🟡 |
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
| D18 | Types et fonctions natifs (phase 3) | Types de §5 + `Bytes` (§11.1) ; fonctions `print(x)`, `sleep(ms)` et `duplicate(x)` (D37) | Seuls éléments natifs cités par la spec, plus la copie demandée par MCF Studio | 🟡 |
| D19 | Portée des événements | Visibles dans tout le projet, sans import ; nom unique dans le projet | Les événements servent à relier des modules entre eux | 🟡 |
| D20 | Projet et imports | Un dossier vérifié = un projet ; `import` cible un élément d'un **autre** fichier ; un nom d'un autre fichier sans import → MCF0200 avec suggestion | §8 : « les imports doivent être représentés clairement dans le code source » | 🟡 |
| D21 | Masquage | Avertissement MCF0204 (erreur en `--strict`) | §7.1 : autorisé mais « le compilateur doit détecter les masquages » | 🟡 |
| D22 | Concaténation | `String + (String, Int, Float, Bool, Char)` → `String` | L'exemple §11 écrit `"Joueur connecté : " + _playerId` ; c'est une règle d'opérateur documentée, pas une conversion générale | 🟡 |
| D23 | Promotion numérique | `Int` avec `Float` dans `+ - * / %` et les comparaisons → `Float` ; **jamais** à l'affectation (`speed: Float = 180;` refusé) | L'exemple §10 écrit `_points * _multiplier` (Int × Float) | 🟡 |
| D24 | Narrowing des globales | Autorisé (exemple §6.1), mais annulé après tout appel de fonction MCF ou `emit` | Une fonction appelée peut modifier la globale : on préfère refuser que se tromper | 🟡 |
| D25 | Membres d'une valeur | Vérifiés sur les structures ; une valeur d'un type natif n'a que les méthodes natives listées (D38), tout autre membre → MCF0241 | Pas de membre non documenté | 🟡 |
| D26 | Syntaxe des structures | `struct Nom { champ: Type; }` sans `;` final ; littéral `Nom { champ: valeur }` ; champs sans `_` | 1.0 §5.2 adapté aux conventions 1.2 | 🟡 |
| D27 | Littéral de structure | Tous les champs obligatoires, aucune valeur par défaut implicite | « Explicite plutôt qu'implicite » | 🟡 |
| D28 | Énumérations | `enum Nom { A, B }`, variantes sans données, comparables par `==` / `!=` uniquement | Point §24.3 ouvert ; version minimale extensible | 🟡 |
| D29 | Point d'entrée | `func main(): Void` à la racine d'un fichier, sans paramètre, un seul par projet (MCF0250, MCF0251) ; `main` réservé à la racine ; projet sans `main` accepté par `mcf check` (bibliothèque), exigé par `mcf run` | Choix de MCF Studio ; convention connue, explicite. Les arguments de ligne de commande passeront par une API native plutôt que par un paramètre | 🟡 |
| D30 | Partage des valeurs | Structures, `Array` et `Map` partagés (référence) ; nombres, booléens, chaînes et énumérations copiés ; copie explicite avec `duplicate` (D37) | L'exemple `rename(_user, _name)` de structures.md suppose qu'une fonction modifie la structure reçue | 🟡 |
| D31 | Variable sans valeur | `Array` / `Map` démarrent vides ; autre type : lecture avant affectation → MCF0405 à l'exécution | Pas de littéral de `Map` ; pas de valeur par défaut inventée pour `Int` ou une structure. Une analyse statique pourra avancer cette erreur à la vérification | 🟡 |
| D32 | `emit` | Synchrone : chaque `on` s'exécute aussitôt, dans l'ordre du projet ; plusieurs `on` par événement permis | Comportement prévisible avant la phase 8 (asynchrone) | 🟡 |
| D33 | Calculs à l'exécution | `Int` vérifié (dépassement MCF0402, division par zéro MCF0401, division tronquée vers zéro) ; `Float` IEEE 754 | Pas de résultat faux silencieux sur les entiers | 🟡 |
| D34 | Collections à l'exécution | Index hors limites MCF0403 ; clé absente en lecture MCF0404 ; `foreach` sur une `Map` = clés dans l'ordre d'insertion, sur le contenu du début de boucle | Erreur explicite plutôt que valeur inventée | 🟡 |
| D35 | Démarrage | Globales initialisées dans l'ordre (fichiers alphabétiques, puis déclarations), puis `main` ; récursion limitée à 10 000 appels (MCF0406) | Ordre déterministe ; pas de plantage de la pile native | 🟡 |
| D36 | Noms qui ne sont pas des valeurs | Une fonction, un scope ou un type utilisé comme valeur → MCF0232 ; DualType de deux collections de même sorte (`Array<Int>, Array<String>`) → MCF0203 | Pas de fonctions comme valeurs pour l'instant ; `is` ne distingue pas deux `Array` à l'exécution. Même règle pour deux `Task` (D41) | 🟡 |
| D37 | Copie explicite | `duplicate(x)` : copie profonde du même type que `x` ; la copie ne partage rien avec l'original (partages internes et cycles reproduits) | Demande de MCF Studio : copier sans toucher à l'original, le partage restant le comportement par défaut | ✅ |
| D38 | Méthodes natives | `Array` : `length push pop removeAt contains` ; `Map` : `length has remove keys values` ; `String` : `length`. Appel obligatoire (`x.length()`), méthode utilisée comme valeur → MCF0232 ; `pop` vide → MCF0409 | Minimum pour faire grandir et parcourir les collections ; noms anglais comme le reste du langage. Validé par MCF Studio : des noms courants, déjà vus ailleurs | ✅ |
| D39 | Méthodes de structures | `func` déclarées dans le corps du `struct` ; receveur implicite `self` (mot-clé), champs atteints par `self.champ` uniquement ; appel `valeur.methode()` ; nom distinct des champs (MCF0242) ; `self` hors méthode → MCF0247. Pas encore de méthodes statiques | Convention répandue (Swift, Rust, Python) ; accès explicite par `self.` pour savoir d'où vient chaque nom | 🟡 |
| D40 | Alias d'import | `import Chemin as alias;` ; seul l'alias est visible ; alias sans `_`, ni `main`, ni en collision (MCF0103, MCF0250, MCF0208) ; un élément importé une seule fois par fichier (MCF0208). `as` passe de mot réservé à mot-clé | §8.2 : « l'IDE propose une référence qualifiée ou un alias » ; §24.4. Un seul nom par élément et par fichier pour que l'origine de chaque nom reste lisible | 🟡 |
| D41 | Asynchrone | Fonctions **sans coloration** (pas de `async`) : `start appel(...)` lance une tâche et vaut `Task<T>`, `await t` attend son résultat ; `Task<T>` a `cancel()` et `isDone()`. `start` et `await` deviennent mots-clés | Choix de MCF Studio (§12, §24.8) : le plus simple à écrire ; contrepartie assumée, le compilateur ne peut pas prouver qu'une fonction ne bloque pas | ✅ |
| D42 | Exécution des tâches | Une seule tâche à la fois, changement uniquement aux points de suspension (`sleep`, `await`, fin) ; ordre déterministe (tâches prêtes, puis minuteries sur horloge virtuelle) ; le programme attend toutes les tâches ; une erreur dans une tâche arrête tout ; annulation coopérative (MCF0410) ; blocage mutuel détecté (MCF0411) ; narrowing des globales annulé après `sleep` / `await` | Pas de course de données ni de résultat qui change d'une exécution à l'autre ; rien ne se perd ni n'échoue en silence. Le parallélisme réel sur plusieurs cœurs pourra venir avec le backend natif, sans changer la syntaxe | 🟡 |
| D43 | Conseils pour apprendre | Troisième sévérité `conseil` (codes MCF05xx), jamais bloquante, même en `--strict` ; masquable par `--no-hints` ; donnée seulement quand le compilateur est sûr (dans le doute, rien) | Demande de MCF Studio : MCF doit aussi servir aux débutants. Un conseil faux enseignerait une mauvaise règle, d'où le critère « sûr ou silencieux » | 🟡 |
