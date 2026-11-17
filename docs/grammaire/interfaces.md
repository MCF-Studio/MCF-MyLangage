# Interfaces `.uimcf` (phase 10)

Cahier des charges 1.2, §13 : la présentation vit dans les fichiers `.uimcf`, la logique dans les `.mcf`, et les deux forment un seul projet. Choix de MCF Studio (D52) : syntaxe en blocs MCF, styles inline ou nommés sans cascade, aucune logique dans un `.uimcf`. Les arbitrages de détail sont D53 à D57.

La phase 10 couvre la syntaxe et la vérification (`mcf check`). L'affichage arrive avec le backend web (phase 11) : `mcf run` ignore pour l'instant les `.uimcf`.

## Exemple

```mcf
// compteur.mcf
clics: Int = 0;
func ajouter(): Void {
    clics = clics + 1;
}
```

```mcf
// compteur.uimcf
import clics;
import ajouter;

style titre {
    fontSize: 24;
    fontWeight: 700;
}

ui Compteur {
    prop libelle: String = "Clics";
    state ouvert: Bool = false;

    view {
        Box(style: carte) {
            Text(libelle + " : " + clics, style: titre);
            Button("+1", onClick: ajouter);
            Button("Détails", onClick: ouvert = !ouvert);
            if (ouvert) then {
                Text("Un clic de plus à chaque appui.", color: #9aa0a6);
            }
        }
    }

    style carte {
        layout: column;
        gap: 8;
        padding: 16;
        background: #1e1e2e;
        radius: 8;
    }
}
```

## Structure d'un fichier

```text
UiFile      = { ImportDecl | StyleDecl | UiDecl } ;
UiDecl      = "ui" Ident "{" { Member } "view" ViewBlock { StyleDecl } "}" ;
Member      = ( "prop" | "state" ) Ident ":" TypeDecl [ "=" Expr ] ";" ;
StyleDecl   = "style" Ident "{" { Property ";" } "}" ;
ViewBlock   = "{" { Node } "}" ;
Node        = Element
            | "if" "(" Expr ")" "then" ViewBlock [ "else" ( ViewBlock | IfNode ) ]
            | "foreach" "(" LocalIdent [ ":" TypeDecl ] "in" Expr ")" "do" ViewBlock ;
Element     = Ident [ "(" [ Arg { "," Arg } ] ")" ] ( ViewBlock | ";" ) ;
Arg         = Expr                                  (* propriété principale, en premier *)
            | [ "bind" ] Ident ":" Value ;
Property    = Ident ":" Value ;
Value       = Expr | Percent | Color | Action ;
Percent     = Number "%" ;                          (* 50% *)
Color       = "#" HexDigits ;                       (* 3, 4, 6 ou 8 chiffres *)
Action      = Expr                                  (* appel : f, f(x) *)
            | "emit" Path "(" [ Args ] ")"
            | Place "=" Expr ;                      (* état local uniquement *)
```

`ui`, `prop`, `state`, `view`, `style` et `bind` sont des mots contextuels : ils ne sont pas réservés dans les `.mcf`. `#` n'existe que dans les `.uimcf`.

Tout ce qui est de la logique est refusé avec un message qui renvoie vers un `.mcf` (MCF0715) : `func`, `struct`, `enum`, `event`, `on`, `config`, variables globales, et dans les expressions `start`, `await` et `on X => f`.

## Composants (`ui`)

- Un `ui` est un composant réutilisable. Son nom est unique dans tout le projet et utilisable dans n'importe quel `.uimcf` **sans import** (D53). Il ne peut pas reprendre le nom d'un élément de base (MCF0708).
- `prop` : paramètre reçu de celui qui utilise le composant (`Carte(titre: "Bienvenue")`). Sans valeur par défaut, il est obligatoire (MCF0704).
- `state` : état propre au composant, valeur initiale obligatoire. Il se modifie seulement par un `bind` ou une action d'affectation (D55).
- Les membres s'écrivent sans `_`, comme des champs, et sont lus directement par leur nom. Un membre ne peut pas reprendre le nom d'une variable, fonction ou d'un type visible dans le fichier (MCF0708) : on sait toujours ce que désigne un nom.
- `view` est obligatoire (MCF0713).
- Un composant qui s'inclut lui-même sans condition (directement ou par un autre) ne s'afficherait jamais : MCF0714. Une inclusion sous `if` ou `foreach` (arbre, liste imbriquée) est permise.

## Éléments de base

Volontairement peu nombreux (§13 : pas de bibliothèque de composants imposée) ; tout le reste se construit avec des `ui`.

| Élément | Propriété principale | Autres propriétés | Enfants |
|---|---|---|---|
| `Window` | — | `title: String` | oui ; seulement à la racine d'une `view` (MCF0709) |
| `Box` | — | — | oui |
| `Text` | `text: String` | — | non |
| `Image` | `source: String` | `alt: String` | non |
| `Button` | `label: String` | — | non |
| `Input` | `value: String` | `placeholder: String`, `onChange`, `onSubmit` | non |

- Tous acceptent les propriétés de style, `style: nom` et les événements `onClick`, `onEnter`, `onLeave`.
- La propriété principale peut s'écrire sans nom, en premier : `Text("Bonjour")`. Une seule valeur sans nom par élément (MCF0706).
- Propriété principale obligatoire pour `Text`, `Image`, `Button` (MCF0704). Enfants interdits là où le tableau dit non (MCF0705).
- Un composant s'utilise de la même façon, avec ses `prop` nommées.
- `Button` existe pour l'accessibilité (focus, touche Entrée) qu'un `Box` cliquable n'aurait pas.

## Styles (D54)

Une propriété s'écrit sur l'élément (`Box(padding: 8)`) ou dans un style nommé (`style carte { padding: 8; }`) appliqué par `style: carte`. Le style nommé est appliqué d'abord, les propriétés de l'élément ensuite. **Pas de cascade ni d'héritage** : une propriété ne vaut que pour l'élément qui la porte, ce qui permet de toujours savoir d'où elle vient (le concepteur visuel l'affiche à un seul endroit).

Un style déclaré dans un `ui` n'est visible que dans ce `ui` ; un style déclaré à la racine du fichier est visible dans tout le fichier. Style inconnu → MCF0707.

| Propriété | Valeur |
|---|---|
| `width`, `height`, `minWidth`, `maxWidth`, `minHeight`, `maxHeight` | longueur |
| `padding`, `paddingTop`, `paddingRight`, `paddingBottom`, `paddingLeft` | longueur |
| `margin`, `marginTop`, `marginRight`, `marginBottom`, `marginLeft` | longueur |
| `gap`, `borderWidth`, `radius`, `fontSize` | longueur |
| `x`, `y` | longueur ; placement libre dans le parent (concepteur visuel) |
| `background`, `color`, `borderColor` | couleur |
| `fontWeight` | `Int`, de 100 à 900 |
| `opacity` | `Float` ou `Int`, de 0 à 1 |
| `grow` | `Int` positif : part de l'espace libre |
| `visible` | `Bool` |
| `layout` | `row`, `column` (défaut) ou `stack` (enfants superposés) |
| `align` | `start`, `center`, `end`, `stretch` |
| `justify` | `start`, `center`, `end`, `between`, `around` |
| `textAlign` | `left`, `center`, `right` |
| `overflow` | `visible`, `hidden`, `scroll` |

- **Longueur** : `Int` en pixels (`16`, ou une expression `Int`), ou pourcentage littéral (`50%`).
- **Couleur** : littéral `#rgb`, `#rgba`, `#rrggbb` ou `#rrggbbaa`, ou expression `String` (une chaîne littérale doit avoir cette forme).
- Valeurs à mot (`layout: row`) : uniquement les mots de la liste.
- Les autres valeurs sont des expressions : elles peuvent lire les `prop`, les `state` et les globales importées (`width: largeur;`), et l'interface suit leurs changements (phase 11).
- Propriété inconnue → MCF0701 (avec suggestion), en double sur un même élément ou style → MCF0702, valeur du mauvais type → MCF0703, hors plage → MCF0712.

## Liaison avec le code (D55)

- Comme dans un `.mcf`, les événements sont visibles sans import ; les variables, fonctions et types s'importent (`import clics;`).
- **Lecture** : toute expression d'un `.uimcf` peut lire les `prop`, les `state`, les variables de `foreach` et ce qui est importé des `.mcf` (`import clics;`), avec les mêmes règles de types qu'en `.mcf`. Les appels de fonctions sont permis (mise en forme d'un texte, `items.length()`) ; leurs effets de bord ne sont pas encore vérifiés ; en phase 11, une fonction qui a des effets de bord sera refusée dans une vue (D56). `start`, `await` et `on X => f` sont refusés (MCF0715).
- **Événements** (`onClick`, `onEnter`, `onLeave`, `onChange`, `onSubmit`) : la valeur est une action, jamais un bloc de code :
  - un appel de fonction `.mcf` qui ne retourne rien : `onClick: ajouter` ou `onClick: retirer(_item)` ;
  - un événement MCF : `onClick: emit Ferme(id)` ;
  - une affectation d'un `state` du composant : `onClick: ouvert = !ouvert`. Seuls les `state` se modifient ainsi : une variable du programme se modifie dans une fonction `.mcf` (MCF0710).
- **Liaison bidirectionnelle** : `Input(bind value: recherche)` affiche la valeur et la met à jour à chaque saisie. La cible est un `state` `String` du composant, jamais une variable du programme, une configuration ou une `prop` (MCF0711). Pour faire entrer la saisie dans le programme, passer par une fonction `.mcf` : `Input(bind value: saisie, onChange: chercher(saisie))`. Seule `value` d'`Input` se lie pour l'instant.

## Vérification

`mcf check <dossier>` lit les `.uimcf` du projet avec les `.mcf` et les `.cfg` (un fichier passé seul est vérifié sans le code qu'il importe : passer le dossier). Diagnostics de la plage MCF07xx (`mcf describe errors`) ; les erreurs de syntaxe et de types communes aux `.mcf` gardent leurs codes.

## Pas encore

- Affichage, réactivité, prévisualisation : phase 11 (backend web), puis IDE.
- Accès aux propriétés d'un élément depuis le code, animations, styles adaptatifs (selon la taille de l'écran), polices, images de fond, ombres, `bind` sur d'autres propriétés.
- Contenu passé à un composant (enfants d'un `ui`).
