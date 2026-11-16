# Résolution des noms (phase 3)

Implémentée par `compiler/semantic`. Base : cahier des charges 1.2, §7 et §8.

## Projet

`mcf check <dossier>` traite **tout le dossier comme un seul projet** : chaque fichier `.mcf` est un module, et les imports sont résolus entre ces fichiers.

## Ordre de recherche d'un nom

1. Blocs locaux, du plus proche au plus lointain (paramètres compris).
2. Scopes englobants, du plus proche jusqu'à la racine du fichier.
3. Éléments importés dans le fichier.
4. Fonctions natives : `print(x)`, `sleep(ms)`.

## Règles

| Règle | Diagnostic |
|---|---|
| Nom introuvable (suggestion d'`import` s'il existe dans un autre fichier) | MCF0200 |
| Deux déclarations du même nom dans le même scope / bloc | MCF0201 |
| Type inconnu | MCF0202 |
| DualType avec deux fois le même type (`Int, Int`) | MCF0203 |
| Locale qui en masque une autre d'un bloc parent | MCF0204 (avertissement, erreur en `--strict`) |
| Événement non déclaré (`on` / `emit`) | MCF0205 |
| Import introuvable | MCF0206 |
| `Scope.membre` inexistant | MCF0207 |
| Import ambigu ou en collision | MCF0208 |
| Nombre de données d'événement incorrect | MCF0209 |
| Nombre d'arguments incorrect | MCF0210 |
| `break` / `continue` hors boucle | MCF0211 |
| Affectation à une fonction ou un scope | MCF0212 |

- Globales, fonctions et scopes d'un fichier sont visibles partout dans ce fichier, quel que soit l'ordre.
- Une locale n'est visible qu'**après** sa déclaration : `_x: Int = _x + 1;` est refusé.
- Une locale déclarée dans un `if` ou une boucle n'existe plus après le bloc.
- `import Player;` rend `Player` utilisable (`Player.name`) ; `import Player.name;` rend `name` utilisable directement.

## Types natifs reconnus

`Bool Int Float String Char Byte Bytes Array Map DateTime Void`

Les structures, énumérations et alias de types viendront avec les phases suivantes.
