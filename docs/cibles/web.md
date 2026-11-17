# Cible web (phase 11) — spécification

**Statut :** spécification à valider (D58 à D65 proposées). Rien n'est encore implémenté.

Cahier des charges 1.2 : §15 (export web autonome, distribuable sans l'IDE), §13 (affichage des `.uimcf`), §26 (rien ne transite en clair, aucune donnée vers MCF Studio). La phase 11 tient aussi l'engagement de D56 : **une fonction qui a des effets de bord est refusée dans une vue**.

## 1. Ce que la phase 11 livre

```bash
mcf build --target web mon_projet/          # → mon_projet/build/web/
mcf build --target web mon_projet/ --out site/
mcf run --web mon_projet/                   # construit, puis ouvre la page dans le navigateur
```

Le résultat est un **dossier statique autonome** : on l'ouvre directement (`file://`) ou on le dépose chez n'importe quel hébergeur HTTPS. Aucun serveur MCF, aucune ressource externe, aucune requête réseau.

```text
build/web/
├── index.html      page d'accueil, CSP en <meta>, scripts avec empreinte SRI
├── mcf.js          couche d'affichage (DOM) + runtime MCF en WebAssembly embarqué
├── app.js          programme vérifié, sérialisé (ni sources, ni commentaires)
├── assets/         images et .cfg réellement utilisés, et rien d'autre
└── _headers        en-têtes de sécurité à appliquer chez l'hébergeur (CSP, HSTS…)
```

`build` affiche la liste de **tous** les fichiers publiés, suivie de : « Tout ce qui est dans ce dossier est public : chaque visiteur peut le lire. »

## 2. Architecture (D58)

**Le même interpréteur qu'en `mcf run`, compilé en WebAssembly, et une couche JavaScript mince qui ne fait qu'appliquer des changements au DOM.**

```text
navigateur                                   │  WebAssembly (crate mcf-runtime)
                                             │
 clic / saisie ──► mcf.js ── événement ─────►│  ordonnanceur D42 → tâches MCF
                                             │        │
                                             │  vue recalculée (fonctions sans effet)
                                             │        │
 DOM  ◄── applique les changements ◄─ patchs ┤  arbre virtuel → comparaison → patchs
```

Pourquoi ce choix plutôt que traduire MCF en JavaScript :

| | Interpréteur en WASM (proposé) | Traduction MCF → JavaScript |
|---|---|---|
| Règles du langage (`Int` 64 bits vérifié D33, ordonnanceur déterministe D42, partage D30, erreurs MCF04xx) | **identiques par construction** : c'est le même code | à réécrire et à garder synchronisé ; `Int` 64 bits impose `BigInt` partout |
| Tests | ceux du runtime valent pour le web | une seconde suite complète |
| Taille | ~0,5 à 1 Mo (runtime) | plus petit |
| Lisibilité du JS produit | sans objet | lisible |

**Coût assumé** : un fichier plus lourd et un démarrage un peu plus lent. En échange, un programme ne peut pas se comporter différemment entre `mcf run` et le navigateur. La cible « JavaScript lisible / TypeScript » du §15 reste possible plus tard, sur la représentation intermédiaire (MIR, §18), avec les tests du runtime comme référence.

Le runtime WASM est compilé par la CI depuis ce dépôt et **embarqué dans le binaire `mcf`** : le développeur n'a besoin ni de Rust, ni de Node, ni d'aucun outil web. Aucune dépendance JavaScript tierce.

## 3. Démarrage et cycle de vie (D59, D60)

1. **Racine** : le projet contient exactement un `ui` dont la vue commence par `Window`. Aucun → MCF0717 ; plusieurs → MCF0718 (avec la liste). `title` de `Window` devient le titre de la page.
2. Les `config` sont lues, puis les globales initialisées (ordre D35).
3. `func main(): Void` est **facultative** pour la cible web ; si elle existe, elle s'exécute avant le premier affichage. Les tâches qu'elle lance continuent ensuite.
4. Premier affichage, puis l'application vit tant que la page est ouverte (elle ne « se termine » pas quand les tâches sont finies, contrairement à `mcf run`).

**Événements et réactivité** :

- Une action d'interface (`onClick`, `onChange`…) devient une tâche dans l'ordonnanceur D42 : une seule tâche à la fois, ordre déterministe, mêmes règles qu'en `mcf run`.
- **Quand il n'y a plus de tâche prête**, la vue racine est recalculée en entier, comparée à la précédente, et seuls les changements sont appliqués au DOM. Pas d'affichage intermédiaire au milieu d'une tâche : l'interface ne montre jamais un état à moitié modifié.
- Recalculer toute la vue est sans risque parce que les vues n'ont pas d'effets (§5). Le suivi fin des dépendances est une optimisation pour plus tard, sans changement de comportement.
- `sleep` suit l'horloge réelle du navigateur ; l'ordre entre tâches prêtes reste déterministe.
- `print` écrit dans la console du navigateur.

**Erreur d'exécution** (MCF04xx) : comme en `mcf run`, une erreur arrête tout. L'interface est figée, un panneau affiche le code, le message et `fichier:ligne`, et plus aucun événement n'est traité. Pas de reprise silencieuse dans un état incohérent.

## 4. Correspondance avec HTML et CSS (D61)

| MCF | HTML | Remarques |
|---|---|---|
| `Window` | `<main>` plein écran | `title` → `document.title` ; une seule fenêtre dans un navigateur |
| `Box` | `<div>` | |
| `Text` | `<span>` | contenu posé par `textContent`, **jamais** interprété comme du HTML |
| `Image` | `<img>` | `alt` → `alt` ; source : voir §6 |
| `Button` | `<button type="button">` | focus et touche Entrée natifs |
| `Input` | `<input type="text">` | `onChange` à chaque saisie, `onSubmit` sur Entrée |
| `onEnter` / `onLeave` | `pointerenter` / `pointerleave` | |

- Styles appliqués propriété par propriété (`element.style.x = …`), jamais par une chaîne CSS assemblée : une valeur ne peut pas en injecter d'autres. Aucune feuille de style globale à part une remise à zéro fixe : pas de cascade, comme D54.
- `layout: row | column` → flexbox ; `stack` → grille à une seule cellule (enfants superposés) ; `x`, `y` → position absolue dans le parent ; `grow` → `flex-grow` ; `radius` → `border-radius`.
- **Couleurs venant d'une expression `String`** : revérifiées à l'exécution (`#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`). Une autre chaîne → erreur MCF0415 plutôt qu'une couleur inventée.
- **Listes (`foreach`)** : en phase 11, les éléments sont identifiés par leur position. Conséquence connue : si une liste est réordonnée, un `Input` qui a le focus peut le céder à son voisin. Une propriété `key:` réglera ce cas plus tard.
- Garde-fou : plus de 10 000 éléments dans une vue → MCF0416, pour qu'une erreur de programme ne fige pas le navigateur sans explication.

## 5. Vues sans effets (D62, engagement D56)

Une expression d'un `.uimcf` (vue, valeur de style, propriété) ne peut appeler que des fonctions **sans effet**. Les actions (`onClick: f`…) ne sont pas concernées : elles sont faites pour modifier l'état.

Une fonction a un effet si son corps (ou une fonction qu'elle appelle, de proche en proche) :

- affecte une variable globale ;
- modifie un champ, un élément ou appelle `push`, `pop`, `removeAt`, `remove` sur une valeur qui n'est pas **fraîche** ;
- utilise `emit`, `start emit`, `start`, `await`, `sleep`, `on X => f`, `cancel()`, `stop()`, `print` ou `loadConfig`.

Une valeur est **fraîche** si c'est une variable locale qui n'a jamais reçu autre chose qu'un littéral (structure, tableau) ou un `duplicate(…)`. Construire une liste filtrée dans une variable locale reste donc permis ; modifier un paramètre (partagé, D30) ne l'est pas. Dans le doute, le compilateur considère qu'il y a un effet.

Erreur MCF0716, avec la chaîne qui mène à l'effet :

```text
MCF0716 : `resume` a un effet de bord et ne peut pas être appelée depuis une vue
  resume → formater (scores.mcf:12) → print (scores.mcf:18)
  suggestion : faire le calcul dans une fonction sans effet, ou le déclencher par une action (onClick: …)
```

Cette règle s'applique à toutes les cibles (y compris `mcf check`), pas seulement au web.

## 6. Fichiers publiés (D63)

Ce qui part dans `build/web/` est public. On n'y met donc que ce qui est **prouvé utile** :

- **`.cfg`** : seulement ceux cités par chemin littéral (`config` et `loadConfig("…")`), après validation. `loadConfig` avec un chemin calculé est refusé pour la cible web (MCF0719) : le compilateur ne pourrait pas savoir quels fichiers publier, et publier tout le dossier risquerait d'exposer un fichier qui n'était pas destiné au public.
- **Images** : fichiers du projet uniquement, même confinement que D51 (chemin réel dans le dossier du projet). Chemin littéral : vérifié et copié à la construction. Chemin calculé : seules les images déjà publiées s'affichent ; une autre → MCF0417. Les images externes (`https://…`) viendront avec les permissions réseau de la phase 12.
- Aucun autre fichier du projet n'est copié.

## 7. Modèle de permissions — qui voit quoi

| Acteur | Ce qu'il voit | Ce qu'il peut faire |
|---|---|---|
| Visiteur de la page | Tout le contenu de `build/web/` (programme, `.cfg` publiés, images) et l'état de **son propre** onglet | Interagir avec son onglet, lire les fichiers publiés |
| Autre visiteur | Rien de l'état d'un autre visiteur | — |
| Hébergeur | Les fichiers publiés, les métadonnées d'accès (IP, horaires) | Servir ou modifier les fichiers : d'où HTTPS et SRI (§8) |
| Développeur | Tout | Construire et publier |
| **MCF Studio** | **Rien** : aucune requête vers MCF Studio, aucun CDN, aucune télémétrie | — |
| Tiers (régies, polices, CDN) | **Rien** : la page ne contacte aucun autre domaine (`connect-src 'none'`) | — |

**Isolation entre utilisateurs** : il n'y a pas de serveur ni d'état partagé. L'état d'une application vit uniquement dans la mémoire de l'onglet ouvert ; rien n'est enregistré dans le navigateur (`localStorage` et équivalents ne sont pas utilisés en phase 11). Fermer l'onglet efface tout.

## 8. Menaces et mesures (D65)

| Menace | Mesure |
|---|---|
| Injection de HTML / script par une donnée affichée (XSS) | Texte posé par `textContent` ; aucune API MCF ne produit de HTML ; CSP sans `unsafe-inline` ni `unsafe-eval` |
| Injection CSS par une couleur ou une longueur calculée | Propriétés posées une à une ; couleurs revérifiées (MCF0415) ; longueurs typées `Int` / pourcentage |
| Fuite d'un fichier sensible du projet dans le site publié | Seuls les fichiers cités par chemin littéral sont publiés (§6) ; liste affichée à chaque `build` ; `Secret<T>` (phase 12) refusé dans un programme web |
| Lecture hors du projet par un chemin construit (`../../`) | Confinement D51 sur le chemin réel, à la construction comme à l'exécution |
| Exfiltration de données vers un autre site | Aucune API réseau en phase 11 ; CSP `default-src 'none'`, `connect-src 'none'`, `img-src 'self'` |
| Site servi en clair, modifié en route | Le runtime refuse de démarrer en `http:` hors `localhost` / `127.0.0.1` (message explicite) ; `_headers` fournit HSTS |
| Hébergeur ou CDN qui altère un script | Empreintes SRI (`integrity=`) sur `mcf.js` et `app.js` ; aucun script externe |
| Page intégrée dans un cadre piégé (clickjacking) | `frame-ancestors 'none'` dans `_headers` (non applicable par `<meta>` : documenté) |
| Dépendance JavaScript compromise | Zéro dépendance JS tierce ; runtime WASM construit par la CI depuis ce dépôt |
| Vue qui boucle et fige l'onglet | Vues sans effets (§5), limite de 10 000 éléments (MCF0416), récursion limitée (MCF0406) |
| Code source ou notes internes exposés | Programme sérialisé sans commentaires ni chemins absolus (seulement `fichier.mcf:ligne` relatifs au projet, pour les erreurs) |
| Surveillance par MCF Studio | Aucune requête sortante, vérifiable par la CSP et par le code public |

En-têtes fournis dans `_headers` et reproduits en `<meta>` quand c'est possible :

```text
Content-Security-Policy: default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; img-src 'self'; style-src 'self'; connect-src 'none'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'
Strict-Transport-Security: max-age=63072000; includeSubDomains
X-Content-Type-Options: nosniff
Referrer-Policy: no-referrer
Permissions-Policy: camera=(), microphone=(), geolocation=()
```

Limite honnête : MCF ne contrôle pas l'hébergeur. Si les en-têtes de `_headers` ne sont pas appliqués, la protection repose sur la CSP en `<meta>`, le refus du `http:` et SRI ; `frame-ancestors` et HSTS manqueront. `build` le rappelle.

## 9. Outillage (D64)

- `mcf build --target web <dossier> [--out <dossier>] [--strict]` : vérifie comme `mcf check` (une erreur → rien n'est produit), puis écrit le dossier. Sortie JSON disponible (`--json`) avec la liste des fichiers publiés.
- `mcf run --web <dossier>` : construit dans `<dossier>/build/web/` puis ouvre `index.html` dans le navigateur par défaut, en `file://` : aucun serveur local, donc aucun flux réseau à chiffrer.
- `mcf run` sans `--web` sur un projet qui a une racine `Window` : exécute le programme comme aujourd'hui (sans affichage) et le signale par un conseil MCF05xx.
- `mcf watch` et le rechargement automatique arrivent avec l'IDE (prévisualisation dans la vue web de Tauri, sans serveur non plus).

## 10. Tests

- **Rust, sans navigateur** : calcul de l'arbre virtuel, comparaison et patchs produits (cas : `if` qui bascule, `foreach` qui grandit/rétrécit, styles qui changent, `bind`), ordre des tâches déclenchées par des événements simulés, analyse des effets (exemples valides et invalides dans `tests/conformance`).
- **Construction** : contenu exact de `build/web/` (fichiers publiés, absence de fichiers non cités, CSP, empreintes SRI), sortie identique d'une construction à l'autre (pas de date ni de chemin absolu dans les fichiers).
- **Navigateur** : quelques tests de bout en bout (Playwright, Chromium et Firefox) en CI : affichage, clic, saisie, panneau d'erreur, refus en `http:`, aucune requête réseau sortante.

## 11. Pas en phase 11

Traduction lisible vers JavaScript / TypeScript ; plusieurs fenêtres ; animations, styles adaptatifs, polices, images externes ; `key:` dans les listes ; enregistrement dans le navigateur ; réseau (`OS.Network`, phase 12) ; rechargement à chaud (IDE).

## 12. Nouveaux diagnostics

| Code | Cas |
|---|---|
| MCF0415 | Couleur calculée invalide (exécution) |
| MCF0416 | Plus de 10 000 éléments dans une vue (exécution) |
| MCF0417 | Image calculée absente des fichiers publiés (exécution) |
| MCF0716 | Fonction avec effet de bord appelée depuis une vue |
| MCF0717 | Cible web sans `ui` racine (`Window`) |
| MCF0718 | Plusieurs `ui` racines |
| MCF0719 | `loadConfig` à chemin calculé dans un programme web |
