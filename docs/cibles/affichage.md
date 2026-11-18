# Couche d'affichage commune (phase 11) — web et MCF Engine

**Statut :** D68 proposée, implémentée dans la crate `mcf-view` (`compiler/view`). Le calcul d'un **instantané** de l'arbre (valeurs initiales, sans événement ni session ; `mcf_runtime::render_view`, commande `mcf view` qui l'exporte en page HTML d'aperçu) est fait ; le calcul vivant (événements, `state`, recalcul : D69) et les ponts vers le DOM ou `bevy_ui` ne sont pas encore écrits.

Le runtime ne sait rien de l'écran. À chaque recalcul de la vue racine (D60), il produit un **arbre de vue** (`Node`) ; `Renderer::update` le compare au précédent et rend une **liste de changements** (`Change`). Chaque cible applique ces changements à son affichage :

```text
runtime MCF ──► arbre de vue ──► Renderer (comparaison) ──► changements ──┬─► web : éléments du DOM (D61)
     ▲                                                                      └─► MCF Engine : entités bevy_ui
     └──────── événement (NodeId, onClick…) ◄──── la cible signale une action ────┘
```

## 1. Arbre de vue

Un `Node` = sorte d'élément (`Window`, `Box`, `Text`, `Image`, `Button`, `Input`), propriétés texte (`title`, `text`, `label`…), **style final** (style nommé puis propriétés de l'élément, D54), événements écoutés, enfants.

- Les composants `ui` sont déjà développés : l'arbre ne contient que des éléments de base.
- Les valeurs sont déjà validées : couleurs analysées en RVBA (`#rgb` à `#rrggbbaa`, MCF0415 sinon), longueurs en pixels ou pourcentage, poids 100–900, opacité 0–1, mots de la liste fermée. Une cible n'a **aucun texte à interpréter** comme du HTML, du CSS, une couleur ou un chemin.
- Les actions (`onClick: f`) restent dans le runtime ; l'arbre dit seulement quels événements sont écoutés.
- Plus de 10 000 éléments → refus (MCF0416), sans rien changer à l'affichage.

## 2. Changements

| Changement | Sens |
|---|---|
| `create { id, parent, index, kind }` | Élément vide inséré à la position `index` parmi les enfants actuels de `parent` (`null` : racine) ; ses propriétés et ses enfants suivent |
| `remove { id, subtree }` | Retire l'élément et ses descendants ; `subtree` les liste tous (parents d'abord) pour que la cible oublie leurs identifiants |
| `setProp` / `removeProp` | Texte d'une propriété |
| `setStyle` / `removeStyle` | Une propriété de style ; `removeStyle` = valeur par défaut |
| `listen` / `unlisten` | Événement écouté ou non |

- La liste s'applique **dans l'ordre**, en entier, entre deux images : pas d'état à moitié affiché (D60).
- Identité **par position** (D61) : l'enfant n° `i` est comparé à l'ancien enfant n° `i` ; même sorte → mis à jour en place, sinon remplacé. Les enfants en trop sont retirés en partant de la fin. Pas de déplacement d'élément en phase 11 (`key:` plus tard).
- Un `NodeId` est attribué à la création et n'est **jamais réutilisé** : un événement qui arrive pour un élément déjà retiré est reconnu et ignoré (`Renderer::path` rend `None`).
- `Input` : la cible ne réécrit pas la valeur affichée si elle est déjà égale à celle reçue (sinon le curseur sauterait pendant la saisie).
- Format JSON stable (`{"op":"setStyle","id":7,"style":"fontSize","value":{"length":{"percent":50.0}}}`) pour la couche JS ; le pont Bevy utilise directement les types Rust.

## 3. Correspondance MCF Engine (`bevy_ui`), à confirmer avec le pont

| MCF | Bevy (0.20) |
|---|---|
| `Window` | nœud racine plein écran (la fenêtre du jeu existe déjà ; `title` ignoré ou titre de la fenêtre, à décider côté engine) |
| `Box` | `Node` |
| `Text` | `Text` |
| `Image` | `ImageNode`, image chargée **uniquement** depuis les assets du jeu (même confinement que D51/D63) |
| `Button` | `Button` + `Text` enfant |
| `Input` | champ de saisie de l'engine (Bevy n'en fournit pas de complet : à écrire dans `mcf_ui` ou à reporter) |
| `layout`, `align`, `justify`, `gap`, `grow`… | `Node` (flexbox / grille de Bevy) |

## 4. Modèle de permissions et menaces

Un `.uimcf` est traité comme **non fiable** par l'engine (mods, contenu téléchargé) : il ne peut qu'afficher et déclencher les fonctions MCF de son projet.

| Menace | Mesure |
|---|---|
| Injection de balisage ou de style par une donnée affichée | Texte affiché tel quel, styles typés et validés avant l'arbre ; aucune chaîne interprétée par la cible |
| Vue énorme qui fige le jeu ou le navigateur | 10 000 éléments au plus (MCF0416), vérifié avant toute modification ; vues sans effets (D62) |
| Événement forgé pour un élément retiré ou inexistant | Identifiants jamais réutilisés ; `path` → `None` → ignoré |
| Image hors des fichiers autorisés | Chemins confinés (D51, D63), revérifiés par la cible |
| Liste de changements incohérente (bug) | `Document` de référence : chaque test vérifie que les changements reproduisent exactement la vue ; la cible refuse un identifiant inconnu au lieu de deviner |

Isolation : un `Renderer` par interface affichée ; aucun état partagé entre deux sessions (deux joueurs, deux onglets).
