# PLAN DE FINITION — terminer Polygone et chaque brique de l'écosystème

> NOTE DE PORTAGE — 4 novembre 2026 : spécification des travaux à réaliser, pas un rapport d’avancement ; les états terminés sont tenus dans ../agent/backlog.json.

> Rédigé le 2026-11-03. **Inventaire de travail, pas calendrier.** 132 tâches
> réparties en 15 épiques, chacune avec une preuve nommée. Aucun délai n'est
> promis : une tâche dure ce qu'elle dure, et elle n'est finie que quand sa
> preuve a été exécutée et citée.
>
> Les épiques détaillés sont dans [`epiques/`](epiques/). Ce document dit
> **dans quel ordre**, **pourquoi**, et **ce qui bloque**.
>
> **Ce document ne couvre que le cœur de Polygone.** Le gateway, l'agent, le
> corpus, les 15 versions et l'Atlas lui-même sont dans
> [`PLAN-ECOSYSTEME.md`](PLAN-ECOSYSTEME.md), qui contient aussi l'ordre
> d'exécution entrelacé des six briques (vagues W0…W5).

---

## 1. Ce que « fini » veut dire

Polygone n'est pas fini quand il compile, ni quand la documentation le décrit.
Il est fini quand ces six phrases sont vraies, chacune avec sa preuve :

| # | Phrase | Preuve acceptée |
|---:|---|---|
| F1 | Une personne non technique installe, appaire deux appareils et échange un message | transcript d'une session réelle, VM neuve, aucun terminal avancé |
| F2 | Deux **machines physiques** distinctes échangent un message et un fichier, à travers un pare-feu réel | journal horodaté avec deux hôtes nommés, transfert comparé octet à octet |
| F3 | Une charge consommatrice est **contrainte par l'OS**, et la restitution est prouvée | `cgroup` vérifié pendant l'exécution, scope déchargé après, zéro résidu |
| F4 | Chaque capacité annoncée publiquement a un test qui échoue si elle disparaît | test de cohérence documentaire vert, et rouge quand on casse une affirmation |
| F5 | Un nœud qui meurt sans rendre son bail ne bloque aucune ressource au-delà de son bail | test de crash : bail expiré, ressources récupérées, jamais de fuite |
| F6 | Un tiers peut vérifier une release (empreinte, SBOM, provenance) et refuser le reste | `sha256sum -c` + SBOM attaché ; la phrase publique dit « empreinte vérifiée », jamais « release signée » |

**F1 et F2 sont les deux qui manquent depuis le début.** Tout le reste est
nécessaire pour qu'elles tiennent dans le temps, pas pour les atteindre.

---

## 2. Les 15 épiques

Chiffres = nombre de tâches. « État » = ce qui existe réellement aujourd'hui,
d'après `ETAT-REEL.md`.

| Épique | Module | Tâches | État aujourd'hui | Fichier |
|---|---|---:|---|---|
| **MSG** | messagerie éphémère | 7 | partiel — TTL annoncé 30 s, réel 300 s côté session | [`01`](epiques/01-transport-confidentialite.md) |
| **HIDE** | proxy SOCKS5 | 6 | mono-saut prouvé, multi-hop absent | [`01`](epiques/01-transport-confidentialite.md) |
| **MESH** | découverte LAN | 4 | broadcast sur toutes les interfaces, y compris VPN | [`01`](epiques/01-transport-confidentialite.md) |
| **DRIVE** | stockage distribué | 9 | module **non monté**, zéro appelant | [`02`](epiques/02-stockage.md) |
| **BLOCKS** | magasin de blocs | 10 | branché, sans chiffrement au repos | [`02`](epiques/02-stockage.md) |
| **DAEMON** | bail et isolation | 13 | bail sans expiration, `nice` seul | [`03`](epiques/03-calcul-ia.md) |
| **PETALS** | inférence distribuée | 5 | planner sans consommateur | [`03`](epiques/03-calcul-ia.md) |
| **SRV** | serverless | 6 | reconstitue localement, aucun nœud contacté | [`03`](epiques/03-calcul-ia.md) |
| **CLI** | produit en une commande | 7 | commandes présentes, pas de `doctor` | [`04`](epiques/04-interfaces.md) |
| **GW** | passerelle HTTP | 7 | statut = table mémoire, pas de heartbeat | [`04`](epiques/04-interfaces.md) |
| **UI** | tableau de bord / PWA | 7 | **simulation** encore active dans `dashboard.js` | [`04`](epiques/04-interfaces.md) |
| **INST** | installation | 9 | fail-closed **déjà** en place, test à 5 modes | [`05`](epiques/05-livraison.md) |
| **SDK** | intégration | 8 | API locale non gelée, pas de client externe | [`05`](epiques/05-livraison.md) |
| **CI** | intégration continue | 10 | matrice Windows bloquante alors que le code refuse Windows | [`05`](epiques/05-livraison.md) |
| **SEC** | sécurité prouvée | 10 | zéro fuzzing, anti-rejeu testé en unitaire seulement | [`06`](epiques/06-confiance.md) |
| **OPS** | observabilité | 6 | motifs de refus sans compteurs | [`06`](epiques/06-confiance.md) |
| **DOC** | vérité documentaire | 8 | ~34 documents, certains faux, aucun test de cohérence | [`06`](epiques/06-confiance.md) |

**Règle de lecture :** un « constat » d'épique vient d'une **lecture du code**,
pas d'une exécution. La première action de chaque tâche est de **reproduire son
constat**. Si le constat est faux, on corrige l'épique avant de coder — trois
constats se sont déjà révélés faux pendant la rédaction (§8).

---

## 3. Les six portes

Une porte est un ensemble de preuves ; rien de la porte suivante ne commence
avant. Elles ne sont pas des phases calendaires.

### G0 — Ne rien perdre, ne rien mentir *(franchie le 2026-11-03)*
- ✅ P0.1 archive restaurable · ✅ P0.2 inventaire des 23 commits · ✅ P0.3 docs mesurées
- ✅ `DOC.1` — `scripts/check-docs.mjs` recalcule les chiffres publiés, recompte
  les tableaux de `ETAT-REEL.md`, vérifie les preuves citées et refuse les
  revendications fausses. Test négatif fait : un chiffre cassé fait échouer.
- ✅ `UI.5` — « libp2p with Kad DHT », « Five tabs », « Sparklines » retirés des
  deux pages publiques ; elles décrivent maintenant le TCP + relais et les six
  écrans réels.

**Sortie atteinte :** aucun document ne contredit le code sans faire échouer un
commande. `node scripts/check-docs.mjs --tests N` est cette commande.

### G1 — La ressource est contrainte, pas promise *(F5 acquise, F3 en attente de D2)*
`DAEMON.1` ✅ → `DAEMON.3` ✅ → `DAEMON.13` ✅ · `DAEMON.10` ⬜ (bloqué par D2) · puis `OPS.1`, `CLI.1`.

L'**expiration** était le chemin critique : `expires_at` valait `None`, donc un
processus mort gardait sa réservation pour toujours. Les baux portent maintenant
une échéance de 15 min (ou `POLYGONE_LEASE_SECS`), balayée avant chaque décision,
et `renew` existe pour les consommateurs vivants. `scripts/crash-recovery.mjs`
le prouve sur de vrais processus : SIGKILL du daemon, bail durable, marge rendue
à l'échéance, redémarrage qui répond — 9/9 PASS.

**Reste pour Fermer G1 :** `DAEMON.10` (`--isoler` par défaut) est bloqué par la
décision **D2** ; c'est elle qui manque à F3, pas le code.

### G2 — Deux machines, un vrai réseau
`BLOCKS.9` et `MSG.6` d'abord — ce sont les moins coûteux et ils prouvent le
transport réel. Puis `MESH.1` (ne pas diffuser sur le VPN), `SEC.4`, `SEC.3`.

**Sortie :** F2 vraie. Un journal avec deux hôtes, un pare-feu, un transfert
comparé. Sans cette porte, aucun service distribué n'est démontrable.

### G3 — Le parcours utilisateur
`CLI.1` → `CLI.2` → `CLI.5` → `INST.2` → `INST.9` → `UI.1` → `UI.2` → `UI.4`.

**Sortie :** F1 vraie. Une personne non technique, une VM neuve, zéro terminal.

### G4 — Les services, dans l'ordre des dépendances
`SRV.1` → `SRV.2` → `SRV.4` (serverless réel) · `PETALS.2` → `PETALS.3` (la
mesure qui peut tout invalider) · `DRIVE.1` (gate) → `DRIVE.3` → `DRIVE.5` →
`DRIVE.6` · `HIDE.3` (multi-hop).

**Règle :** un service n'entre pas en G4 sans son modèle de menace (`SEC.1`).
`PETALS.3` peut conclure « le transfert coûte plus que le calcul gagné » — c'est
un résultat acceptable, et il faut alors retirer la promesse au lieu d'insister.

**Sortie :** chaque service a un mode dégradé écrit, et un refus explicite quand
il ne peut pas tenir sa promesse.

### G5 — Publiable
`CI.1` → `CI.3` → `CI.6` → `CI.8` · `SEC.7`, `SEC.8`, `SEC.9` · `INST.6`,
`INST.8` · `DOC.2` → `DOC.7`.

**Sortie :** F4 et F6 vraies.

---

## 4. La chaîne critique, en une ligne

```
DAEMON.1 → DAEMON.3 → BLOCKS.9 → MSG.6 → MESH.1 → CLI.1 → CLI.2 → INST.9 → UI.1 → UI.2 → F1+F2
```

12 tâches. C'est le plus court chemin vers un produit qu'on peut montrer à
quelqu'un sans mentir. **Tout le reste peut attendre** — et c'est le seul
classement qui compte quand le temps manque.

---

## 5. Les décisions qui ne m'appartiennent pas

Neuf points bloquent du travail. Chacun est marqué `GATE-LÉVY` dans son épique.
Tant qu'ils sont ouverts, la tâche correspondante reste bloquée, pas « en cours ».

| # | Question | Bloque | Conséquence si on ne tranche pas |
|---:|---|---|---|
| D1 | **Licence : AGPL confirmée ou retour à MIT ?** | toute publication | deux licences coexistent dans l'historique ; on ne publie pas |
| D2 | **`--isoler` devient-il le défaut sous Linux ?** | `DAEMON.10`, `CLI.4` | le comportement change au premier lancement ; c'est une décision produit |
| D3 | **Drive redevient-il un module monté, et avec quel service minimal ?** | `DRIVE.1` et les 8 suivantes | 9 tâches restent bloquées sur un mot |
| D4 | **Périmètre mDNS / Bluetooth / Wi-Fi Direct** | `MESH.3` | on continue de promettre Bluetooth sans code |
| D5 | **File hors ligne de la messagerie** | `MSG.3` | un nœud qui stocke change le modèle de menace |
| D6 | **Politique de logs du relais** (il journalise les IP) | `SEC.6` | `LEGAL.md` promet « pas de serveur central » ; écart juridique |
| D7 | **Gateway exposée sur l'internet, ou loopback seulement ?** | `GW.7`, `SEC.1` | change tout le modèle de menace public |
| D8 | **Plafond de dette et de crédit de l'unité de réciprocité** | `DAEMON.12` | sans plafond, thésaurisation ; sans dette, le réseau ne démarre pas |
| D9 | **Cible de distribution** (`.deb` seul ? conteneur ? brew ?) | `INST.7`, `CI.1` | on teste des paquets que personne n'utilisera |

Les décisions D3, D5, D7 et D8 sont les plus coûteuses : chacune bloque entre 3
et 9 tâches. Ce sont aussi les plus courtes à répondre — une phrase suffit.

---

## 6. Comment le travail s'exécute

Règles pour l'orchestrateur et pour tout agent qui prend une tâche.

1. **Reproduire avant de corriger.** La tâche commence par exécuter la preuve
   *attendue* et constater qu'elle échoue. Un correctif sans échec reproduit
   corrige peut-être autre chose.
2. **Une tâche = une preuve citée.** Sortie de commande, nom de test, mesure.
   « Ça marche » ferme la tâche sans la finir.
3. **Ne jamais assouplir une assertion pour passer au vert.** Si un garde-fou
   bloque (le test anti-cgroup l'a fait pendant P1.1), on corrige le code ou la
   prose, pas le garde-fou.
4. **Deux agents ne touchent jamais le même fichier** en parallèle. Les épiques
   sont découpés pour que ce soit vrai ; sinon sérialiser.
5. **Pas plus de 2 tâches en cours.** La troisième est un signe que la première
   est mal découpée.
6. **Un commit = une tâche = une preuve.** Le message de commit cite la preuve.
7. **Signaler, pas contourner.** Une décision manquante se marque `blocked` avec
   la question posée ; elle ne se tranche pas à la place de Lévy.
8. **Le travail des autres est relu en entier**, pas par morceau : six épiques
   écrits séparément peuvent se contredire, et c'est arrivé (§8).

---

## 7. Comment on mesure l'avancement

Pas en tâches cochées — c'est une mesure d'activité, pas de résultat. Trois
compteurs, recalculés à chaque session et écrits dans `STATE.md` :

| Compteur | Aujourd'hui | Cible |
|---|---:|---:|
| Capacités **VRAIES** (`ETAT-REEL.md` §2) | 11 / 33 | 33 / 33 |
| Capacités **FAUSSES** (§3) | 18 | **0** — par correction ou par retrait de la promesse |
| Portes franchies (G0…G5) | 0 sur 6 (G0 à 3/4) | 6 |

**Le compteur qui compte est le deuxième.** Faire passer une capacité de « faux »
à « vrai » vaut mieux que d'en ajouter une dix-neuvième. Et retirer une promesse
fausse compte comme un progrès : c'est une dette de vérité remboursée.

---

## 8. Corrections apportées pendant la rédaction

Ces épiques ont été rédigés en six passages parallèles. Les relectures ont déjà
invalidé trois constats — c'est le signe que la règle « reproduire avant de
corriger » est nécessaire :

1. **L'installateur n'a pas besoin d'être repris.** `install.sh:155-177` est
   **déjà** fail-closed (refus sans `SHA256SUMS`, format 64 hex exigé, refus si
   aucun outil de hachage), et `scripts/test-installer.sh:201-217` exerce
   **cinq** modes d'échec. La version locale est plus forte que le patch distant.
   `RECONCILIATION.md` §2 A1 a été corrigé en conséquence : **2 éléments à
   reprendre, pas 3**.
2. **La limite de 64 Kio de `DriveFile::receive()` appartient au code distant**,
   pas au local. Le `drive.rs` local n'a pas de `receive()` du tout — il est
   simplement non monté. Les vrais défauts locaux sont l'absence de découpage en
   blocs et un `sender: NodeId([0u8; 16])` en dur.
3. **Un commentaire du code contredit son propre test** : `socket.rs` affirme que
   `reserved_storage_bytes` « vit en mémoire » et qu'un redémarrage l'oublie,
   alors que le champ est sérialisé et que le test le relit à l'identique. D'où
   `DAEMON.4` / `BLOCKS.1`, et la leçon : un commentaire n'est pas une preuve.

**Trois tâches sont en P0 et non en P4 pour cette raison** : elles ne construisent
rien, elles retirent un mensonge. C'est le meilleur rapport effort/effet du plan.

---

## 9. Ce que ce plan ne fait pas

- Il ne promet **aucune date**. 132 tâches, c'est un inventaire, pas un engagement.
- Il ne planifie pas **Windows** (non supporté, `ETAT-REEL.md` §6) ni
  **macOS** au-delà de « expérimental ».
- Il ne rouvre **pas** POLY comme monnaie (`DECISION-POLY.md`) ; l'unité de
  réciprocité reste `RECIP.1`, non transférable, et conditionnée à G2.
- Il ne planifie **pas** le téléphone-nœud, Hide multi-hop au-delà de 2 sauts,
  l'OS Polygone, la marketplace, ni « mieux que Tor ».
- Il ne remplace pas `ROADMAP.md` : il le détaille. Les identifiants `P0.x` à
  `P5` restent la référence longitudinale ; les identifiants d'épique
  (`DAEMON.1`) sont la référence de travail.