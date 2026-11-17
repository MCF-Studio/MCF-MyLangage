# Exécution (phase 6)

Implémentée par `compiler/runtime` : un interpréteur qui parcourt l'AST. Il sert à exécuter et tester les programmes dès maintenant ; le backend natif viendra en phase 12.

```bash
mcf run examples/hello          # vérifie, puis exécute à partir de main
mcf run mon_projet/ --strict    # les avertissements bloquent aussi l'exécution
```

`mcf run` vérifie d'abord tout le projet comme `mcf check`. S'il y a une erreur, **rien n'est exécuté** (code de sortie 1). Une erreur pendant l'exécution donne le code de sortie 3.

## Déroulement

1. Les globales reçoivent leur valeur : fichiers dans l'ordre alphabétique, puis ordre de déclaration (scopes compris).
2. `func main(): Void` est appelée (voir [résolution des noms](resolution-des-noms.md#point-dentrée)).
3. Le programme s'arrête à la fin de `main`, ou à la première erreur d'exécution.

## Valeurs

| Type | Comportement |
|---|---|
| `Int`, `Float`, `Bool`, `String`, énumération | copiés |
| structure, `Array`, `Map` | **partagés** : `_b = _a;` désigne la même valeur ; une fonction qui modifie un champ de son paramètre modifie l'original (D30) |

Pour travailler sur une copie sans toucher à l'original, `duplicate` (D37) :

```mcf
_shared: Team = _a;            // même équipe : modifier _shared modifie _a
_copy: Team = duplicate(_a);   // copie complète : _a ne bouge plus
_copy.members.push("Max");     // _a.members est inchangé
```

`duplicate` recopie tout, à tous les niveaux (les structures, tableaux et maps contenus aussi). Sur un nombre, une chaîne ou une énumération, il rend simplement la même valeur.

- Une variable déclarée sans valeur : un `Array` ou une `Map` démarrent vides ; toute autre variable reste **sans valeur** jusqu'à sa première affectation, et la lire avant → MCF0405 (D31).
- `==` compare le contenu (deux structures aux mêmes champs sont égales) ; `Int` et `Float` se comparent avec promotion.

## Calculs

- `Int` : tout dépassement de 64 bits signés → MCF0402. Division ou reste par zéro → MCF0401. La division entière tronque vers zéro (`7 / 2` = `3`, `-7 % 3` = `-1`).
- `Float` : règles IEEE 754 (`1.0 / 0.0` donne l'infini, pas d'erreur).
- `&&` et `||` n'évaluent leur droite que si nécessaire.

## Collections

- `tableau[i]` hors des limites → MCF0403 (lecture comme écriture : un tableau ne grandit pas par index).
- `map[clé]` : la lecture d'une clé absente → MCF0404 ; l'écriture ajoute ou remplace.
- Méthodes natives (`push`, `pop`, `length`…) : voir [types](types.md#méthodes-natives). `pop` sur un tableau vide → MCF0409.
- `foreach` sur un tableau parcourt ses éléments, sur une `Map` ses clés dans l'ordre d'insertion. Le contenu parcouru est celui du **début** de la boucle.

## Événements

`emit` exécute **tout de suite** chaque gestionnaire `on` de l'événement, dans l'ordre du projet (fichiers puis déclarations), avant de passer à l'instruction suivante. Plusieurs `on` pour un même événement sont permis (D32). L'asynchrone arrive en phase 8.

## Affichage (`print`)

| Valeur | Affichée |
|---|---|
| `String` | telle quelle : `Alex` |
| `Float` | toujours avec sa partie décimale : `3.0`, `0.5` |
| énumération | `Status.Online` |
| `Array` | `["Alex", "Max"]` (chaînes entre guillemets) |
| `Map` | `{"Alex": 31}` |
| structure | `User { name: "Max", level: 3 }` |

La concaténation `"texte" + valeur` utilise le même affichage.

## Limites

- Récursion : 10 000 appels imbriqués au maximum → MCF0406.
- `sleep(ms)` : une durée négative → MCF0407.

## Erreurs d'exécution

Chaque erreur indique le fichier, la ligne, ce qui était attendu et la pile d'appels (les appels récursifs identiques sont regroupés) :

```text
erreur[MCF0403] INDEX_OUT_OF_BOUNDS: index 2 hors du tableau (2 élément(s))
  --> main.mcf:4:15
  |
4 |     print(_xs[2]);
  |               ^
  = attendu : un index entre 0 et 1
  = reçu : 2
  dans `main`
```

`mcf describe errors` liste les codes MCF04xx. MCF0499 signale un bug de l'interpréteur sur un programme pourtant vérifié : à signaler.

## Tests

`tests/execution/` : chaque `nom.mcf` (ou dossier `nom/`) est exécuté, et sa sortie comparée à `nom.attendu`. Un programme qui doit échouer commence par `// erreur: MCFxxxx`.
