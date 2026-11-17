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
3. Le programme s'arrête quand `main` **et toutes les tâches** lancées par `start` sont terminées, ou à la première erreur d'exécution (dans n'importe quelle tâche).

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

`emit` exécute **tout de suite** chaque gestionnaire `on` de l'événement, dans l'ordre du projet (fichiers puis déclarations), avant de passer à l'instruction suivante. Plusieurs `on` pour un même événement sont permis (D32). Un `emit` dans une tâche exécute les gestionnaires dans cette tâche.

`start emit` lance au contraire chaque gestionnaire dans **sa propre tâche** (dans le même ordre) et continue aussitôt (D44). C'est l'émetteur qui choisit, comme pour `start f()` : on voit à l'endroit de l'`emit` ce qui part en arrière-plan.

```mcf
emit Saved("a.txt");        // attend que tous les gestionnaires aient fini
start emit Saved("b.txt");  // n'attend pas ; le programme attendra ces tâches avant de finir
```

### Abonnements

Les `on X(...) { }` sont fixes : déclarés au niveau du module, toujours actifs. Pour s'abonner et se désabonner pendant l'exécution, `on X => fonction` abonne une fonction existante et rend un `Listener` (D45) :

```mcf
func greet(_id: Int): Void {
    print("Bienvenue " + _id);
}

func main(): Void {
    _l: Listener = on PlayerConnected => greet;
    emit PlayerConnected(1);   // Bienvenue 1
    _l.stop();
    emit PlayerConnected(2);   // rien
}
```

- La fonction reçoit exactement les données de l'événement et retourne `Void` (MCF0209, MCF0213).
- À chaque `emit` : les `on` fixes d'abord, puis les abonnements actifs dans l'ordre de leur création. Un abonnement créé pendant un `emit` compte à partir du suivant ; un abonnement arrêté pendant un `emit` n'est plus appelé.
- `start emit` lance aussi chaque abonnement dans sa propre tâche.
- `Listener` est une poignée (comme `Task`) : la copier désigne le même abonnement.

### Métadonnées

Un événement peut transporter des informations en plus de ses données, sans créer une structure pour chaque besoin (§11.1, D46) :

```mcf
event Purchase(_item: String) meta {
    channel: String = "jeu";              // chaque métadonnée a une valeur par défaut
    label: String = "achat de " + _item;  // les données de l'événement sont lisibles
}

on Purchase(_item) {
    print(meta.label + " via " + meta.channel);
    print(meta.source);                   // « main.mcf:12 », fournie automatiquement
}

func main(): Void {
    emit Purchase("épée");                         // achat de épée via jeu
    emit Purchase("arc") meta { channel: "web" };  // achat de arc via web
}
```

- L'émetteur précise seulement ce qu'il veut changer ; les autres métadonnées prennent leur valeur par défaut, **calculée à chaque `emit`**.
- Tout est vérifié : métadonnée inconnue (MCF0241), donnée deux fois (MCF0242), mauvais type (MCF0220), valeur par défaut absente (MCF0100).
- `meta.source` (« fichier:ligne » de l'`emit`) est toujours fournie et ne se donne pas.
- Dans un gestionnaire `on`, `meta` se lit seulement (MCF0212). Une fonction abonnée avec `on X => f` reçoit les données mais pas les métadonnées.
- À venir : `meta.time`, quand le type `DateTime` aura des valeurs.

## Tâches

Une fonction ordinaire peut tourner en tâche : pas de mot-clé `async` sur les fonctions (D41).

```mcf
func load(_id: Int): String {
    sleep(100);                       // laisse tourner les autres tâches
    return "user" + _id;
}

func main(): Void {
    _a: Task<String> = start load(1); // lancée, main continue
    _b: Task<String> = start load(2);
    print(await _a + await _b);       // attend les résultats (~100 ms en tout)
}
```

- `start appel(...)` évalue les arguments **tout de suite**, puis lance l'appel dans une nouvelle tâche et rend un `Task<T>`. La tâche ne démarre pas avant le prochain point de suspension de la tâche qui l'a lancée.
- `await t` attend la fin de `t` et vaut son résultat. On peut attendre plusieurs fois la même tâche (même résultat).
- Une tâche non attendue s'exécute quand même : le programme attend toutes les tâches avant de se terminer.
- `Task` est une poignée : `_t2 = _t` désigne la même tâche ; `==` compare l'identité ; `duplicate` rend la même poignée. `print` affiche `Task#1`.

### Une seule tâche à la fois, ordre déterministe

Les tâches ne s'exécutent **jamais en même temps** : une tâche garde la main jusqu'à un **point de suspension** (`sleep`, `await`, fin de la tâche). Il n'y a donc pas de course de données sur les globales et les valeurs partagées, et un même programme affiche toujours la même chose.

À chaque point de suspension, la suivante est choisie ainsi :

1. les tâches prêtes, dans l'ordre où elles le sont devenues (`start`, fin d'un `await`, annulation) ;
2. sinon, la tâche dont le `sleep` se termine le plus tôt sur une horloge **virtuelle** (somme des durées demandées), à égalité dans l'ordre des appels à `sleep`. Le programme attend réellement l'échéance : `sleep(ms)` dure toujours au moins `ms`.

`sleep(0)` laisse passer les tâches prêtes. Une boucle de calcul sans point de suspension garde la main jusqu'au bout : le compilateur le signale par un [conseil](conseils.md) (MCF0500, MCF0501).

### Annulation

`_t.cancel()` demande l'arrêt de `_t` ; `_t.isDone()` dit si elle est terminée ou annulée.

- Sans effet sur une tâche déjà terminée.
- Une tâche qui dort ou attend est réveillée et s'arrête aussitôt qu'elle reprend la main ; une tâche qui calcule s'arrête à son prochain point de suspension ; une tâche pas encore démarrée ne démarre pas.
- `await` d'une tâche annulée → MCF0410.

### Erreurs

- Une erreur dans une tâche **arrête tout le programme** (code de sortie 3), comme dans `main` : rien n'échoue en silence. La pile d'appels remonte jusqu'au `start`.
- Toutes les tâches s'attendent mutuellement (`a` attend `b` qui attend `a`), ou une tâche s'attend elle-même → MCF0411.
- Le système refuse de créer la tâche (ressources épuisées) → MCF0412.

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
- Chaque tâche a sa propre limite de 10 000 appels imbriqués.

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
