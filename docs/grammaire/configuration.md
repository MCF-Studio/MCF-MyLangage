# Configuration `.cfg` (phase 9)

Cahier des charges 1.2, §2 : un fichier `.cfg` est du JSON standard, validé par le système de types MCF. Implémenté par `compiler/config` (lecture et validation, partagées par `mcf check` et `mcf run`). Arbitrages : [DECISIONS](../DECISIONS.md) D47 à D51.

## Format du fichier (D48)

- JSON standard (RFC 8259), encodé en UTF-8 (un BOM en tête est ignoré).
- Seul ajout : les commentaires `// …` et `/* … */`.
- Refusé : `null` (MCF0620), une clé présente deux fois dans le même objet (MCF0601), virgule finale et toute autre extension (MCF0600).
- Limites : 4 Mio par fichier, 64 niveaux d'imbrication (MCF0602).

```json
{
  // serveur de production
  "port": 8080,
  "name": "prod",
  "mode": "Release",
  "hosts": ["a", "b"],
  "quotas": { "free": 10, "pro": 100 },
  "limits": { "users": 50, "ratio": 1.5 }
}
```

## Types acceptés

| Type MCF | Valeur JSON |
|---|---|
| `Bool` | `true` / `false` |
| `Int` | nombre entier, sans partie décimale ni exposant, dans la plage de `Int` (MCF0624) |
| `Float` | tout nombre fini (MCF0624 sinon) |
| `String` | chaîne |
| énumération | chaîne qui nomme exactement une variante (`"Release"`) — MCF0623, avec suggestion si la casse diffère |
| structure | objet avec **exactement** ses champs : champ absent → MCF0621, clé en trop → MCF0622 |
| `Array<T>` | tableau |
| `Map<String, T>` | objet (clés quelconques) |
| DualType `A, B` | la forme JSON choisit le type : `ratio: Int, Float` vaut `Int` pour `1` et `Float` pour `1.5` |

Tout autre type (`Task`, `Listener`, `Map` à clés non `String`, types encore inexistants comme `DateTime`) → MCF0612. Un DualType dont les deux types s'écrivent pareil en JSON (`String, Mode`, structure et `Map`, deux tableaux…) → MCF0613 : la valeur ne permettrait pas de savoir lequel est voulu. Une valeur du mauvais type → MCF0620.

Toutes les erreurs sont signalées **dans le `.cfg`**, à la valeur fautive, avec ce qui était attendu.

## `config` : configuration du programme (D47, D49)

```mcf
config server: Server = "server.cfg";

scope Settings {
    config limits: Limits = "cfg/limits.cfg";
}

address: String = server.name + ":" + server.port;
```

- Déclaration de module ou de scope (pas dans un bloc : MCF0116). Le type est obligatoire, le chemin est une **chaîne littérale** : le compilateur lit et vérifie le fichier.
- Chemin relatif au fichier `.mcf` qui déclare, dossiers séparés par `/`, terminé par `.cfg` ; `..` permis pour remonter dans le projet. Chemin absolu, `\` ou autre extension → MCF0611. Fichier absent du projet → MCF0610.
- La valeur est une globale **en lecture seule** : toute modification (`server.port = 1`, `server.hosts.push(…)`, `server.hosts[0] = …`) → MCF0614. Pour la modifier, en faire une copie : `_copy: Server = duplicate(server);`. Une modification qui passerait par un autre chemin (valeur transmise à une fonction) est arrêtée à l'exécution (MCF0414).
- Les configurations sont lues **avant** les autres globales, qui peuvent donc en dépendre.

## `loadConfig` : lecture à la demande (D50)

```mcf
_l: Limits = loadConfig("limites/" + _n + ".cfg");
```

- Seulement pour initialiser une variable dont le type est écrit (locale ou globale) : c'est ce type qui valide le fichier. Ailleurs (`print(loadConfig(…))`, affectation simple) → MCF0615.
- Le chemin peut être calculé. S'il est littéral et que le fichier est dans le projet, `mcf check` le valide ; un fichier absent n'est pas une erreur de compilation (il peut être créé avant l'exécution).
- La valeur obtenue est une copie ordinaire, modifiable.

## Exécution (D51)

- Le fichier a pu changer depuis `mcf check` : `mcf run` le relit et le revalide avec le même code. Fichier absent, illisible ou non conforme → MCF0413, avec la première erreur trouvée (fichier, ligne, colonne) ; `mcf check` les liste toutes.
- **Confinement** : seuls les `.cfg` situés dans le dossier du projet (le dossier passé à `mcf run`, ou celui du fichier passé) sont lus. Le chemin réel est vérifié après résolution des `..` et des liens symboliques : un chemin calculé à partir d'une donnée (`loadConfig("../" + nom + ".cfg")`) ne peut pas faire lire un fichier ailleurs.

## `mcf check`

Un dossier vérifié comprend ses `.cfg` ; un fichier `.mcf` seul est vérifié avec les `.cfg` de son dossier. Chaque `.cfg` est analysé (erreurs de syntaxe signalées même s'il ne sert à rien encore), puis validé par chaque type qui l'utilise.

## Pas encore

- Valeurs par défaut de champs absents, champs optionnels : en attente des valeurs optionnelles.
- Écriture d'un `.cfg` depuis le programme.
- Données sensibles (mots de passe, clés) : elles n'ont pas leur place dans un `.cfg` en clair ; le module `Secure` (phase 12, §26) fournira le stockage chiffré.
