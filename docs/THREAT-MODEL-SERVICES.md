# Modèle de menace par service

> SEC.1 — exigence `FV-SEC-08` du registre `FOUNDER_VISION.md`.
> Ce document ne remplace pas les modèles par audience
> (`docs/threat-commodity.md`, `docs/threat-high-value.md`) : il les applique
> **service par service**, avec la surface d'attaque réellement exposée par le
> code de ce dépôt. Un tiers doit pouvoir contredire chaque promesse ci-dessous
> en lisant le code cité.
>
> Date : 2026-11-05. Aucun audit externe n'a été réalisé : ce document est une
> auto-évaluation, pas une certification.

## 1. Méthode

Pour chaque service : actifs protégés, adversaires, points d'entrée, frontière
de confiance, attaques considérées, contrôles **présents dans le code** avec
leur preuve, risques résiduels, et ce que le service ne protège pas.

Règle de rédaction : un contrôle n'est listé que s'il a un test ou un chemin de
code identifiable. Une intention sans code est dans « risque résiduel ».

## 2. Vue d'ensemble

| Service | Binaire / crate | Exposition réseau | Actif principal |
|---|---|---|---|
| Client local | `polygone` (`crates/client`) | sortant seulement (relais, pairs, Ollama local) | identité ML-DSA, clés de session, fichiers reçus |
| Relais | `polygone-relay` (`crates/relay`) | TCP entrant, port public | routage de fragments, disponibilité |
| Gateway | `polygone-gateway` (`crates/gateway`) | HTTP entrant (réseau local) | site statique, comptes de nœuds appairés, mesures |
| Daemon | `polygoned` (`daemon/`) | socket Unix local | allocations CPU/RAM/BW, décisions de refus |
| Calcul serverless | `crates/serverless` + client | via relais | intégrité des entrées/sorties de tâches |
| Pétales IA | `crates/petals` + client | HTTP local (`127.0.0.1:11434`) | prompts, jamais envoyés au cloud |

## 3. Client local — `polygone`

**Actifs.** Clé privée ML-DSA/KEM (`~/.polygone/identity.json`, chmod 600),
fichiers reçus, ancres `peers.json`, carnet de réputation.

**Adversaires.** Malware local, colocataire avec accès au compte, FAI, relais
curieux, pair malveillant, collecteur « harvest now, decrypt later ».

**Points d'entrée.** Arguments CLI, fichiers d'entrée, messages et fragments
reçus du relais, réponses des pairs, fichiers de configuration locaux.

**Frontière de confiance.** Tout ce qui vient du réseau est non fiable jusqu'à
vérification ML-DSA et déchiffrement ML-KEM/AES-GCM. Le disque local est
partiellement fiable : l'identité est lisible par le compte utilisateur.

**Attaques et contrôles.**

| Attaque | Contrôle dans le code | Preuve |
|---|---|---|
| Lire le contenu en transit | ML-KEM-1024 + AES-256-GCM, relais aveugle | tests `crates/core`, `crates/client/src/net` |
| Usurper l'expéditeur | signature ML-DSA-65 vérifiée avant déchiffrement, `from` lié au KEM | tests réseau, `SEC.4` |
| Rejouer une session | `ts` signé, fenêtre ±300 s, cache des sessions complétées | `REPLAY.SESSION_ALREADY_COMPLETED`, `REPLAY.STALE_TIMESTAMP` |
| Voler un nom de fichier | `name_ct` chiffré, hors bande | tests drive |
| Premier contact substitué (TOFU) | empreinte affichée à l'écoute, clé différente pour un `from` connu = rejet | `ARCHITECTURE.md` §11 |
| Fuite de secrets dans le dépôt | test `no_tracked_secrets` sur `git ls-files` | `SEC.8` |

**Risques résiduels.** Keylogger ou malware déjà présent avant chiffrement ;
coercition physique ; métadonnées de timing et de taille ; corrélation du
`node_id` stable ; `identity.json` en clair au repos (assumé, `duress` efface).

**Ne protège pas.** Un adversaire qui contrôle la machine au moment de l'usage,
ni la révélation humaine.

## 4. Relais — `polygone-relay`

**Actifs.** Disponibilité du routage, intégrité de la table de sessions,
absence de contenu lisible.

**Adversaires.** Client abusif, usurpateur de `from`, observateur réseau,
opérateur du relais lui-même (curieux ou sous subpoena).

**Points d'entrée.** Connexions TCP, trames HELLO/ROUTE/DROP, identifiants de
session.

**Frontière de confiance.** Le relais est **aveugle au contenu** par
construction, mais voit `from`, `to`, session, tailles et timings. Il ne doit
jamais recevoir de clé.

**Attaques et contrôles.**

| Attaque | Contrôle | Preuve |
|---|---|---|
| Trame géante / saturation mémoire | plafond 64 KiB par trame | tests relais |
| Déni de service par flood | 200 enveloppes/s par connexion | tests relais |
| Usurpation de `from` | `from` doit égaler le HELLO de la connexion | `SEC.4` |
| Vol silencieux de slot | HELLO dupliqué refusé, `HELLO_OK`/`HELLO_DENIED` explicites | tests relais |
| Lecture du payload | aucune clé côté relais, fragments opaques | audit de démo |
| Rejeu | rejet au destinataire, pas au relais (choix assumé) | tests réseau |

**Risques résiduels.** HELLO non authentifié cryptographiquement (l'authenticité
vit chez le destinataire) ; métadonnées de graphe social ; corrélation
relais + sortie Hide en mono-saut.

**Ne protège pas.** Un opérateur qui conserve les métadonnées, ni un adversaire
qui observe les deux extrémités du chemin.

## 5. Gateway — `polygone-gateway`

**Actifs.** Site statique servi, table des nœuds appairés et de leurs clés
publiques, mesures de latence, disponibilité locale.

**Adversaires.** Machine du réseau local non appairée, nœud appairé malveillant,
observateur passif du LAN, opérateur de la gateway.

**Points d'entrée.** HTTP sur le LAN : `/api/v1/pair/*`, `/api/v1/nodes/heartbeat`,
`/api/v1/nodes/ping`, `/api/v1/nodes/pong`, `/api/v1/network/status`, fichiers
statiques.

**Frontière de confiance.** Seule une clé **épinglée à l'appairage** signe au
nom d'un nœud. Le corps d'une requête ne peut jamais fournir sa propre clé.

**Attaques et contrôles.**

| Attaque | Contrôle | Preuve |
|---|---|---|
| Traversée de chemin sur les fichiers statiques | normalisation et refus des `..` | `gateway_answers_real_http_requests_and_rejects_path_traversal` |
| Framing HTTP ambigu (smuggling) | refus avant routage | `ambiguous_http_framing_is_rejected_before_routing` |
| Corps déclaré surdimensionné | 413 sans lire le corps | `declared_body_over_one_mib_returns_413_without_reading_it` |
| Heartbeat forgé / rejoué | ML-DSA-65, clé épinglée, ±300 s | `heartbeat_requires_valid_signature`, `heartbeat_outside_replay_window_is_refused` |
| Latence inventée | pong accepté seulement après un ping servi, challenge unique, TTL 30 s, RTT ≤ 60 s | `latency_populated_after_pong`, `pong_without_ping_is_refused`, `pong_replay_is_refused` |
| Écrasement de `nodes.json` | écriture atomique + verrou de sérialisation | `heartbeat_survives_process_restart` |

**Risques résiduels.** Un nœud appairé peut mentir sur **sa propre** latence :
la gateway garantit la fraîcheur et la signature, pas l'honnêteté de la mesure
(c'est le nœud qui voit son chemin réseau). Le challenge de ping vit en mémoire :
un redémarrage annule les pings en attente. Le site servi n'a pas de CSP : il
est statique et sans entrée utilisateur, mais un fichier déposé sur disque est
servi tel quel. Pas de TLS : la gateway suppose un LAN de confiance, les
signatures protègent l'intégrité, pas la confidentialité du trafic local.

**Ne protège pas.** Un adversaire qui contrôle la machine hôte de la gateway, ni
la confidentialité des métadonnées vis-à-vis du LAN.

## 6. Daemon — `polygoned`

**Actifs.** Décisions d'allocation, compteurs de refus, intégrité des ressources
de la machine.

**Adversaires.** Processus local d'un autre utilisateur, client local bogué,
tâche RES qui tente de sortir de son bac à sable.

**Points d'entrée.** Socket Unix (`daemon/src/socket.rs`), requêtes d'allocation
et de statut.

**Frontière de confiance.** Le socket est local ; les demandes sont validées
contre les mesures réelles, pas contre les déclarations du client.

**Attaques et contrôles.**

| Attaque | Contrôle | Preuve |
|---|---|---|
| Allocation au-delà du mesuré | refus `AllocDenied` avec motif, compteur serveur | `OPS.2`, session réelle 100000 Gio |
| Tâche lisant `$HOME` | `ProtectHome=yes`, `InaccessiblePaths=~/.polygone`, `PrivateDevices`, filtre syscalls | tests `exec.rs` |
| Boucle WASM infinie | fuel metering wasmi | tests `exec.rs` |
| Unité orpheline après timeout | arrêt explicite de l'unité nommée | tests `exec.rs` |
| Compteurs non mesurés présentés comme réels | `refusals_request`, statut « non mesuré » quand le daemon est arrêté | `test_status_probe_does_not_hang_on_silent_daemon` |

**Risques résiduels.** Isolation OS seulement avec `--isoler` tant que D2 n'est
pas tranchée ; même UID pour le shell sandboxé hors systemd-run ; socket local
sans authentification par pair (le contrôle est l'utilisateur Unix).

**Ne protège pas.** Un processus du même utilisateur qui contourne le daemon, ni
un noyau compromis.

## 7. Calcul serverless — `crates/serverless`

**Actifs.** Intégrité de l'entrée, du résultat et de l'association
tâche ↔ nœud.

**Adversaires.** Nœud de calcul qui renvoie un faux résultat, coordinateur qui
réassocie une tâche, rejoueur.

**Points d'entrée.** Fragments de tâche via relais, résultats signés.

**Contrôles.** Découpage et réassemblage liés à la session et au `from` ;
signatures ML-DSA sur le chemin réseau ; timeout et fuel côté exécution.

**Risques résiduels.** Pas de redondance ni de vérification croisée du résultat :
un nœud malveillant peut renvoyer un résultat faux **signé par lui**. La
réputation (`reputation.rs`) est locale, pas une preuve globale. Ce service n'est
pas un marché vérifié.

## 8. Pétales IA — `crates/petals`

**Actifs.** Prompts et réponses, absence d'envoi cloud.

**Adversaires.** Processus local qui usurpe le port Ollama, observateur réseau.

**Contrôles.** endpoint local par défaut (`127.0.0.1:11434`), aucun client
cloud, aucune télémétrie (`FV-SEC-09`).

**Risques résiduels.** Le modèle local peut être empoisonné ou remplacé ; aucune
vérification d'intégrité du serveur Ollama. Les prompts restent en clair sur la
machine.

## 9. Synthèse des frontières

1. **Réseau → processus** : tout est vérifié par signature ou déchiffré ; sinon
   refus explicite, jamais de repli silencieux.
2. **Disque → processus** : l'identité et les reçus sont en clair, protégés par
   les permissions et `duress`, pas par le chiffrement au repos.
3. **Processus → OS** : l'isolation est opt-in tant que D2 n'est pas tranchée.
4. **Gateway → LAN** : intégrité par signature, confidentialité absente (pas de
   TLS) ; le LAN est supposé hostile pour l'intégrité, pas pour l'écoute.

## 10. Ce qui manque encore

- **Audit externe indépendant** : non réalisé. `FV-SEC-08` reste à l'état 5.
- **Rotation/révocation de clés** (`FV-SEC-05`) : absente.
- **Limites par identité** (`FV-SEC-10`) : compteurs oui, quotas par pair non.
- **Chiffrement au repos** : décision non prise ; documenté comme limite.
- **TLS sur la gateway** : non implémenté ; le modèle assume un LAN de confiance
  pour la confidentialité, ce qui doit être dit à l'utilisateur.

---

*Auto-évaluation SEC.1, 2026-11-05. Chaque contrôle listé renvoie à un test ou un
chemin de code de ce dépôt ; les risques résiduels ne sont pas des TODO
marketing, ce sont les promesses que le code ne tient pas encore.*
