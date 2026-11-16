> **Document historique.** Cette édition 1.0 est remplacée par l'[édition 1.2](edition-1.2.md) (syntaxe sans `set`, `func`, préfixe `_` pour les locales, chiffrement de bout en bout). En cas de contradiction, l'édition 1.2 fait foi.

# MCF – MyLangage
## Cahier des charges technique et fonctionnel

**Projet :** MCF – MyLangage  
**Éditeur :** MCF Studio  
**Version du document :** 1.0 – Spécifications initiales  
**Statut :** Conception  
**Extensions :** `.mcf` / `.uimcf` / `.cfg`  
**Technologies principales :** Rust, Tauri 2, Vue 3

---

# 1. Présentation du projet

## 1.1. Objectif

MCF (MyLangage) est un langage de programmation conçu pour permettre le développement de scripts, d'applications et particulièrement d'interfaces utilisateur avancées.

Le projet vise à créer un langage propriétaire disposant :
- D'une syntaxe identifiable et cohérente.
- D'un compilateur strict.
- D'un système de typage statique.
- D'un système de types doubles permettant une flexibilité contrôlée.
- D'une gestion optimisée de la mémoire.
- D'un système d'événements fortement typés.
- D'une gestion avancée des scopes.
- D'une intégration avec les fonctionnalités du système d'exploitation.
- D'un système de création d'interfaces basé sur les technologies web.
- D'un compilateur capable de produire des applications web autonomes.

MCF doit également être pensé pour être utilisé par des agents IA. Le langage doit donc limiter les ambiguïtés, faciliter la génération de code et fournir des diagnostics extrêmement précis.

## 1.2. Philosophie

MCF s'inspire notamment de :
- C# pour sa syntaxe structurée et sa lisibilité.
- Rust pour son système de types, sa rigueur et sa gestion mémoire.
- HTML et CSS pour la liberté de création des interfaces.
- TypeScript pour les concepts de typage et d'intégration web.

MCF ne doit pas être une copie de ces langages. Il doit posséder ses propres mécanismes et sa propre identité.

### Principes fondamentaux

1. Explicite plutôt qu'implicite.
2. Strict sans être inutilement contraignant.
3. Flexible lorsque cette flexibilité est encadrée par des règles.
4. Optimisé pour la performance et la mémoire.
5. Accessible aux humains et aux IA.
6. Compatible avec les technologies web existantes.
7. Extensible sans compromettre la stabilité du langage.

---

# 2. Les extensions du langage

MCF possède trois extensions officielles.

## 2.1. `.mcf` – Langage principal

Extension destinée au code généraliste.

Elle permet notamment :
- Déclaration de variables.
- Fonctions et procédures.
- Structures et types personnalisés.
- Conditions.
- Boucles.
- Collections.
- Gestion des événements.
- Programmation asynchrone.
- Gestion des erreurs.
- Accès aux bibliothèques.
- Communication avec le système d'exploitation.

Exemple :

```rust
scope App.Main {

    set name: String = "MyIA";
    set active: Bool = true;

    action welcome(user: String) -> Void {
        print("Bienvenue " + user);
    };

    if (active) then {
        welcome(name);
    } else {
        print("Application inactive");
    };

};
```

## 2.2. `.uimcf` – Langage d'interface

Extension destinée à la création d'interfaces utilisateur.

Elle doit permettre :
- La création de composants.
- La définition de structures HTML.
- L'utilisation de CSS natif.
- Les propriétés dynamiques.
- Les états réactifs.
- Les événements.
- Les animations.
- Les interactions entre composants.
- La communication avec les fichiers `.mcf`.
- La création de composants réutilisables.

L'utilisateur doit pouvoir conserver une liberté de conception comparable à celle d'un développeur HTML/CSS classique.

Exemple conceptuel :

```html
ui Dashboard {

    state count: Int = 0;

    view {
        <div class="dashboard">
            <h1>MyIA Dashboard</h1>

            <button on:click={increment}>
                Ajouter
            </button>

            <span>{count}</span>
        </div>
    }

    style {
        .dashboard {
            min-height: 100vh;
            background: #10151e;
            color: white;
            display: flex;
            align-items: center;
            justify-content: center;
        }
    }

    action increment() -> Void {
        set count = count + 1;
    };

};
```

Cet exemple définit une direction syntaxique. La grammaire exacte du mélange entre HTML, CSS et MCF devra être formalisée avant l'implémentation.

### Liberté CSS

Ne pas limiter le développeur à un système de composants prédéfinis.

Prendre en charge autant que possible :
- Flexbox.
- CSS Grid.
- Variables CSS.
- Animations et transitions.
- Media queries.
- Pseudo-classes et pseudo-éléments.
- SVG.
- Canvas.
- Responsive design.
- Polices personnalisées.
- Import de feuilles de style.
- Bibliothèques CSS compatibles.

Le développeur doit pouvoir écrire du CSS natif sans devoir passer par une API propriétaire.

## 2.3. `.cfg` – Configuration

Extension dédiée aux configurations.

Objectifs :
- Compatibilité avec JSON.
- Lecture des fichiers JSON existants.
- Validation par schéma.
- Typage des configurations.
- Possibilité d'utiliser les configurations directement depuis `.mcf`.
- Sérialisation et désérialisation.

Tout fichier JSON valide doit pouvoir être chargé comme fichier `.cfg`, sans nécessiter de transformation manuelle.

Exemple :

```json
{
    "app": {
        "name": "MyIA",
        "version": 1.0,
        "enabled": true
    },
    "interface": {
        "theme": "dark",
        "accent": "#00aaff"
    }
}
```

Une structure de validation pourra être définie dans le langage :

```rust
config AppConfig {
    name: String;
    version: Float;
    enabled: Bool;
};
```

Le compilateur devra vérifier la compatibilité entre la configuration et le schéma déclaré.

---

# 3. Architecture du compilateur

## 3.1. Technologies

Le compilateur et les composants fondamentaux du langage seront développés en Rust.

L'objectif est de profiter :
- De la sécurité mémoire de Rust.
- De ses performances.
- De son écosystème.
- De son système de types.
- De sa capacité à produire des exécutables natifs.

## 3.2. Pipeline de compilation

Le compilateur devra posséder les étapes suivantes :

1. Chargement des fichiers sources.
2. Analyse lexicale (Lexer).
3. Analyse syntaxique (Parser).
4. Construction de l'AST.
5. Analyse sémantique.
6. Résolution des symboles et des scopes.
7. Vérification des types.
8. Vérification des règles d'ownership.
9. Vérification des événements et des interfaces.
10. Génération d'une représentation intermédiaire (MIR).
11. Optimisation.
12. Génération du code cible.

## 3.3. Représentation intermédiaire

Créer une représentation intermédiaire commune afin de ne pas dépendre d'un backend spécifique.

Cette représentation permettra de développer plusieurs cibles de compilation sans dupliquer les analyses du langage.

## 3.4. Backends

Prévoir les cibles suivantes :

| Backend | Objectif |
|---|---|
| HTML | Structure des interfaces |
| CSS | Styles des interfaces |
| JavaScript | Logique d'exécution web |
| TypeScript | Logique web typée |
| WebAssembly | Calculs et modules optimisés |
| Native | Exécution native, à développer progressivement |

La sortie web doit être autonome et ne pas exiger l'installation du runtime MCF chez l'utilisateur final.

Le backend natif pourra être développé progressivement après stabilisation du langage et de son modèle mémoire.

---

# 4. Système de types

## 4.1. Typage statique

MCF utilise un système de typage statique.

Chaque variable, paramètre, propriété et valeur de retour doit posséder un type connu du compilateur.

Types fondamentaux prévus :

| Type | Description |
|---|---|
| `Bool` | Booléen |
| `Int` | Entier signé |
| `UInt` | Entier non signé |
| `Float` | Nombre décimal |
| `String` | Chaîne de caractères |
| `Char` | Caractère |
| `Bytes` | Données binaires |
| `Void` | Absence de valeur |
| `DateTime` | Date et heure |

Les tailles exactes des types numériques devront être définies dans la spécification technique.

Types composés :
- `List<T>`
- `Map<K, V>`
- `Array<T>`
- `Set<T>`
- Structures personnalisées.
- Énumérations.
- Types optionnels.
- Types de résultat.

## 4.2. Déclaration des variables

Syntaxe :

```rust
set name: String = "MCF";
set age: Int = 25;
set active: Bool = true;
```

Règles :
- Le type est obligatoire dans la première version.
- Une variable ne peut pas être utilisée avant initialisation.
- Une variable est mutable par défaut avec `set`.
- Une déclaration immuable pourra être proposée avec `let`.
- Les affectations doivent respecter le type déclaré.

## 4.3. Système DualType

Fonctionnalité fondamentale de MCF.

Une variable peut accepter un ou deux types différents.

Exemple :

```rust
set simple: Bool = true;

set dynamic: Bool, Int = true;

dynamic = 42;
dynamic = false;
```

Le type double constitue une union de deux types autorisés.

### Règles

1. Maximum deux types distincts.
2. Une seule valeur active à la fois.
3. Le type actif est conservé avec la valeur.
4. Une affectation vers l'un des deux types est autorisée.
5. Une affectation vers un troisième type est interdite.
6. Aucune conversion implicite générale entre les types.
7. Le compilateur connaît les deux types possibles.
8. Les opérations doivent respecter les types actifs.

Exemple invalide :

```rust
set value: Bool, Int = true;

value = "Hello";
```

Le compilateur doit signaler une erreur.

### Type Narrowing

Le développeur doit pouvoir vérifier le type actif.

```rust
set value: Int, String = 42;

if (value is Int) then {
    set result: Int = value + 10;
};

if (value is String) then {
    print(value);
};
```

Dans chaque branche, le compilateur doit connaître le type actif et autoriser uniquement les opérations compatibles.

### Généralisation

Le système DualType doit fonctionner avec :
- Variables.
- Paramètres de fonctions.
- Valeurs de retour.
- Propriétés UI.
- Données d'événements.
- Métadonnées.

### Implémentation

Utiliser une représentation discriminée de type somme, équivalente conceptuellement à une Tagged Union.

Éviter de transformer toutes les valeurs en objets dynamiques génériques.

Le stockage doit être adapté aux tailles et alignements des types concernés.

---

# 5. Fonctions et structures

## 5.1. Fonctions

Syntaxe officielle envisagée :

```rust
action calculate(a: Int, b: Int) -> Int {
    return a + b;
};
```

Règles :
- Nom obligatoire.
- Paramètres typés.
- Type de retour obligatoire.
- Vérification du nombre d'arguments.
- Vérification des types.
- Vérification des retours.
- Support des fonctions privées et publiques.

## 5.2. Structures

Prévoir des structures personnalisées.

```rust
struct User {
    id: Int;
    name: String;
    active: Bool;
};
```

Création :

```rust
set user: User = User {
    id: 1,
    name: "Alex",
    active: true
};
```

Prévoir également les énumérations et les méthodes associées aux structures.

---

# 6. Système de contrôle

La première version doit disposer des mécanismes classiques d'un langage généraliste.

## 6.1. Conditions

```rust
if (condition) then {
    print("Vrai");
} else {
    print("Faux");
};
```

Prévoir :
- `if`
- `else`
- `else if`
- `switch` ou équivalent.

## 6.2. Boucles

Boucle classique :

```rust
for (i: Int = 0; i < 10; i++) {
    print(i);
};
```

Boucle foreach :

```rust
foreach (item: String in items) {
    print(item);
};
```

Boucle while :

```rust
while (active) {
    print("Actif");
};
```

Prévoir :
- `break`
- `continue`
- Boucles imbriquées.
- Vérification des types des itérateurs.

## 6.3. Temporisation

Prévoir `sleep` :

```rust
sleep(1000);
```

La valeur devra être définie en millisecondes dans la première version.

Prévoir une variante asynchrone compatible avec le système de tâches.

## 6.4. Opérateurs

Prévoir :
- Opérateurs arithmétiques.
- Comparaisons.
- Opérateurs logiques.
- Affectations composées.
- Incrémentation et décrémentation.
- Opérateurs de nullabilité.
- Opérateurs de coalescence, si retenus.

La priorité et l'associativité de chaque opérateur devront être documentées.

---

# 7. Système de scopes

Les scopes doivent être nommables, accessibles et manipulables.

Exemple :

```rust
scope App {

    scope User {
        set name: String = "Alex";
        set id: Int = 42;
    };

    scope Settings {
        set theme: String = "dark";
    };

};
```

Accès :

```rust
print(App.User.name);

set App.Settings.theme = "light";
```

Prévoir plusieurs catégories :

- Namespace Scope : organisation du code.
- State Scope : gestion des données.
- Component Scope : données propres aux composants UI.
- Lifetime Scope : durée de vie des ressources.

Les règles d'accès, de visibilité et de durée de vie devront être définies précisément.

Les scopes ne doivent pas tous être assimilés à des allocations mémoire indépendantes : leur implémentation devra rester optimisée.

---

# 8. Système événementiel

Le système événementiel doit être natif au langage.

Il doit permettre :
- De déclarer un événement.
- D'écouter un événement.
- De déclencher un événement.
- De transmettre des données.
- De gérer les événements locaux et globaux.
- De gérer plusieurs écouteurs.
- De gérer les erreurs des callbacks.
- De retirer les écouteurs.
- De contrôler leur durée de vie.

## 8.1. Déclaration

```rust
event User.Connected {
    user_id: Int;
    username: String;
    metadata: Map<String, MetaValue>;
};
```

## 8.2. Écoute

```rust
on User.Connected (event) {
    print(event.username);
};
```

## 8.3. Déclenchement

```rust
emit User.Connected {
    user_id: 42,
    username: "Alex",
    metadata: {
        source: "web",
        verified: true,
        level: 12
    }
};
```

## 8.4. Métadonnées

Créer un type `MetaValue` permettant de transporter plusieurs catégories de données :

- String.
- Int.
- Float.
- Bool.
- DateTime.
- Bytes.
- JSON.
- Structures sérialisables.

Les métadonnées doivent conserver leur type réel.

Pour les événements déclarés avec un schéma, le compilateur doit vérifier les propriétés, les valeurs et les types.

Les événements personnalisés non déclarés doivent pouvoir être refusés en mode strict.

---

# 9. Interface utilisateur et réactivité

## 9.1. Principes

`.uimcf` doit permettre de développer des interfaces web sans sacrifier la liberté HTML/CSS.

Il ne doit pas imposer un framework visuel propriétaire.

## 9.2. Composants

Prévoir :
- Composants locaux.
- Composants réutilisables.
- Composants importés.
- Propriétés typées.
- États internes.
- Communication parent/enfant.
- Slots ou mécanisme équivalent.

## 9.3. Réactivité

Une modification d'un état doit provoquer uniquement les mises à jour nécessaires.

Éviter de reconstruire inutilement toute l'interface.

Prévoir :
- États locaux.
- États partagés.
- Binding de données.
- Propriétés réactives.
- Calculs dérivés.
- Cycle de vie des composants.

## 9.4. Événements UI

Les composants doivent pouvoir utiliser directement le système événementiel de MCF.

Exemple :

```html
<button on:click={increment}>
    Ajouter
</button>
```

Prévoir les événements standards :
- Click.
- Input.
- Change.
- Submit.
- Focus.
- Blur.
- Keydown.
- Keyup.
- Mouseover.
- Mouseout.
- Événements personnalisés.

## 9.5. Export

Le compilateur doit générer :

```text
dist/
    index.html
    assets/
        styles.css
        app.js
        modules/
```

Options de sortie :
- JavaScript.
- TypeScript.
- CSS.
- HTML.
- WebAssembly pour les modules compatibles.

Les fichiers produits doivent pouvoir être utilisés indépendamment du compilateur MCF.

---

# 10. Gestion mémoire et performances

La performance est un objectif transversal.

## 10.1. Objectifs

- Limiter les allocations.
- Éviter les copies inutiles.
- Favoriser les références lorsque leur durée de vie est sûre.
- Contrôler les ressources natives.
- Libérer les ressources de manière déterministe.
- Optimiser les collections.
- Éviter les abstractions coûteuses sans nécessité.

## 10.2. Ownership

S'inspirer du modèle Rust.

Prévoir :
- Propriétaire unique par défaut.
- Transfert explicite de propriété.
- Références immuables.
- Références mutables contrôlées.
- Vérification des durées de vie.
- Destruction déterministe des ressources.

La première version pourra utiliser un modèle simplifié, mais l'architecture devra permettre son évolution.

## 10.3. Optimisations

Prévoir :
- Constant folding.
- Dead code elimination.
- Inlining contrôlé.
- Optimisation des allocations.
- Réduction des copies.
- Optimisation des structures de données.
- Optimisation des bundles web.

Les optimisations ne doivent jamais modifier le comportement observable du programme.

---

# 11. Intégration au système d'exploitation

Créer une bibliothèque standard `OS`.

Modules envisagés :

| Module | Fonction |
|---|---|
| `OS.File` | Fichiers et dossiers |
| `OS.Process` | Processus |
| `OS.System` | Informations système |
| `OS.Network` | Réseau |
| `OS.Memory` | Informations et ressources mémoire |
| `OS.Window` | Fenêtres |
| `OS.Input` | Clavier et souris |
| `OS.Environment` | Variables d'environnement |

Prévoir des permissions explicites.

Une application devra déclarer les capacités OS auxquelles elle souhaite accéder.

La disponibilité des modules devra dépendre de la cible de compilation.

Un projet web ne doit pas obtenir automatiquement les permissions d'un programme natif.

---

# 12. Gestion des erreurs

Le compilateur doit produire des erreurs précises et compréhensibles.

Chaque erreur devra posséder :
- Un identifiant stable.
- Un fichier.
- Une ligne.
- Une colonne.
- Une catégorie.
- Une description.
- Le contexte de l'erreur.
- Une suggestion de correction lorsque possible.

Exemple :

```text
MCF0012: TYPE_MISMATCH

Expected: Int
Received: String

File: main.mcf
Line: 4
Column: 17

Suggestion:
Use an explicit conversion if appropriate.
```

Prévoir des catégories distinctes :
- Erreurs lexicales.
- Erreurs syntaxiques.
- Erreurs sémantiques.
- Erreurs de types.
- Erreurs d'ownership.
- Erreurs d'import.
- Erreurs de compilation UI.
- Erreurs de configuration.

Les diagnostics doivent être disponibles en texte lisible et en JSON structuré.

---

# 13. MCF AI-Ready

La compatibilité avec les agents IA constitue un objectif prioritaire.

Le langage doit permettre à une IA de produire du code, de le vérifier et de comprendre précisément les erreurs.

## 13.1. Diagnostics structurés

Fournir un format JSON stable contenant :
- Identifiant de l'erreur.
- Type de diagnostic.
- Fichier.
- Position.
- Type attendu.
- Type reçu.
- Élément concerné.
- Suggestion de correction.

## 13.2. Vérification sans exécution

Prévoir une commande permettant de vérifier intégralement un projet sans l'exécuter.

Exemple :

```bash
mcf check ./MyProject
```

Cette commande devra vérifier les fichiers sources, imports, types, événements, configurations et interfaces.

## 13.3. Autodescription

Prévoir des commandes permettant de découvrir :
- Les mots-clés disponibles.
- Les types natifs.
- Les fonctions natives.
- Les modules disponibles.
- Les schémas d'événements.
- Les API accessibles.
- Les erreurs possibles.

## 13.4. Format de sortie

Prévoir plusieurs niveaux de diagnostic :

```bash
mcf check ./MyProject
mcf check ./MyProject --format json
mcf check ./MyProject --strict
```

Le mode strict ne devra pas permettre de masquer silencieusement des erreurs ou des avertissements importants.

---

# 14. CLI et commandes

Créer une interface en ligne de commande `mcf`.

Commandes prévues :

| Commande | Fonction |
|---|---|
| `mcf new` | Créer un projet |
| `mcf run` | Exécuter un projet en développement |
| `mcf build` | Compiler |
| `mcf check` | Vérifier |
| `mcf watch` | Surveiller les modifications |
| `mcf clean` | Nettoyer les fichiers générés |
| `mcf info` | Informations sur le compilateur |
| `mcf version` | Version |
| `mcf fmt` | Formater le code |
| `mcf doc` | Générer la documentation |

La compilation sera obligatoire pour les livrables de production.

Un mode développement devra permettre la recompilation automatique et l'actualisation de l'aperçu sans redémarrage manuel systématique.

---

# 15. Éditeur graphique MCF

## 15.1. Technologies

- Rust.
- Tauri 2.
- Vue 3.
- Éditeur de code compatible avec les besoins du langage.
- Language Server Protocol.

## 15.2. Fonctionnalités

### Gestion de projet
- Création de projets.
- Ouverture de projets.
- Gestion des fichiers.
- Explorateur de fichiers.
- Importation de bibliothèques.

### Éditeur
- Coloration syntaxique.
- Autocomplétion.
- Diagnostics en direct.
- Navigation.
- Renommage.
- Formatage.
- Documentation contextuelle.

### UI Designer
- Aperçu instantané.
- Prévisualisation responsive.
- Inspection des composants.
- Édition des propriétés.
- Gestion des événements.
- Navigation dans la hiérarchie.
- Possibilité de modifier le code et de voir le résultat.

### Console
- Sorties `print`.
- Erreurs.
- Diagnostics.
- Logs.
- Informations de compilation.

Le Designer ne doit jamais imposer une interface limitée aux composants visuels proposés par l'éditeur.

Le code doit rester la source de vérité.

---

# 16. Architecture du dépôt

Organisation initiale recommandée :

```text
MCF/
│
├── compiler/
│   ├── lexer/
│   ├── parser/
│   ├── ast/
│   ├── semantic/
│   ├── types/
│   ├── ownership/
│   ├── mir/
│   ├── optimizer/
│   └── diagnostics/
│
├── backends/
│   ├── web/
│   ├── javascript/
│   ├── typescript/
│   ├── wasm/
│   └── native/
│
├── runtime/
│   ├── memory/
│   ├── events/
│   ├── collections/
│   └── os/
│
├── standard-library/
│   ├── core/
│   ├── io/
│   ├── filesystem/
│   ├── network/
│   └── ui/
│
├── language-server/
│
├── cli/
│
├── editor/
│   ├── src-tauri/
│   └── src/
│
├── examples/
│   ├── hello-world/
│   ├── variables/
│   ├── events/
│   ├── configs/
│   └── dashboard/
│
├── tests/
│   ├── lexer/
│   ├── parser/
│   ├── types/
│   ├── dual-types/
│   ├── events/
│   ├── ui/
│   └── integration/
│
└── documentation/
    ├── language/
    ├── syntax/
    ├── types/
    ├── compiler/
    └── api/
```

Cette organisation est une proposition initiale. Les composants Rust pourront être regroupés en crates indépendantes selon les dépendances réelles.

---

# 17. Phases de développement

Le développement doit suivre un ordre strict afin de ne pas construire les fonctionnalités sur une base instable.

## Phase 1 – Fondations

- Définition de la grammaire.
- Définition des tokens.
- Lexer.
- Parser.
- AST.
- Messages d'erreurs.
- Premier exécutable CLI.

**Livrable :** MCF peut analyser des fichiers et détecter les erreurs lexicales et syntaxiques.

## Phase 2 – Langage de base

- Variables.
- Types.
- DualType.
- Opérateurs.
- Conditions.
- Boucles.
- Fonctions.
- Structures.
- Scopes.
- Collections.

**Livrable :** MCF peut compiler et exécuter des programmes simples.

## Phase 3 – Système strict

- Vérification sémantique.
- Type checking.
- Type narrowing.
- Vérification des appels.
- Résolution des symboles.
- Première version de l'ownership.
- Diagnostics JSON.

**Livrable :** compilateur capable de refuser les programmes invalides avant exécution.

## Phase 4 – Événements et asynchronisme

- Déclaration des événements.
- Écouteurs.
- Déclenchement.
- Métadonnées.
- Tâches.
- Temporisation.
- Gestion des durées de vie des écouteurs.

**Livrable :** programme événementiel fonctionnel.

## Phase 5 – Configuration

- Parser JSON.
- Lecture `.cfg`.
- Schémas.
- Validation.
- Sérialisation.
- Désérialisation.

**Livrable :** configurations intégrées au langage.

## Phase 6 – UI

- Parser `.uimcf`.
- Structure HTML.
- CSS natif.
- États.
- Bindings.
- Événements UI.
- Composants.
- Réactivité.

**Livrable :** première interface complète.

## Phase 7 – Export web

- Génération HTML.
- Génération CSS.
- Génération JavaScript.
- Génération TypeScript.
- Bundling.
- Optimisation.
- Aperçu local.

**Livrable :** projet web autonome.

## Phase 8 – Outils développeur

- Language Server.
- Formatage.
- Documentation.
- Éditeur graphique.
- UI Designer.
- Console.
- Autocomplétion.

**Livrable :** environnement de développement MCF.

## Phase 9 – Stabilisation

- Tests de conformité.
- Tests de performances.
- Tests mémoire.
- Tests de compatibilité web.
- Documentation complète.
- Exemples.
- Version expérimentale publique.

---

# 18. Tests et critères de validation

Chaque fonctionnalité doit disposer de tests automatisés.

Aucune fonctionnalité ne sera considérée comme terminée uniquement parce qu'un exemple fonctionne.

### Tests indispensables

- Code valide accepté.
- Code invalide refusé.
- Mauvais types détectés.
- DualType limité à deux types.
- Type narrowing correct.
- Scopes correctement isolés.
- Événements correctement typés.
- JSON valide accepté.
- JSON invalide correctement signalé.
- Interfaces générées correctement.
- Export web autonome.
- Ressources correctement libérées.
- Absence de régression.

### Tests IA

Prévoir des fichiers de test comprenant :
- Des programmes volontairement incorrects.
- Des erreurs de typage.
- Des erreurs de syntaxe.
- Des appels invalides.
- Des déclarations DualType incorrectes.
- Des erreurs d'événements.
- Des configurations incompatibles.

Le compilateur doit toujours produire des diagnostics exploitables.

---

# 19. Objectifs de la première version publique

La première version publique ne doit pas nécessairement posséder toutes les fonctionnalités prévues.

Elle devra toutefois démontrer que MCF est un langage réellement utilisable.

Objectifs minimums :

1. Compiler un programme `.mcf`.
2. Utiliser les variables et les types.
3. Utiliser DualType.
4. Utiliser les fonctions et les conditions.
5. Utiliser les boucles.
6. Manipuler les scopes.
7. Déclarer et écouter des événements.
8. Lire une configuration JSON.
9. Créer une interface `.uimcf`.
10. Exporter cette interface en HTML/CSS/JS.
11. Obtenir des erreurs précises.
12. Vérifier un projet complet avec `mcf check`.

---

# 20. Principes de développement à respecter

Ces règles s'appliquent à toute l'équipe de développement et aux agents IA intervenant sur le projet.

- Ne jamais inventer une règle de syntaxe sans la documenter.
- Ne jamais modifier une règle fondamentale sans mettre à jour la spécification.
- Ne jamais contourner le compilateur pour faire fonctionner un exemple.
- Ne jamais utiliser un type dynamique générique pour contourner DualType.
- Ne jamais introduire de conversion implicite non documentée.
- Ne jamais sacrifier la sécurité mémoire pour simplifier une fonctionnalité.
- Ne jamais limiter artificiellement les capacités du CSS.
- Ne jamais introduire une dépendance inutile.
- Chaque fonctionnalité doit avoir ses tests.
- Chaque erreur du compilateur doit posséder un identifiant stable.
- Chaque modification de grammaire doit être accompagnée de tests de conformité.

La documentation officielle constitue la référence du langage.

---

# Conclusion

MCF doit devenir un langage cohérent, performant et agréable à utiliser, avec une identité propre.

Sa particularité repose sur l'association de trois éléments :

1. Un langage généraliste strict avec une flexibilité contrôlée.
2. Un système de création d'interfaces offrant la liberté du web.
3. Un compilateur conçu pour être fiable, explicite et exploitable par des agents IA.

Le projet doit commencer par le langage lui-même. L'éditeur graphique viendra ensuite s'appuyer sur une base stable.

**Priorité absolue : définir la grammaire officielle, les règles de typage et l'architecture du compilateur avant d'attaquer les fonctionnalités avancées.**