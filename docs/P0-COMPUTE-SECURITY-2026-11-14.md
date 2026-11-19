# P0 Compute — confidentialité de bout en bout, sandbox et vérification

> **Instantané daté — 2026-11-14, corrigé et complété le 2026-11-15.** Ce
> document décrit l'état vérifié à cette date et ne remplace pas `STATE.md` ni
> `ETAT-REEL.md`. Les résultats physiques entre deux machines restent bloqués ;
> aucun test local n'est présenté comme une validation inter-hôtes.

## 1. Chiffrement de bout en bout des demandes et résultats

### Protocole

Le chemin Compute utilise désormais quatre trames, toutes signées par ML-DSA-65
et liées à une session aléatoire :

1. `kem_hello` : le demandeur envoie sa clé publique KEM et son identité signée.
2. `kem_offer` : le ghost prouve son identité et renvoie sa clé publique
   ML-KEM-1024, signée. Le relais ne peut pas substituer cette clé sans casser
   la signature et le `node_id`.
3. `req` : le demandeur encapsule un secret ML-KEM-1024 vers la clé du ghost,
   dérive une clé AES-256-GCM, chiffre la tâche, puis signe le ciphertext.
4. `grant` : le ghost décapsule, exécute, chiffre le résultat avec la même clé,
   puis signe le ciphertext.

Format exact : `ComputeEnvelope { v: "polygone-compute-e2e-v1", kem_ct, body }`
où `body = nonce[12] || AES-256-GCM(ciphertext + tag)`.

Le relais voit `from`, `to`, `session`, `type`, la taille et le nombre de
trames. Il ne voit ni la tâche, ni la sortie, ni le nom d'un fichier. Un grant
en clair, une version inconnue ou un `kem_ct` absent sont refusés : il n'existe
pas de repli silencieux vers le plaintext.

### Preuves exécutées

| Commande / test | Résultat |
|---|---|
| `cargo test -p polygone-client --bin polygone net::tests::compute_e2e_hides_task_and_result_from_the_relay` | **1 réussi** |
| `cargo test -p polygone-client --bin polygone net::tests::grant_reply_routes_back_and_seals_the_final_output_payload` | **1 réussi** |
| `cargo test -p polygone-client --bin polygone net::tests::` | **46 réussis, 0 échec** |
| `cargo test --workspace --no-fail-fast` | **554 réussis, 0 échec** |

Le test principal construit une tâche et un résultat marqués, vérifie que les
octets sur le fil ne contiennent ni l'un ni l'autre, puis ouvre la demande avec
le ghost et le grant avec la clé dérivée. Il vérifie aussi qu'une autre identité
ne peut pas ouvrir la demande et qu'un grant en clair est rejeté.

### Limites honnêtes

- Le relais voit les métadonnées (qui, quand, combien, quelle taille). Ce n'est
  pas un réseau à métadonnées nulles.
- La signature du ghost prouve l'auteur du résultat, **pas sa justesse**.
- La clé KEM du ghost est liée à son identité par la signature de l'offre, pas
  par un certificat séparé.
- Aucun test réel à travers un relais intercepté par `tcpdump` n'a encore été
  exécuté entre deux hôtes. Le test actuel prouve la construction et le refus
  du plaintext, pas l'absence de fuite dans un déploiement physique.

### Rotation et révocation de clé (2026-11-15)

Le `node_id` est dérivé de la **clé de signature**
(`node_id_from_sign_pk_hex`), pas de la clé KEM. Une rotation change donc
l'adresse du nœud : un grant signé par la nouvelle clé ne peut pas revendiquer
l'ancien `node_id`, et les pairs qui détenaient l'ancienne clé refusent les
nouveaux grants jusqu'à réapprentissage. Preuve :
`net::tests::key_rotation_changes_the_node_id_and_old_grants_stay_with_the_old_key`.

**Limite mesurée, non corrigée** : la révocation est **locale**
(`accepts_signature_from`). Aucune liste de révocation n'est distribuée, donc
l'ancienne clé reste mathématiquement valide pour l'ancien `node_id` — un
détenteur de l'ancienne clé peut continuer à parler sous l'ancienne adresse. Une
révocation distribuée est un changement de protocole, non décidé ici.
L'interruption en cours de session (demandeur qui disparaît entre `kem_hello` et
`req`) n'est pas testée : elle exige le déploiement deux hôtes.

## 2. Sandbox réellement exécutée

L'hôte dispose de systemd 261 et de `systemd-run --user`. Le probe de test
utilisait un nom d'unité fixe et sautait les tests à tort ; il utilise maintenant
un nom unique et `--collect`.

| Contrôle | Résultat observé (2026-11-15) |
|---|---|
| Tests `exec::tests::` | **12 réussis, 0 échec** |
| Commande simple sandboxée | `echo hello-res` → sortie correcte |
| Code de sortie non nul | échec explicite, code 23 conservé |
| Timeout de 1 s sur `sleep 10` | arrêt en **1 296 ms**, aucune unité restante |
| Accès à `$HOME/.polygone/identity.json` | **BLOCKED** |
| Réseau privé | `/proc/net/dev` ne montre que `lo:`, pas `wlp*`/`eth*` |
| `CPUQuota=10%` | `CPUQuotaPerSecUSec=100ms` observé |
| Effet réel du quota CPU | 48 Mio SHA-256 : **3,11 s à 100 % → 13,04 s à 10 % (4,2x)** ; seconde mesure 2,63 s → 13,06 s (**5,0x**) |
| cgroup mémoire vu depuis la sandbox | `memory.max = 268435456` pour 256 Mo demandés, `memory.swap.max = 0` |
| `RLIMIT_AS` dans le shell | `ADDRESS_SPACE_KB = 262144` pour 256 Mo ; `ulimit -v` hérité par les enfants |
| 96 Mo touchés sous 32 Mo | **échec** (code non nul) |
| Unité transitoire 96 Mo / 32 Mo | tuée, aucune unité `polygone-res-*` résiduelle |

### Correction du 2026-11-15 : la limite mémoire *est* appliquée

La session du 14 novembre avait écrit que « `MemoryMax=32M` n'était pas appliqué
par le cgroup sur cet hôte ». **C'était faux.** La sonde manquait
`MemorySwapMax=0` : le swap absorbait l'allocation, si bien qu'une allocation de
96 Mo survivait à un plafond de 32 Mo. Mesures discriminantes rejouées hors
`RLIMIT_AS` :

| Sonde | Résultat |
|---|---|
| 16 Mo touchés, plafond 32 Mo | **survit** (`exit 0`) |
| 96 Mo touchés, plafond 32 Mo, `MemorySwapMax=0` | **tué** (`exit 1`, aucune sortie) |
| 96 Mo touchés, **sans** plafond | **survit** (`exit 0`) |

`MemorySwapMax=0` est désormais posé par `run_sandboxed`, et le test
`sandbox_memory_cap_is_configured_in_the_cgroup_and_enforced` vérifie les deux
barrières. `RLIMIT_AS` reste en défense en profondeur pour les hôtes dont le
contrôleur mémoire n'est pas délégué au gestionnaire utilisateur.

Le quota CPU n'est plus seulement « configuré et visible » : le test
`sandbox_cpu_quota_slows_a_cpu_bound_workload` exécute le même travail borné à
100 % puis à 10 % et exige un ralentissement d'au moins 1,5x. Mesuré 4,2x et
5,0x sur deux exécutions.

La sandbox reste une isolation systemd/wasmi à UID partagé, pas un bac à sable
cryptographique. `ProtectHome`, `PrivateNetwork`, `PrivateDevices`,
`NoNewPrivileges`, `ProtectSystem=strict`, `RuntimeMaxSec`, `MemoryMax`,
`MemorySwapMax=0` et `RLIMIT_AS` sont les barrières réellement posées.

## 3. Vérification indépendante — niveaux et limites

Chaque grant porte maintenant :

```json
{
  "verification": "producer-signed",
  "verification_limit": "la signature prouve l'auteur, pas la justesse du résultat"
}
```

Niveaux définis :

| Niveau | Garantie | État |
|---|---|---|
| **L0 — producer-signed** | Le nœud qui a produit le résultat l'a signé ; l'identité est vérifiée. | **Implémenté**, niveau par défaut. |
| **L1 — recalcul indépendant** | Le demandeur réexécute la même tâche déterministe et compare. | **Implémenté en opt-in** : `--verifier-local` avec `--executer`. |
| **L2 — quorum** | Plusieurs opérateurs indépendants exécutent et renvoient le même hash ; le quorum est vérifié. | Non implémenté. Un quorum de nœuds contrôlés par le même opérateur ne prouve rien. |
| **L3 — preuve d'exécution / TEE** | Preuve vérifiable ou enclave attestée. | Non implémenté ; aucun TEE disponible sur l'hôte. |

### L1 en pratique

`net::annotate_local_recomputation` compare la sortie du grant à la sortie du
recalcul local et ajoute au grant :

```json
{
  "verification": "recomputed-local",
  "recompute": {
    "level": "recomputed-local",
    "match": true,
    "grant_output_blake3": "…",
    "local_output_blake3": "…"
  }
}
```

- Concordance : `verification` passe à `recomputed-local`, la limite dit
  explicitement que cela ne protège pas d'un bug partagé.
- Divergence : `verification` **reste** `producer-signed`, `match` vaut `false`,
  les deux empreintes sont conservées, la commande sort en erreur et le
  producteur est enregistré en échec de réputation.
- Refus du producteur : aucun niveau n'est ajouté, il n'y a rien à recalculer.

Limites : le recalcul coûte le même calcul que celui qu'on voulait déléguer ;
il détecte un producteur menteur ou fautif, pas un bug présent dans le même
binaire sur les deux hôtes ; ce n'est pas une seconde implémentation.

**Règle produit** : un résultat L0 ne doit jamais être affiché comme « vérifié »
ou « fiable ». Il est « signé par le producteur ». Le champ est visible dans la
sortie CLI.

## 4. Planificateur

Aucune distribution n'a été ajoutée. Compute ne mesure pas encore le RTT, le
débit ou la disponibilité entre deux pairs : les bornes CPU/mémoire sont
désormais mesurées sur un hôte, mais rien ne l'est **entre** hôtes. Les seules
mesures réseau existantes sont loopback (27 ms observés / 37 ms mur sur le
prototype Fabric). Porter le planificateur maintenant créerait une précision
fictive.

## 5. Validation physique et supervision

- **Deux machines : bloqué.** Un seul hôte Linux avec shell ; l'iPhone actif sur
  Tailscale n'exécute pas le binaire. Le protocole Compute est ajouté à
  `docs/P0-DEUX-MACHINES.md`, mais son tableau de preuves reste vide.
- **Supervision : non commencée.** Aucun endpoint JSON, aucune PWA, aucun
  compteur simulé. `polygone compute` continue d'afficher les mesures réelles
  existantes (RAM libre, pairs, réputation), sans latence inventée.
- **Babel : aucun code.** `DEC-028` reste ouverte ;
  `docs/prototypes/BABEL-RESEARCH-2026-11-14.md` reste la référence.

## 6. État P0

| Critère | État |
|---|---|
| Un seul Compute visible | Oui |
| Chiffrement E2E des tâches/résultats | Oui, testé localement |
| Rejeu/authentification | Oui, tests ciblés |
| Sandbox temps réel | Oui, 12 tests réels |
| Limite mémoire prouvée | Oui — cgroup (`memory.max`, `memory.swap.max=0`) **et** `RLIMIT_AS` |
| Limite CPU mesurée | Oui — ralentissement 4,2x à 5,0x sous `CPUQuota=10%` |
| Vérification indépendante | L1 opt-in pour tâches déterministes ; L2/L3 non |
| Rotation/révocation | Rotation testée (nouveau `node_id`) ; révocation distribuée non |
| Deux machines physiques | Non |
| Release | **Non** |

**Conclusion : P0 n'est pas satisfait.** Confidentialité E2E, sandbox, quota
CPU et vérification L1 disposent de preuves reproductibles, mais il manque la
validation inter-hôtes, la capture du relais réel et le quorum indépendant.
