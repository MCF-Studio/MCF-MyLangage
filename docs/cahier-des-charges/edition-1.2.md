# MCF – MyLangage
## Cahier des charges officiel – Langage, grammaire et IDE

**Édition :** 1.2 (1.1 + chiffrement de bout en bout et souveraineté des clés)  
**Organisation :** MCF Studio  
**Statut :** Spécification de conception consolidée  
**Objectif :** Définir le langage MCF, son environnement de développement et les bases techniques nécessaires à son implémentation.

---

# 1. Vision du projet

MCF (MyLangage) est un langage de programmation propriétaire développé par MCF Studio.

Il doit permettre de développer :
- Des applications générales.
- Des outils et logiciels de bureau.
- Des applications web.
- Des interfaces graphiques personnalisées.
- Des systèmes de scripting.
- Des outils automatisés.
- Des applications utilisant des événements et des traitements asynchrones.

MCF doit être suffisamment généraliste pour ne pas être limité à un domaine particulier.

Son identité repose sur plusieurs principes :

1. Une syntaxe explicite et lisible.
2. Un typage strict, avec une particularité de double typage contrôlé.
3. Une gestion claire des scopes et des imports.
4. Une gestion des événements native au langage.
5. Une création d'interfaces graphiques très libre.
6. Une compilation fiable et des diagnostics précis.
7. Une conception adaptée au développement assisté par IA.
8. Un environnement de développement complet réunissant code et conception visuelle.
9. **Un chiffrement systématique de tout ce qui transite, avec des clés propres à chaque utilisateur : MCF Studio n'a techniquement aucun moyen de lire ou de surveiller les échanges réalisés avec son propre langage** (voir §26).

L'objectif n'est pas de créer simplement un langage expérimental, mais un véritable écosystème de développement.

# 2. Extensions et organisation des fichiers

| Extension | Utilisation |
|---|---|
| `.mcf` | Code source principal |
| `.uimcf` | Définition des interfaces utilisateur |
| `.cfg` | Configuration au format JSON |
| `.mcfproj` | Projet MCF et configuration de développement |

Les fichiers `.uimcf` et `.cfg` doivent être intégrés au projet MCF et reconnus nativement par le compilateur et l'IDE.

Les fichiers `.uimcf` ne doivent pas constituer un environnement indépendant : ils font partie du même projet et sont liés au code MCF.

Les fichiers `.cfg` utilisent une structure JSON standard, avec validation des données par le système de types MCF.

# 3. Conventions générales de syntaxe

| Élément | Convention |
|---|---|
| Sensibilité à la casse | Oui |
| Blocs | `{ }` |
| Instructions | `;` obligatoire |
| Conditions | Parenthèses obligatoires |
| Déclaration locale | Préfixe `_` |
| Déclaration globale | Sans préfixe `_` |
| Déclaration de fonction | `func` |
| Déclaration de scope | `scope` |
| Commentaire simple | `//` |
| Commentaire multiligne | `/* */` |
| Affectation | `=` |
| Comparaison | `==`, `!=`, `<`, `>`, `<=`, `>=` |

Les blocs de contrôle et les déclarations de fonctions ne nécessitent pas de point-virgule après leur accolade fermante. Cette convention devra être confirmée dans la grammaire définitive.

# 4. Système de déclaration des variables

## 4.1. Suppression du mot-clé `set`

MCF n'utilise pas de mot-clé `set` pour déclarer les variables.

La déclaration est directement composée du nom, du type et éventuellement d'une valeur initiale.

```rust
playerName: String = "Alex";
playerLevel: Int = 10;
isConnected: Bool = true;

_playerSpeed: Float = 4.5;
_tempScore: Int = 0;
_isValid: Bool = false;
```

## 4.2. Variables globales de module

Une variable sans préfixe `_` est une variable appartenant au module.

Elle est accessible dans le contexte de son module, sous réserve des règles de visibilité.

```rust
playerName: String = "Alex";
playerLevel: Int = 10;
```

Les variables globales peuvent être organisées dans des scopes nommés.

## 4.3. Variables locales

Une variable locale doit obligatoirement commencer par `_`.

```rust
func calculate(): Int {
    _result: Int = 10;
    _multiplier: Int = 2;

    return _result * _multiplier;
}
```

Cette convention est universelle.

Elle s'applique :
- Aux variables locales.
- Aux paramètres de fonctions.
- Aux variables de boucles.
- Aux variables de conditions.
- Aux variables temporaires.
- Aux variables locales dans les scopes imbriqués.

Exemple de fonction :

```rust
func calculate(_points: Int, _multiplier: Float): Float {
    _result: Float = _points * _multiplier;
    return _result;
}
```

Exemple de boucle :

```rust
foreach (_player in players) do {
    print(_player);
}
```

## 4.4. Affectation

Une affectation ne doit jamais créer implicitement une variable.

```rust
playerName = "Max";
_playerSpeed = 5.0;
```

Si la variable n'existe pas dans le contexte accessible, le compilateur doit retourner une erreur.

Une déclaration et une affectation sont deux opérations distinctes.

# 5. Système de types

Le typage est strict.

Types fondamentaux envisagés :

| Type | Description |
|---|---|
| `Bool` | Booléen |
| `Int` | Entier |
| `Float` | Nombre décimal |
| `String` | Chaîne de caractères |
| `Char` | Caractère |
| `Byte` | Octet |
| `Array` | Tableau |
| `Map` | Dictionnaire |
| `DateTime` | Date et heure |
| `Void` | Absence de valeur de retour |

Les types composés, génériques, structures, énumérations et interfaces devront compléter le système.

Le type `Any` reste à arbitrer. Il ne doit pas permettre de contourner librement le typage strict.

# 6. Particularité MCF : DualType

MCF permet à une variable d'accepter un ou deux types maximum.

```rust
playerState: Bool, Int = true;

playerState = 42;
playerState = false;
```

La variable conserve les types autorisés pendant toute son existence.

Une affectation avec un troisième type est interdite.

```rust
playerState = "online"; // ERREUR
```

Il s'agit d'un système de types somme, représenté par une valeur active unique parmi les types autorisés.

## 6.1. Vérification du type actif

Le langage doit permettre de vérifier le type actif d'une variable.

```rust
if (playerState is Int) then {
    print(playerState + 10);
}
else {
    print(playerState);
}
```

Le compilateur doit comprendre le type actif dans chaque branche.

Cette opération est appelée *type narrowing*.

## 6.2. Utilisation générale

DualType doit être disponible pour :
- Les variables.
- Les paramètres de fonctions.
- Les valeurs de retour.
- Les propriétés des interfaces.
- Les données d'événements.
- Les métadonnées.

Le nombre maximal de types autorisés reste fixé à deux.

Les conversions implicites générales ne sont pas autorisées.

# 7. Scopes et espaces de noms

Les scopes permettent d'organiser les variables, fonctions et autres éléments du programme.

Exemple :

```rust
scope Player {
    name: String = "Alex";
    level: Int = 10;

    func getName(): String {
        return name;
    }
}

scope Vehicle {
    name: String = "Comet";
    speed: Float = 180.0;
}
```

Les deux scopes peuvent posséder une variable `name` sans conflit.

Accès qualifié :

```rust
print(Player.name);
print(Vehicle.name);
```

## 7.1. Visibilité locale

Les variables locales sont accessibles dans leur scope et dans les scopes enfants.

```rust
func calculate(): Int {
    _value: Int = 10;

    if (true) then {
        _result: Int = _value * 2;
        print(_result);
    }

    return _value;
}
```

Règles :
- Un enfant peut accéder aux variables locales de ses parents.
- Un parent ne peut pas accéder aux variables de ses enfants.
- Un scope frère ne peut pas accéder aux variables locales d'un autre scope frère.
- Une variable locale cesse d'être accessible lorsque son scope se termine.
- Une variable locale peut masquer une variable homonyme d'un scope parent.

Le compilateur doit détecter les masquages et permettre de retrouver les déclarations concernées.

# 8. Imports intelligents

Les scopes peuvent être importés depuis d'autres modules.

Syntaxe proposée :

```rust
import Player;
```

Ou pour un élément particulier :

```rust
import Player.name;
```

L'IDE doit automatiser autant que possible la gestion de ces imports.

## 8.1. Autocomplétion contextuelle

Lorsqu'un développeur commence à écrire un identifiant, l'IDE recherche les éléments accessibles et les éléments disponibles dans les modules importables.

Exemple :

```rust
na
```

Suggestions :

| Élément | Type | Origine |
|---|---|---|
| `name` | String | Player — player.mcf |
| `name` | String | Vehicle — vehicle.mcf |
| `name` | String | Company — company.mcf |

L'origine doit toujours être clairement affichée.

## 8.2. Sélection et import automatique

Lorsqu'un développeur sélectionne un élément provenant d'un autre scope :

1. L'IDE identifie le scope et le module d'origine.
2. Il vérifie si l'import existe déjà.
3. Il ajoute automatiquement l'import nécessaire.
4. Il vérifie les éventuelles collisions.
5. Il actualise les suggestions.

Si deux imports créent une ambiguïté, l'IDE propose une référence qualifiée ou un alias.

L'accès explicite reste possible :

```rust
Player.name;
Vehicle.name;
```

Les imports doivent être représentés clairement dans le code source, même lorsqu'ils ont été générés automatiquement.

# 9. Conditions et contrôle du programme

## 9.1. Conditions

Syntaxe principale :

```rust
if (condition) then {
    // Instructions
}
else {
    // Instructions
}
```

Exemple :

```rust
playerLevel: Int = 15;

if (playerLevel >= 10) then {
    print("Niveau suffisant");
}
else {
    print("Niveau insuffisant");
}
```

Opérateurs logiques :
- `&&`
- `||`
- `!`

Opérateurs de comparaison :
- `==`
- `!=`
- `<`
- `>`
- `<=`
- `>=`

## 9.2. Boucle while

```rust
_counter: Int = 0;

while (_counter < 10) do {
    print(_counter);
    _counter = _counter + 1;
}
```

## 9.3. Boucle for

```rust
for (_i: Int = 0; _i < 10; _i = _i + 1) do {
    print(_i);
}
```

## 9.4. Boucle foreach

```rust
players: Array = ["Alex", "Max", "Tom"];

foreach (_player in players) do {
    print(_player);
}
```

## 9.5. Contrôle des boucles

```rust
break;
continue;
```

# 10. Fonctions

Déclaration :

```rust
func calculateScore(_points: Int, _multiplier: Float): Float {
    _result: Float = _points * _multiplier;
    return _result;
}
```

Appel :

```rust
score: Float = calculateScore(100, 1.5);
```

Fonction sans retour :

```rust
func sayHello(_name: String): Void {
    print("Bonjour " + _name);
}
```

Les fonctions devront prendre en charge :
- Les paramètres typés.
- Les paramètres DualType.
- Les valeurs de retour typées.
- Les fonctions sans retour.
- Les fonctions récursives.
- Les fonctions génériques.
- Les fonctions asynchrones.
- Les paramètres optionnels, si retenus.
- Les surcharges, si retenues.

Les conventions exactes de surcharge et de fonctions génériques seront définies dans une évolution ultérieure.

# 11. Événements

Le système d'événements est une fonctionnalité native de MCF.

Il doit permettre de déclarer, écouter et déclencher des événements.

Syntaxe proposée :

```rust
event PlayerConnected(_playerId: Int);

on PlayerConnected(_playerId) {
    print("Joueur connecté : " + _playerId);
}

emit PlayerConnected(42);
```

Les événements doivent prendre en charge :
- Des données typées.
- Plusieurs paramètres.
- Des métadonnées.
- Plusieurs abonnés.
- Une gestion des abonnements et désabonnements.
- Des événements synchrones et asynchrones.

## 11.1. Métadonnées

Les métadonnées doivent pouvoir transporter des informations supplémentaires sans obliger à créer une nouvelle structure pour chaque besoin.

Types envisagés :
- String
- Int
- Float
- Bool
- DateTime
- Bytes
- JSON
- Structures sérialisables

Le système devra être typé et contrôlé par le compilateur.

# 12. Programmation asynchrone

MCF devra intégrer un système natif de gestion asynchrone.

Objectifs :
- Exécuter plusieurs opérations sans bloquer l'interface.
- Gérer les tâches longues.
- Attendre des opérations externes.
- Gérer les événements asynchrones.
- Contrôler les erreurs et annulations.

Les mots-clés et conventions définitifs restent à définir.

# 13. Interfaces utilisateur – `.uimcf`

Les interfaces sont une composante centrale de MCF.

L'objectif est de permettre la création d'interfaces très libres, comparables en flexibilité à HTML et CSS.

Le développeur doit pouvoir :
- Créer des fenêtres.
- Créer des composants.
- Définir des positions.
- Définir des dimensions.
- Définir des couleurs.
- Définir des bordures.
- Définir des animations.
- Gérer les interactions.
- Lier les composants aux variables MCF.
- Déclarer des événements.
- Créer des interfaces adaptatives.

Les interfaces ne doivent pas être enfermées dans une bibliothèque de composants prédéfinis.

Le développeur doit pouvoir concevoir ses propres éléments et styles.

## 13.1. Séparation logique et présentation

Le code métier reste dans les fichiers `.mcf`.

La définition visuelle peut être stockée dans les fichiers `.uimcf`.

Les deux doivent être intégralement intégrés au même projet et gérés depuis le même IDE.

Le système doit permettre :
- La liaison des propriétés UI aux variables.
- La liaison des événements UI aux fonctions.
- La gestion des états réactifs.
- La création de composants réutilisables.
- L'accès aux propriétés depuis le code.
- La prévisualisation instantanée.

Les règles précises de syntaxe `.uimcf` seront définies dans une spécification dédiée.

# 14. IDE MCF – Environnement de développement unifié

## 14.1. Principe fondamental

L'IDE MCF est le centre de développement de l'écosystème.

Il ne doit pas exister un logiciel pour coder et un autre pour concevoir les interfaces.

**Tout doit être réalisable depuis une seule application, dans un seul projet et un seul environnement de travail.**

L'IDE doit permettre de passer librement :
- Du code à la conception visuelle.
- De la conception visuelle au code.
- Des propriétés d'un composant à sa déclaration.
- D'un événement UI à sa fonction.
- D'une variable à ses références.
- D'un aperçu à l'élément qui le produit.

L'objectif est une synchronisation bidirectionnelle entre le code et le concepteur visuel.

## 14.2. Éditeur de code

Fonctionnalités attendues :
- Coloration syntaxique.
- Autocomplétion intelligente.
- Suggestions de types.
- Affichage des scopes d'origine.
- Import automatique.
- Navigation entre déclarations et utilisations.
- Renommage intelligent.
- Diagnostics en temps réel.
- Documentation contextuelle.
- Formatage automatique.
- Recherche dans le projet.
- Gestion des fichiers et modules.

L'éditeur doit être conçu pour le développement humain et assisté par IA.

## 14.3. Concepteur visuel d'interfaces

Le concepteur doit permettre de construire les interfaces directement à la souris.

Fonctionnalités :
- Zone de conception.
- Placement libre des composants.
- Déplacement et redimensionnement.
- Gestion des calques.
- Gestion des groupes.
- Gestion des conteneurs.
- Alignement et distribution.
- Édition des dimensions.
- Édition des couleurs.
- Gestion des marges et espacements.
- Gestion des bordures.
- Gestion des effets et animations.
- Gestion des événements.
- Prévisualisation responsive.

Les propriétés doivent pouvoir être modifiées aussi bien visuellement que depuis le code.

## 14.4. Synchronisation code / visuel

Un changement dans le code doit actualiser la représentation visuelle lorsqu'il est possible de le faire sans ambiguïté.

Un changement dans le concepteur visuel doit modifier les fichiers `.uimcf` correspondants.

L'IDE ne doit pas générer deux sources de vérité indépendantes.

Le projet doit disposer d'une représentation structurée commune, permettant de synchroniser le code, les fichiers d'interface et la prévisualisation.

## 14.5. Prévisualisation intégrée

L'IDE doit intégrer un aperçu exécutable de l'application.

Fonctionnalités :
- Actualisation automatique.
- Rechargement rapide.
- Simulation de différentes tailles d'écran.
- Inspection des composants.
- Inspection des propriétés.
- Affichage des erreurs.
- Débogage des interactions.

Le mode développement doit privilégier une boucle de modification et de prévisualisation rapide.

## 14.6. Gestion de projet

L'IDE doit intégrer :
- Explorateur de fichiers.
- Gestion des modules.
- Gestion des dépendances.
- Configuration du projet.
- Gestion des ressources.
- Gestion des interfaces.
- Gestion des paramètres de compilation.
- Terminal intégré.
- Console de diagnostics.
- Débogueur.

Aucun outil externe ne doit être nécessaire pour les opérations courantes de développement MCF.

# 15. Export et cibles de compilation

MCF doit pouvoir compiler vers plusieurs cibles.

Cibles envisagées :

| Cible | Résultat |
|---|---|
| Web | HTML, CSS, JavaScript |
| Web typé | TypeScript |
| WebAssembly | WASM |
| Bureau | Application native |
| Scripting | Runtime MCF |

L'export web doit produire un résultat autonome, propre et exploitable.

Le développeur doit pouvoir distribuer le résultat sans nécessiter l'installation de l'IDE MCF.

# 16. Interaction avec le système d'exploitation

MCF doit proposer une bibliothèque standard pour accéder aux fonctionnalités du système.

Espaces d'API envisagés :

```rust
OS.File
OS.Process
OS.System
OS.Network
OS.Memory
OS.Window
OS.Input
OS.Environment
```

Les fonctionnalités devront être adaptées à la cible de compilation.

L'accès aux fonctions sensibles devra être contrôlé par un système de permissions explicites.

`OS.Network` ne propose **aucune API de communication en clair par défaut** : toute connexion est chiffrée (voir §26). Une connexion non chiffrée exige une permission explicite `network.plaintext` déclarée dans le `.mcfproj`, produit un avertissement à chaque compilation et est refusée en mode `--strict`.

# 17. Gestion mémoire et performances

L'architecture doit s'inspirer des principes de Rust.

Objectifs :
- Limiter les allocations inutiles.
- Éviter les fuites mémoire.
- Contrôler les références.
- Prévenir les accès invalides.
- Garantir la cohérence des données.
- Optimiser les opérations fréquentes.

Le système d'ownership et d'emprunt doit être étudié pour fournir une sécurité mémoire forte sans rendre le langage inutilement difficile à utiliser.

# 18. Compilateur et architecture technique

Le compilateur sera développé en Rust.

Architecture envisagée :

1. Lexer : analyse lexicale.
2. Parser : construction de l'AST.
3. Analyse sémantique.
4. Résolution des symboles et des scopes.
5. Vérification des types.
6. Analyse DualType et type narrowing.
7. Vérification des règles de mémoire.
8. Construction d'une représentation intermédiaire MIR.
9. Optimisations.
10. Backends de compilation.

Le compilateur doit être indépendant de l'IDE afin de pouvoir être utilisé par la CLI, les outils automatisés et les agents IA.

## 18.1. Architecture du dépôt

```text
mcf/
├── compiler/
│   ├── lexer/
│   ├── parser/
│   ├── ast/
│   ├── semantic/
│   ├── types/
│   ├── ownership/
│   ├── mir/
│   └── optimizer/
│
├── backends/
│   ├── web/
│   ├── wasm/
│   └── native/
│
├── runtime/
├── standard-library/
├── language-server/
├── cli/
├── editor/
│   ├── code-editor/
│   ├── ui-designer/
│   ├── preview/
│   └── project-manager/
│
├── tests/
├── examples/
└── documentation/
```

## 18.2. Technologies proposées

| Élément | Technologie |
|---|---|
| Compilateur | Rust |
| Runtime | Rust |
| CLI | Rust |
| IDE | Tauri 2 |
| Interface IDE | Vue 3 |
| Langage de l'IDE | TypeScript |
| Parser | À choisir selon les besoins du langage |
| Éditeur de code | Composant d'édition intégré à l'IDE |

L'IDE et le compilateur communiqueront via une interface stable, permettant notamment d'obtenir les diagnostics, les symboles, les suggestions et les informations de compilation.

# 19. Diagnostics et développement assisté par IA

MCF doit être particulièrement fiable pour les outils d'assistance au développement.

Les diagnostics doivent contenir :
- Un identifiant stable.
- Le fichier concerné.
- La ligne et la colonne.
- La position exacte dans le code.
- La nature de l'erreur.
- Le type attendu.
- Le type reçu.
- Une explication.
- Une suggestion de correction lorsqu'elle est pertinente.

Une sortie JSON doit être disponible pour les outils automatisés.

Le compilateur doit permettre de vérifier un programme sans l'exécuter.

L'IDE doit proposer un Language Server Protocol (LSP) ou une interface équivalente pour :
- L'autocomplétion.
- Les diagnostics.
- La navigation.
- Les références.
- Le renommage.
- La documentation.
- L'analyse de code.

L'IA doit pouvoir interroger la structure du projet, les types, les fonctions, les modules et les erreurs sans devoir analyser manuellement tous les fichiers.

# 20. Interface en ligne de commande

La CLI devra proposer les commandes suivantes :

| Commande | Fonction |
|---|---|
| `mcf new` | Créer un projet |
| `mcf run` | Exécuter un projet |
| `mcf build` | Compiler |
| `mcf check` | Vérifier le code |
| `mcf watch` | Surveiller les changements |
| `mcf clean` | Nettoyer les fichiers générés |
| `mcf info` | Informations du projet |
| `mcf version` | Version du compilateur |
| `mcf fmt` | Formater le code |
| `mcf doc` | Générer la documentation |

La CLI doit fonctionner indépendamment de l'IDE.

# 21. Grammaire formelle – première base

La grammaire définitive devra être exprimée dans une notation formelle exploitable par le parser.

Base actuelle :

```text
Program =
    { ModuleDeclaration } ;

ModuleDeclaration =
    GlobalDeclaration
  | FunctionDeclaration
  | ScopeDeclaration
  | ImportDeclaration
  | EventDeclaration ;

GlobalDeclaration =
    Identifier ":" TypeDeclaration
    [ "=" Expression ] ";" ;

LocalDeclaration =
    "_" Identifier ":" TypeDeclaration
    [ "=" Expression ] ";" ;

TypeDeclaration =
    TypeName [ "," TypeName ] ;

FunctionDeclaration =
    "func" Identifier "(" [ Parameters ] ")"
    [ ":" TypeName ] Block ;

Parameters =
    Parameter { "," Parameter } ;

Parameter =
    "_" Identifier ":" TypeDeclaration ;

ScopeDeclaration =
    "scope" Identifier Block ;

ImportDeclaration =
    "import" QualifiedIdentifier ";" ;

Block =
    "{" { Statement } "}" ;

Statement =
    LocalDeclaration
  | Assignment
  | IfStatement
  | WhileStatement
  | ForStatement
  | ForeachStatement
  | ReturnStatement
  | ExpressionStatement ;

IfStatement =
    "if" "(" Expression ")" "then" Block
    [ "else" Block ] ;

WhileStatement =
    "while" "(" Expression ")" "do" Block ;

ForStatement =
    "for" "(" LocalDeclaration
    Expression ";" Expression ")" "do" Block ;

ForeachStatement =
    "foreach" "(" LocalDeclaration "in" Expression ")"
    "do" Block ;

Assignment =
    VariableReference "=" Expression ";" ;

ReturnStatement =
    "return" [ Expression ] ";" ;

ExpressionStatement =
    Expression ";" ;
```

Cette base est une ébauche : les productions d'expressions, les priorités d'opérateurs, les déclarations de structures, les événements et les interfaces devront être ajoutés avant de considérer la grammaire comme complète.

# 22. Plan de développement

Le développement doit être progressif et testable.

| Phase | Objectif |
|---|---|
| 1 | Grammaire officielle et spécifications |
| 2 | Lexer, parser et AST |
| 3 | Types, variables et scopes |
| 4 | DualType et analyse sémantique |
| 5 | Fonctions et structures |
| 6 | Conditions et boucles |
| 7 | Imports et résolution des symboles |
| 8 | Événements et asynchrone |
| 9 | Configuration `.cfg` |
| 10 | Interfaces `.uimcf` |
| 11 | Backend web |
| 12 | Runtime et backend natif |
| 13 | Language Server et outils IA |
| 14 | IDE unifié |
| 15 | Tests, optimisation et stabilisation |

Les étapes peuvent se chevaucher lorsque les interfaces techniques sont stabilisées, mais aucune fonctionnalité ne doit être considérée comme terminée sans tests.

# 23. Tests et qualité

Chaque fonctionnalité doit disposer de tests.

Le projet doit notamment vérifier :
- Les déclarations locales et globales.
- Les règles de visibilité.
- Les scopes imbriqués.
- Les variables homonymes.
- Les imports.
- Les collisions.
- Le typage strict.
- DualType.
- Le type narrowing.
- Les fonctions.
- Les boucles.
- Les événements.
- Les diagnostics.
- La compilation web.
- La synchronisation entre code et concepteur visuel.

Des exemples valides et invalides devront accompagner chaque règle de grammaire.

Les tests du parser doivent garantir que le même code produit toujours la même interprétation.

# 24. Décisions restant à formaliser

Les principes généraux sont établis, mais certains détails nécessitent encore une décision avant de figer la grammaire et de lancer le développement complet.

1. Priorité et associativité de tous les opérateurs.
2. Déclaration des tableaux et dictionnaires typés.
3. Structures, classes, interfaces et énumérations.
4. Alias et collisions des imports.
5. Syntaxe définitive des événements et métadonnées.
6. Syntaxe complète des fichiers `.uimcf`.
7. Gestion des erreurs et exceptions.
8. Syntaxe asynchrone.
9. Système de modules et dépendances.
10. Règles exactes d'ownership et d'emprunt.
11. Syntaxe des génériques et fonctions surchargées.
12. Règles de synchronisation du concepteur visuel avec le code.
13. Syntaxe et règles de validation des fichiers `.cfg`.
14. API exacte du module `Secure`, format des clés, procédure de sauvegarde/restauration des clés (voir §26).

Ces points devront être documentés explicitement. Aucune décision structurante ne doit être prise silencieusement pendant l'implémentation.

# 25. Principes directeurs de MCF Studio

MCF doit rester :
- Lisible.
- Strict.
- Prévisible.
- Performant.
- Extensible.
- Facile à analyser par une IA.
- Agréable à utiliser dans l'IDE.
- Adapté aux projets de petite et grande taille.

Le langage, le compilateur et l'IDE doivent être conçus comme les éléments d'un seul écosystème.

**L'IDE MCF est le point d'entrée central : le développeur doit pouvoir écrire le code, construire ses interfaces, gérer son projet, tester et compiler sans devoir jongler entre plusieurs logiciels ou outils indépendants.**

# 26. Chiffrement de bout en bout et souveraineté des clés

> **Engagement fondamental : MCF Studio ne détient aucune clé, n'exploite aucun serveur obligatoire et ne collecte aucune télémétrie. MCF Studio n'a donc strictement aucun moyen technique de lire ou de surveiller les échanges réalisés avec son propre langage.**

Le détail (modèle de permissions, menaces, primitives, limites) est dans [`docs/securite/chiffrement-et-cles.md`](../securite/chiffrement-et-cles.md). Ce chapitre en fixe les règles normatives.

## 26.1. Règles

1. **Rien ne transite en clair.** Toute donnée qui quitte un processus MCF (réseau, événements distants, IPC IDE ↔ compilateur, registre de paquets, mises à jour) est chiffrée et authentifiée.
2. **Clés propres à l'utilisateur.** Les clés sont générées localement, sur l'appareil de l'utilisateur, et ne le quittent jamais en clair. Aucune clé maîtresse, aucun séquestre (*key escrow*), aucune porte dérobée.
3. **Zéro télémétrie.** Ni le compilateur, ni la CLI, ni l'IDE, ni le runtime n'envoient de données à MCF Studio.
4. **Chiffré par défaut, le clair est l'exception.** Le chemin le plus simple dans le langage est le chemin chiffré. Le clair exige une permission explicite, visible et refusée en mode strict.
5. **Pas de cryptographie maison.** Seules des primitives et bibliothèques reconnues et auditées sont utilisées.
6. **Les secrets sont un type.** Le type `Secret<T>` empêche, à la compilation, l'affichage, la journalisation ou la sérialisation en clair d'une valeur sensible.
7. **Vérifiable.** Le code source de la chaîne d'outils est public ; l'objectif est des compilations reproductibles pour que chacun puisse vérifier qu'aucun binaire officiel ne contient de mécanisme de surveillance.

## 26.2. Conséquence assumée

Si un utilisateur perd sa clé et sa sauvegarde, **MCF Studio ne peut pas récupérer ses données**. C'est la contrepartie directe de l'engagement ci-dessus et cela doit être expliqué clairement à l'utilisateur.

## 26.3. Diagnostics de sécurité

Une catégorie de diagnostics `security` est ajoutée (identifiants `MCF03xx`) : exposition d'un `Secret`, connexion en clair, permission manquante, etc.

---

**Fin du cahier des charges – Édition 1.2**

**Prochaine étape recommandée :** rédiger la grammaire formelle complète de MCF, avec toutes les productions, les priorités d'opérateurs, les règles de résolution des scopes et les tests de conformité nécessaires à l'implémentation du parser Rust.