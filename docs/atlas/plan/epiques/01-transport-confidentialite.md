# Épiques TRANSPORT — MSG · HIDE · MESH

> NOTE DE PORTAGE — 4 novembre 2026 : constats techniques établis le 3 novembre 2026 ; vérifier l’état courant, les commits et les tests avant intervention.

> Après lecture de `net/mod.rs` (1 470 l.), `relay.rs` (901 l.), `hide.rs`
> (898 l.), `mesh.rs` (351 l.), `msg.rs`. Chaque constat porte un fichier:ligne.

**État réel.** Le transport est la partie la plus aboutie : KEM ML-KEM-1024 +
AES-256-GCM + Shamir 4/7 + ML-DSA-65 sur octets canoniques incluant `ts`
(`net/mod.rs:198-290`), ancre TOFU persistée en `0600` (`:34-70`), anti-rejeu
±300 s + cache des sessions complétées, plafonds fail-closed. Relais aveugle et
sans état (`relay.rs:1-40`). Hide : SOCKS5 **mono-saut** branché, multi-hop en
plan seulement (`hide.rs:265-310`). Mesh : broadcast UDP maison, non authentifié.

---

## MSG — messagerie éphémère

### MSG.1 — Rendre la durée de rétention vraie, et la nommer
- **Phase** : P4 · **Dépend de** : aucune
- **Constat** : la doc affirme « TTL ≤ 30 s sur relay » (`LEGAL.md:33`,
  `THREAT_MODEL.md:19`, `docs/threat-commodity.md:38`). Faux deux fois : le
  relais ne conserve **rien** (« It never stores fragments », `relay.rs:30`),
  et le récepteur garde une session incomplète **300 s**
  (`SESSION_TTL_SECS`, `net/mod.rs`), purgée seulement au balayage (tous les
  256 enregistrements ou table pleine).
- **Travail** : fixer la durée réelle (30 s est défendable), la câbler, purger
  aussi par balayage temporisé pour que la borne ne dépende pas du trafic.
  Corriger les trois docs : « 0 s au relais ; ≤ N s en mémoire du destinataire
  pour une session incomplète ».
- **Preuve** : test `session_incomplete_purged_at_ttl` — 1 fragment injecté,
  horloge avancée au-delà du TTL, session absente **sans nouveau message**.
  Plus `grep -rn "30 s" docs/ LEGAL.md THREAT_MODEL.md` ne renvoie qu'une
  formulation datée et sourcée.
- **Refus** : pas un effacement mémoire garanti (SSD, swap, core dump non
  couverts) — cf. `VISION-ET-PLAN…:91`.

### MSG.2 — Accuser réception ou échouer bruyamment
- **Phase** : P4 · **Dépend de** : MSG.1
- **Constat** : `send_network` écrit 8 enveloppes, `flush`, retourne l'id de
  session. Aucun accusé attendu. Destinataire absent = le relais n'a pas de
  route et le message disparaît **sans que l'expéditeur le sache** :
  `polygone envoyer` affiche un succès qui n'en est pas un.
- **Travail** : ack signé du destinataire (session + hash du ciphertext),
  attendu avec délai borné ; code non nul et « remis au relais, non confirmé »
  si absent. Un hors-ligne n'est pas un échec dur : deux états, deux mots.
- **Preuve** : `no_recipient_reports_unconfirmed` (code 2, pas « envoyé ») et
  `ack_received_reports_delivered`.
- **Refus** : un ack prouve la réception par le pair, pas la lecture humaine.

### MSG.3 — File d'attente hors ligne bornée et signée
- **Phase** : P4 · **Dépend de** : MSG.2
- **Constat** : absente, par choix d'architecture (`relay.rs` sans état). C'est
  le conflit nommé en `VISION-ET-PLAN…:123` (« éphémère vs fiabilité »).
- **Travail** : `mailbox` par `node_id` chez un nœud volontaire — pas chez le
  relais, qui doit rester aveugle : dépôt chiffré, quota, TTL court, suppression
  à l'ack. Le dépositaire stocke les 7 fragments et ne peut rien lire.
- **Preuve** : `offline_delivery_within_ttl` (B se connecte 10 s plus tard,
  reçoit) et `offline_message_expires` (B après TTL : rien, A prévenu).
- **Refus** : tampon qui meurt, pas historique. `GATE-LÉVY` : un nœud qui
  stocke, même chiffré, change le modèle de menace.

### MSG.4 — Permettre un second appareil sans voler l'identité
- **Phase** : P4 · **Dépend de** : MSG.2
- **Constat** : second `HELLO` pour un `node_id` vivant = `HELLO_DENIED`
  (`relay.rs`), traité comme erreur par `connect` (`net/mod.rs`). Téléphone +
  portable sur une identité = **impossible**. `ETAT-REEL.md` §3 l.11 le classe
  d'ailleurs en non câblé.
- **Travail** : identifiant d'appareil (`node_id` + suffixe), routage vers le
  dernier actif ou vers tous. `node_id` reste l'identité, l'appareil est une
  route.
- **Preuve** : `two_devices_same_identity_both_receive` et
  `device_revocation_stops_delivery`.
- **Refus** : pas de synchronisation d'historique. Un appareil rejoint, il ne
  rattrape pas le passé.

### MSG.5 — Révoquer une identité compromise
- **Phase** : P4 · **Dépend de** : MSG.4
- **Constat** : absent. `duress --confirmer` détruit les fichiers locaux
  (`duress.rs:37-80`) mais n'avertit aucun pair ; une clé volée reste acceptée
  puisque TOFU a épinglé l'ancienne.
- **Travail** : liste de révocation signée, diffusée aux pairs connus, avec
  date de compromission ; un pair qui la reçoit refuse la clé **et** les
  messages signés avant cette date. Étape optionnelle dans `duress`.
- **Preuve** : `revoked_key_messages_rejected`, et `duress` produit la liste
  hors ligne dans un fichier vérifiable.
- **Refus** : révocation ≠ preuve de vol. Elle tue une clé, elle ne désigne
  personne. En mode avion elle ne se diffuse pas — à écrire.

### MSG.6 — Mesurer le débit réel, pas le débit espéré
- **Phase** : P4 · **Dépend de** : MSG.1
- **Constat** : `MAX_WIRE_LINE = 64 Kio`, 200 enveloppes/s par connexion
  (`RATE_LIMIT_PER_SEC`, `relay.rs`). Aucun chiffre produit n'existe — alors que
  `ARCHITECTURE_RESEAU.md:30-37` rappelle que le transport domine le coût.
- **Travail** : instrumenter 1 Mio (≈ 56 trames), mesurer temps et débit utile,
  publier avec machine et date.
- **Preuve** : `scripts/msg-throughput.sh` → `n trames, X s, Y kio/s`, sortie
  collée dans `docs/BENCH-MSG.md`.
- **Refus** : loopback = borne haute théorique, jamais un débit réseau.

### MSG.7 — Fuzzer le parseur d'enveloppes
- **Phase** : P5 · **Dépend de** : MSG.1
- **Constat** : `process_line` est le point d'entrée de toute attaque, avec des
  gardes explicites mais aucune exécution aléatoire.
- **Travail** : cible `cargo fuzz` sur `serde_json::from_str::<NetEnvelope>` +
  `process_line`, corpus initial = trames valides des tests.
- **Preuve** : 10 min sans panique, corpus commité dans `fuzz/corpus/`, trois
  cas régressés ajoutés en tests.
- **Refus** : 10 min ne prouvent pas l'absence de bug.

---

## HIDE — proxy SOCKS5

### HIDE.1 — Prouver l'absence de fuite DNS côté client
- **Phase** : P4 · **Dépend de** : P2.2
- **Constat** : le SOCKS5 transporte un nom d'hôte (`Socks5Target`,
  `hide.rs:67`), la résolution est censée se faire à la sortie — mais rien
  n'impose `socks5h` ni ne refuse un `CONNECT` déjà résolu localement.
- **Travail** : mode « noms seulement » refusant les IP littérales, et réglage
  navigateur documenté.
- **Preuve** : `scripts/hide-dns-leak.sh` — `tcpdump -i any port 53` pendant une
  requête via le proxy : **zéro paquet DNS client**. Collé dans `docs/HIDE-LEAKS.md`.
- **Refus** : le proxy ne corrige pas un navigateur mal configuré ; le test
  prouve le client Polygone.

### HIDE.2 — Décider et tester le comportement IPv6
- **Phase** : P4 · **Dépend de** : HIDE.1
- **Constat** : le nœud de sortie fait `TcpStream::connect` ; une cible avec
  AAAA sort en IPv6 depuis son adresse. Aucun test, aucune règle, aucune
  mention dans `HIDE-SPEC.md`.
- **Travail** : politique par nœud de sortie (`ipv4` / `ipv6` / `both`),
  défaut conservateur.
- **Preuve** : `exit_ipv6_policy_respected` avec deux cibles de test, plus une
  ligne dans `HIDE-SPEC.md` et dans `polygone hide --aide`.
- **Refus** : pas de dual-stack intelligent — on choisit, on documente, on teste.

### HIDE.3 — Brancher le multi-hop, deux sauts d'abord
- **Phase** : P4 · **Dépend de** : HIDE.2, MSG.2
- **Constat** : `plan_circuit` est un dry-run assumé (`hide.rs:265-310`) ;
  `reversed()` décrit le chiffrement en couches (`:240`) qu'aucun transport
  n'applique.
- **Travail** : deux sauts — le premier relaie vers le second **sans pouvoir
  lire la destination**, lisible seulement par la sortie. Réutiliser l'enveloppe
  `stream` avec en-tête chiffré par saut.
- **Preuve** : `scripts/hide-2hops.sh` — logs des deux intermédiaires : le
  premier ne contient **aucune** occurrence du domaine cible.
- **Refus** : deux sauts ne valent pas Tor : ni padding, ni adversaire global,
  ni analyse de trafic.

### HIDE.4 — Définir le padding et mesurer ce qu'il cache
- **Phase** : P4 · **Dépend de** : HIDE.3
- **Constat** : `STREAM_CHUNK = 16 Kio` envoie la taille réelle ; 2 Kio et
  15 Kio donnent des empreintes distinctes.
- **Travail** : paliers fixes (4/8/16 Kio), remplissage CSPRNG, trames factices
  en inactivité si le mode « trafic constant » est activé.
- **Preuve** : `scripts/hide-padding.sh` — deux transferts de tailles
  différentes produisent le **même** histogramme de tailles (diff vide).
- **Refus** : le padding coûte de la bande passante et ne masque pas le volume
  d'une session longue.

### HIDE.5 — Refuser un nœud de sortie qui ment
- **Phase** : P4 · **Dépend de** : HIDE.3
- **Constat** : la sortie est choisie par l'utilisateur ; rien ne vérifie
  qu'elle en est une. Un intermédiaire peut accepter puis ne jamais connecter.
- **Travail** : poignée de main signée « je suis sortie, clé + horodatage »,
  vérifiée contre l'ancre TOFU comme `verify_grant` (`net/mod.rs`).
- **Preuve** : `exit_handshake_rejects_wrong_key` — tunnel non établi.
- **Refus** : empêche l'usurpation, pas la malveillance d'un exit légitime.

### HIDE.6 — Fermer proprement et ne rien laisser
- **Phase** : P4 · **Dépend de** : HIDE.1
- **Constat** : `serve` ouvre un tunnel par connexion SOCKS5 (`hide.rs:366`).
  Aucun test sur l'état des sockets et tâches après SIGKILL du client.
- **Travail** : délai de garde par tunnel, libération à la déconnexion,
  compteur exposé par `polygone hide status`.
- **Preuve** : `tunnels_released_on_abrupt_disconnect` — compteur à zéro en
  moins de 5 s, sortie fermée.
- **Refus** : propreté du cas normal, pas une défense anti-épuisement.

---

## MESH — découverte LAN

### MESH.1 — Ne pas diffuser sur les interfaces qu'on ne veut pas
- **Phase** : P2 · **Dépend de** : aucune
- **Constat** : `announce` binde `0.0.0.0` et diffuse vers
  `255.255.255.255` (`mesh.rs:148`). Sur un portable en VPN, `node_id` et
  adresse du relais partent sur **toutes** les interfaces — y compris le réseau
  du café. Fuite de métadonnées produite par le produit.
- **Travail** : énumérer les interfaces, exclure `tun*`/`wg*`/`utun*` par
  défaut, diffuser une adresse **par interface retenue**.
- **Preuve** : `scripts/mesh-interface-scope.sh` — interface `dummy0` ajoutée,
  `tcpdump -i dummy0 port 7642` **vide** pendant une seconde d'annonce, non
  vide sur l'interface autorisée.
- **Refus** : pas une découverte authentifiée ; le test porte sur la portée.

### MESH.2 — Authentifier l'annonce sans construire un annuaire
- **Phase** : P2 · **Dépend de** : MESH.1
- **Constat** : annonce en clair, 4-5 champs (`mesh.rs:75-99`). N'importe qui
  peut annoncer le `node_id` d'un autre ; le code s'en protège en ne s'y fiant
  pas (`:100-104`) mais affiche quand même le pair.
- **Travail** : annonce signée ML-DSA-65 ; « vérifié » seulement si la
  signature tient, sinon « non vérifié » et jamais sélectionné pour un circuit.
- **Preuve** : `spoofed_announce_marked_unverified`.
- **Refus** : un pair vérifié n'est pas un pair de confiance — la confiance
  reste TOFU, décidée par l'utilisateur.

### MESH.3 — Décider du sort de mDNS, Bluetooth et Wi-Fi Direct
- **Phase** : P2 · **Dépend de** : MESH.2
- **Constat** : absents. Broadcast UDP maison ; la vision demandait « Wi-Fi,
  Bluetooth » (`salvage/13-documentation-polygone/00-vision-source-2026-05-10.md:12`).
- **Travail** : **trancher, pas construire.** Décision argumentée dans
  `VISION.md` §8 : mDNS = découvrabilité pour un coût faible mais bavard ;
  Bluetooth = permissions et surface, incertain hors Linux ; Wi-Fi Direct =
  dépendance plateforme forte. Retirer le mot « Bluetooth » des documents qui
  le promettent encore.
- **Preuve** : section `VISION.md` §8 à jour + `grep -rn -i bluetooth docs/
  README.md` ne renvoyant que la décision.
- **Refus** : ne code rien. Empêche de promettre. `GATE-LÉVY`.

### MESH.4 — Gérer deux machines sur le même relais
- **Phase** : P2 · **Dépend de** : MESH.2
- **Constat** : `discover` déduplique par `node_id` (`mesh.rs:130-136`), pas par
  `(node_id, relay)` : un nœud qui change de relais produit une liste instable
  où le dernier gagne silencieusement.
- **Travail** : une entrée par `node_id`, `relay` mis à jour, horodatage de
  dernière annonce, purge à 30 s.
- **Preuve** : `mesh_relay_change_updates_peer`, `mesh_peer_expires_without_announce`
  (horloge injectée).
- **Refus** : oublier n'est pas bannir — le pair revient à la prochaine annonce.

---

## Ce qui peut faire échouer ces épiques

1. **MSG.3 contredit la promesse fondatrice** : un nœud qui stocke, même 30 s,
   change ce que `THREAT_MODEL.md` affirme. Tant que Lévy n'a pas tranché,
   MSG.2 rend l'échec visible sans le masquer.
2. **HIDE.3 peut sembler fait** : `plan_circuit` imprime déjà un joli circuit.
   La preuve exigée porte sur les logs des intermédiaires, pas sur l'affichage.
3. **MSG.6 peut devenir un argument marketing** : un chiffre loopback présenté
   comme un débit réseau est exactement la promesse corrigée pendant six mois.
4. **MESH.1 touche à la vie privée par l'infra** : exclure `tun*`/`wg*` surprend
   un utilisateur en VPN permanent — option explicite et message, pas silence.
5. **Trop de tâches avant la première preuve réseau** : `P2.2` reste le verrou.
   Ne pas laisser MSG.3 et HIDE.3 passer avant lui.