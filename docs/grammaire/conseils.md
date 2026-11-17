# Conseils pour apprendre (MCF05xx)

MCF veut aussi servir à **apprendre à coder**. En plus des erreurs et des avertissements, le compilateur donne des **conseils** : un code qui fonctionne, mais qui risque de surprendre un débutant.

```text
conseil[MCF0500] LOOP_WITHOUT_PAUSE_IN_TASK: cette boucle ne fait aucune pause, alors que sa fonction est lancée avec `start` : tant qu'elle tourne, les autres tâches (et `main`) attendent
  --> main.mcf:6:5
  |
6 |     while (_i < _n) do {
  |     ^^^^^^^^^^^^^^^
  = suggestion : si la boucle est courte, ce n'est pas grave. Si elle peut durer (beaucoup de tours, attente d'un état), ajoute `sleep(0);` dans la boucle pour laisser les autres tâches avancer à chaque tour
```

- Un conseil ne bloque **jamais** : ni `mcf check`, ni `mcf run`, même en `--strict` (D43).
- `--no-hints` les masque (`mcf check projet --no-hints`).
- Un conseil n'est donné que lorsque le compilateur est **sûr** de la situation. Dans le doute, il se tait : un conseil faux apprendrait une mauvaise règle.

## Liste

| Code | Situation | Pourquoi |
|---|---|---|
| MCF0500 | Boucle `while` sans aucune pause (`sleep`, `await`) dans une fonction ou méthode lancée avec `start` | Une seule tâche s'exécute à la fois ([tâches](execution.md#tâches)) : pendant la boucle, les autres attendent. Sans gravité si la boucle est courte ; sinon `sleep(0);` à chaque tour |
| MCF0501 | `while (true)` sans `break`, `return`, `sleep` ni `await` | La boucle ne finit jamais et occupe tout : le programme se fige. Pour une boucle de jeu, `sleep(16);` (≈ 60 tours par seconde) |
| MCF0502 | `==` ou `!=` avec un `Float` calculé ou variable | `0.1 + 0.2` vaut `0.30000000000000004` : comparer avec une marge, ou compter en entiers (centimes) |
| MCF0503 | `x == true`, `x == false` | `if (x)` et `if (!x)` disent la même chose, plus simplement |

### Quand le compilateur se tait

- Une boucle qui appelle une fonction MCF ou fait un `emit` : la fonction appelée peut faire une pause, le compilateur ne le sait pas → pas de MCF0500 / MCF0501.
- Une boucle sans pause dans une fonction jamais lancée avec `start` → pas de MCF0500 (rien d'autre n'attend).
- `3 == 3.0` : deux littéraux, aucun calcul → pas de MCF0502.

## Ajouter un conseil

Chaque conseil a un test dans `tests/conformance/conseils/` qui le déclenche, et un autre (`// attendu: aucun`) qui vérifie qu'il ne se déclenche pas à tort.
