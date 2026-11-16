# Chiffrement et souveraineté des clés

> ## MCF Studio ne peut pas vous surveiller. Par construction.
>
> Tout ce qui transite avec MCF est chiffré, avec **des clés générées et conservées chez vous**.
> MCF Studio ne détient aucune clé, n'impose aucun serveur relais et ne collecte aucune télémétrie.
> Même si MCF Studio le voulait — ou y était contraint — **il n'aurait techniquement aucun moyen de lire vos échanges**.

**Statut :** spécification (édition 1.2 du cahier des charges, §26). Rien de ce qui suit n'est encore implémenté : ce document fixe les engagements que l'implémentation devra respecter et que les tests devront prouver.

---

## 1. Ce que « tout ce qui transite » couvre

| Flux | Règle | Mécanisme prévu |
|---|---|---|
| Réseau des programmes MCF (`OS.Network`) | Chiffré obligatoirement, pas d'API en clair par défaut | TLS 1.3 vers le monde extérieur ; canal MCF ↔ MCF basé sur le framework Noise |
| Événements émis vers un autre processus ou une autre machine | Chiffrés et authentifiés | Même canal que le réseau, clés de session éphémères |
| IDE ↔ compilateur / Language Server | Chiffré même en local | Clé de session éphémère générée au lancement, IPC authentifié |
| Registre de paquets, dépendances | Chiffré en transit + paquets signés par la clé de l'auteur | TLS 1.3 + signature Ed25519 vérifiée avant installation |
| Mises à jour de la chaîne d'outils | Chiffrées + signées | Signature vérifiée avant installation |
| Applications web compilées par MCF | HTTPS obligatoire, chiffrement côté client disponible | WebCrypto, clés non extractibles |
| Secrets et données locales sensibles (au repos) | Chiffrés avec la clé de l'utilisateur | XChaCha20-Poly1305, clé dérivée de la clé utilisateur |
| Télémétrie vers MCF Studio | **N'existe pas** | — |

## 2. Les clés appartiennent à l'utilisateur

- **Génération locale.** Chaque utilisateur (développeur dans l'IDE, ou utilisateur final d'une application compilée) génère ses clés **sur son propre appareil**, au premier lancement. Aucune clé n'est jamais générée par un serveur de MCF Studio.
- **Paire d'identité :** Ed25519 (signature) + X25519 (accord de clé). Objectif : accord de clé hybride post-quantique (X25519 + ML-KEM-768) dès que la pile retenue le permet de façon stable.
- **Stockage :** trousseau du système (Windows Credential Manager / DPAPI, Trousseau macOS, Secret Service sous Linux) ou fichier chiffré par phrase de passe (Argon2id + XChaCha20-Poly1305). Clés matérielles (TPM, FIDO2) prévues ensuite.
- **Jamais en clair hors de l'appareil.** L'export d'une clé (sauvegarde, nouvel appareil) produit uniquement un fichier chiffré ou une phrase de récupération affichée localement.
- **Aucun séquestre, aucune clé maîtresse, aucune porte dérobée.** Il n'existe pas de « clé MCF » capable d'ouvrir les échanges des utilisateurs.

**Conséquence assumée :** une clé perdue sans sauvegarde = des données irrécupérables, y compris pour MCF Studio. L'IDE doit proposer la sauvegarde dès la création de la clé et l'expliquer sans jargon.

## 3. Modèle de permissions — qui voit quoi

| Acteur | Contenu des échanges | Métadonnées (IP, horaires, tailles) | Clés |
|---|---|---|---|
| L'utilisateur, sur son appareil | Oui | Oui | Oui (les siennes uniquement) |
| Le destinataire légitime d'un échange | Oui, uniquement ce qui lui est adressé | Partielles | Non |
| Le serveur d'une application (si l'application en a un) | Seulement si l'application est conçue pour qu'il soit destinataire ; avec les canaux de bout en bout, un relais ne voit rien | Oui | Non |
| Fournisseur d'accès, hébergeur, réseau | Non | Oui (voir limites) | Non |
| **MCF Studio** | **Non** | **Non** (aucun serveur sur le chemin, aucune télémétrie) | **Non** |

## 4. Le chiffrement dans le langage

Le chiffrement n'est pas une bibliothèque optionnelle : il est intégré au compilateur.

### 4.1. Le chemin simple est le chemin chiffré

Esquisse non normative (la syntaxe exacte du module `Secure` reste à arbitrer, cf. cahier des charges §24, point 14) :

```rust
import Secure;

func sendScore(_score: Int): Void {
    _channel: Secure.Channel = Secure.connect("game.example.org");
    _channel.send(_score);
}
```

Aucune option `encrypted: true` à penser : il n'existe pas de variante en clair dans l'API standard.

### 4.2. Le clair est une exception visible

Pour parler à un service ancien qui n'accepte que le clair :

1. Déclarer la permission `network.plaintext` dans le `.mcfproj`.
2. Le compilateur émet un avertissement à **chaque** compilation, listant les points du code concernés.
3. En mode `mcf check --strict`, c'est une erreur.

### 4.3. Le type `Secret<T>`

```rust
apiToken: Secret<String> = Secure.load("api_token");

print(apiToken);              // ERREUR MCF0300 : SECRET_EXPOSURE
_header: String = apiToken;   // ERREUR : pas de conversion implicite Secret<String> -> String
```

- Impossible d'afficher, de journaliser, de sérialiser en JSON ou d'envoyer en clair un `Secret`.
- La mémoire d'un `Secret` est effacée (zeroize) à sa destruction.
- L'accès à la valeur brute exige un appel explicite et repérable dans le code, signalé par les outils d'audit.

### 4.4. Diagnostics de sécurité réservés

| Identifiant | Nom | Cas |
|---|---|---|
| `MCF0300` | `SECRET_EXPOSURE` | Un `Secret` est passé à une sortie en clair |
| `MCF0301` | `PLAINTEXT_NETWORK` | Connexion non chiffrée (avertissement, erreur en strict) |
| `MCF0302` | `MISSING_PERMISSION` | Capacité utilisée sans être déclarée dans le `.mcfproj` |
| `MCF0303` | `UNSIGNED_DEPENDENCY` | Dépendance sans signature valide |

## 5. Primitives et bibliothèques

Pas de cryptographie maison. Choix envisagés (à confirmer à l'implémentation, toujours parmi des bibliothèques Rust maintenues et auditées) :

| Usage | Primitive | Bibliothèque envisagée |
|---|---|---|
| Transport vers l'extérieur | TLS 1.3 uniquement | `rustls` |
| Canal MCF ↔ MCF | Noise (XX / IK), secret persistant (*forward secrecy*) | `snow` |
| Chiffrement symétrique | XChaCha20-Poly1305 / AES-256-GCM | RustCrypto / `aws-lc-rs` |
| Signature | Ed25519 | `ed25519-dalek` |
| Dérivation depuis une phrase de passe | Argon2id | `argon2` |
| Effacement mémoire | zeroize | `zeroize` |
| Cible web | WebCrypto, clés non extractibles | API du navigateur |

## 6. Menaces et mesures

| Menace | Mesure |
|---|---|
| MCF Studio (ou quelqu'un qui le contraint) veut lire les échanges | Aucune clé chez MCF Studio, aucun serveur sur le chemin, aucune télémétrie, code public |
| Interception réseau (Wi-Fi public, FAI, attaquant actif) | TLS 1.3 / Noise authentifiés, pas de repli en clair |
| Vol de données passées après compromission d'une clé | Secret persistant (*forward secrecy*) via clés de session éphémères |
| Dépendance ou mise à jour piégée | Signatures vérifiées avant installation, refus si invalide |
| Binaire officiel modifié pour espionner | Code source public, objectif de compilations reproductibles |
| Développeur qui oublie de chiffrer | Pas d'API en clair par défaut, permission + avertissement + refus en strict |
| Fuite d'un secret dans les logs | Type `Secret<T>` vérifié à la compilation |
| Vol du fichier de clés | Fichier chiffré (Argon2id) ou trousseau du système / matériel |
| Ordinateur quantique futur (« stocker maintenant, déchiffrer plus tard ») | Objectif : accord de clé hybride X25519 + ML-KEM-768 |

## 7. Limites — ce que le chiffrement ne protège pas

Être honnête sur les limites fait partie de l'engagement :

- **Les métadonnées** : un observateur réseau voit qui communique avec qui, quand et en quelle quantité, même s'il ne voit pas le contenu.
- **Un appareil compromis** : un logiciel espion sur la machine de l'utilisateur voit ce que l'utilisateur voit.
- **Le code du développeur** : un développeur peut écrire une application qui envoie volontairement des données à son propre serveur. MCF garantit que *MCF Studio* ne voit rien et que rien ne passe en clair, pas que chaque application écrite en MCF est honnête.
- **La cible web** dépend de la sécurité du navigateur et du serveur qui distribue le code.
- **Statut actuel** : ces garanties sont une spécification. Elles ne pourront être affirmées comme acquises qu'une fois implémentées, testées et auditées.
