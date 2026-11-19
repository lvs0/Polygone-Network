# Preuve P0 — deux hôtes distincts, Hope ↔ cue-sandbox (2026-11-19)

> **Instantané daté — 2026-11-19.** Ce document décrit une exécution unique et
> ne vaut que pour l'arbre, les binaires et les hôtes décrits ci-dessous ; il ne
> remplace pas `STATE.md` ni le protocole `docs/P0-DEUX-MACHINES.md`.

- **Statut** : exécutée. Remplit le tableau §7 de `docs/P0-DEUX-MACHINES.md` et coche §9.4.
- **Mandat** : opérateur, 2026-11-19 (« finir la production en toute autonomie » ; « tu peux accéder en SSH à la sandbox Cue »).
- **Arbre testé** : contenu de `d8d08f3` + câblage `evidence.v1` (commité avec la présente preuve).
  Binaires release construits sur Cue (cargo 1.97.1, 08:32–08:34 UTC+1), rapatriés sur Hope par scp ;
  mêmes fichiers aux deux bouts (SHA256 identiques par construction). glibc Hope 2.44 ≥ glibc Cue 2.39 :
  binaires construits sur Cue, exécutés sur Hope — compatible.
- **Chemin réseau** : WAN via Tailscale (relay DERP « iad »). Cue n'a pas de tailscaled interne :
  l'IP 100.99.15.112 est proxyfiée par la plateforme Cue vers la sandbox (eth0 169.254.0.21/30).

## 1. Ce qui est établi

Deux noyaux distincts (`7.2.5-3-omarchy` vs `6.18.38+`), deux horloges (écart mesuré **0 s**),
deux identités ML-DSA-65 distinctes, un relais aveugle, messagerie fragmentée authentifiée,
Compute E2E (ML-KEM-1024 + AES-256-GCM) avec vérification L1 et preuve persistée, contrôles
négatifs sur le fil (rejeu, clair, usurpation, HELLO nu, doublon, AUTH_RESP rejoué), coupure
réseau sans faux succès, redémarrage, rotation de clé avec réappairage.

## 2. Ce qui n'est PAS établi (honnêteté du périmètre)

- **LAN / NAT résidentiel** : aucun pair derrière une box ; la traversée de NAT n'est pas testée.
- **Perte de paquets à grande échelle** : non testée.
- **pcap tcpdump** : la capture a tourné sur Cue (`/root/p0/p0.pcap`, filtre `tcp port 7000`)
  mais n'a pas pu être rapatriée : en fin de session, le port 22 de la sandbox est passé en
  mode « Tailscale SSH check » (authentification interactive exigée par la plateforme).
  La preuve de confidentialité repose donc sur les **captures de trames applicatives**
  (instruments §6), qui couvrent les deux jambes : A↔R (capture locale Hope) et R↔B
  (capture proxy Cue, rapatriée). Le proxy recopie les octets sans les modifier : les
  trames R↔B journalisées sont exactement celles que le relais a émises vers B.
- **Journaux relais/B non rapatriés** (même cause SSH) : le scan de secrets porte sur les
  trames capturées et les journaux côté A. Les journaux relais observés en direct
  (INFO, handshake non journalisé) ne montraient aucun matériel secret.
- **Révocation distribuée** : la rotation révoque localement ; l'ancienne clé reste
  mathématiquement valide pour l'ancien `node_id` (limite ADR-0002, documentée).
- **Premier réappairage post-rotation** : la première tentative compute vers le nouveau
  `node_id` a échoué (course avec l'enregistrement du ghost, relancé 3 s plus tôt) ;
  la seconde a réussi. Aucun faux succès ; la convergence demande un retry.

## 3. Tableau de preuves (§7 du protocole)

| Champ | Machine A | Machine B |
|---|---|---|
| Nom d'hôte | Hope | 046b262ac98b (cue-sandbox) |
| `uname -srm` | Linux 7.2.5-3-omarchy x86_64 | Linux 6.18.38+ x86_64 |
| Commit | d8d08f3 + câblage evidence (même contenu Rust) | idem (tar 08:31, build 08:32) |
| `node_id` | 548f7ee41cdcd5f2033e43348f5d2153 | 8a95afb7064cbd314ed84ab413cd1f2f (puis a2c416951f4decef076d2f281e85d757 après rotation) |
| Empreinte BLAKE3-256 clé de signature A | e8931355841dffa4bd6690c6272e34c91e382a42d890ed97c57e64cd925a0730 (calculée, blake3 sur sign_pk) | e8931355841dffa4bd6690c6272e34c91e382a42d890ed97c57e64cd925a0730 (affichée à l'apprentissage) — **MATCH** |
| Adresse du relais | 100.99.15.112:7000 | 127.0.0.1:7003 (proxy instrument) → 127.0.0.1:7000 |
| Horodatage réception | — | 1795074028 ≤ réception ≤ 1795074040 (message P0WAN-1795074028-4022707) |
| Message reçu (extrait) | — | `P0WAN-1795074028-4022707`, session 598a4f8f2299214e, 4/7 fragments |
| Horloges (préflight) | 1795073885 | 1795073885 — écart 0 s < 300 s |

La clé KEM de B (3136 hex) a été transférée de B vers A **par SSH** (canal contrôlé par
l'opérateur), jamais par le relais — propriété §4 du protocole respectée.

| Contrôle | Résultat observé | Verdict |
|---|---|---|
| `AUTH_OK` sur A | envoi confirmé (« message envoyé », session 598a4f8f2299214e) ; le client refuse tout ack ≠ AUTH_OK (`net/mod.rs:539`) | PASS |
| `AUTH_OK` sur B | observé sur le fil : `R2B AUTH_OK` (wire-b.log, plusieurs sessions) | PASS |
| message livré et lisible | `P0WAN-1795074028-4022707` affiché chez B, 4/7 fragments | PASS |
| empreintes identiques | calcul BLAKE3 indépendant côté A = empreinte affichée côté B (64 hex, MATCH) | PASS |
| `HELLO` nu refusé | `HELLO_DENIED auth_required` (connexion brute depuis A) | PASS |
| double enregistrement refusé | `Error: relay: échec d'authentification … AUTH_DENIED duplicate` | PASS |
| `AUTH_RESP` rejoué refusé | nouvelle connexion AUTH_INIT (identité A) → CHALLENGE frais → AUTH_RESP capturé de B rejoué → **`AUTH_DENIED invalid_token`** | PASS |
| aucune clé privée dans les journaux | scan régions strictement privées (sign_sk[64:192], kem_sk[0:128], kem_sk[64:192]) sur 6 journaux/captures : **0 fuite** ; deux faux positifs structurels expliqués §5 | PASS |

## 4. Volet Compute (§9)

| Contrôle | Résultat observé | Verdict |
|---|---|---|
| NODE_A ≠ NODE_B, dérivées ML-DSA | 548f7ee4… ≠ 8a95afb7… (32 hex, `node_id_from_sign_pk_hex`) | PASS |
| `kem_offer` signé, KEM liée à NODE_B | trame `kem_offer` avec `sig` + `signer` = clé ML-DSA de B (wire-a.log, wire-b.log) | PASS |
| tâche sandboxée, sortie non vide, L0 | `printf p0-compute-ok` → sortie `p0-compute-ok`, `vérification : producer-signed` ; chemin `exec::run_sandboxed`, systemd 255 présent sur Cue | PASS |
| capture sans tâche ni résultat en clair | §5 : 0 occurrence sur les deux jambes | PASS |
| rejeu `req` refusé | trame req complète réinjectée vers B (INJECT_OK) → `WARN polygone::net] RES : requête compute rejouée ignorée (548f7ee4… / 6e4a767665c7cd36)` ; aucun second grant (comptage grants = comptage courses légitimes) | PASS |
| requête en clair refusée | trame req non signée, payload `{"action":"compute","task":"printf PLAINTEXT-LEAK"}` injectée → aucune exécution, aucun grant, `PLAINTEXT-LEAK` absent de tous les journaux | PASS |
| usurpation `from` refusée | trame req avec `from` = NODE_B, signature de A inchangée → aucun grant, aucune exécution ; B ferme la connexion (fail-closed) puis se reconnecte | PASS |
| coupure réseau sans faux succès | relais tué par PID (957170), port 7000 vérifié fermé (`ss` count 0) pendant `sleep 25` → A : « ✖ aucune réponse vérifiable », `Error`, **EXIT=1** ; aucun résultat livré | PASS |
| redémarrage B sans session partielle | B tué puis relancé : ré-enregistrement AUTH_OK, compute `restart-ok` réussi ; la course interrompue n'a jamais produit de grant (coupure ci-dessus) | PASS |
| révocation après rotation | `clef-tourner` sur B : preuve de rotation 3309 octets, KEM inchangée, NODE_B2 = a2c41695… ≠ NODE_B ; compute vers l'ancien node → échec ; vers le nouveau → réappairage TOFU réussi (2e tentative, cf. §2), peers.json contient les deux ancres | PASS |
| vérification L1 + preuve persistée | `--verifier-local` : recalcul dans le sandbox de Hope, `vérification : recomputed-local`, fichier `evidence-1795074079-6cea9859.json` (0600) : schema polygone.evidence.v1, committed = observed = 3fe743286099ef39887c4ac9f70e37c9b1ff5bb0ef5a471bc6020324ddbc4d61, producer = NODE_A, host = Hope | PASS |

## 5. Confidentialité — méthode et résultats

**Instruments** : deux proxies TCP ligne (§6) qui recopient les octets sans les modifier et
journalisent les trames délimitées par newline. Jambe A↔R : proxy local Hope
(`127.0.0.1:7001 → 100.99.15.112:7000`), course `final-cap-1795075399`. Jambe R↔B :
proxy Cue (`127.0.0.1:7003 → 127.0.0.1:7000`), 24 trames sur 3 courses + injections.

**Résultats** :
- Trames observées : `kem_hello`, `kem_offer`, `req`, `grant` (4 par course).
- `req.payload` = ComputeEnvelope `polygone-compute-e2e-v1` : `kem_ct` présent, `body`
  85 octets opaques (AES-256-GCM), aucune chaîne de tâche lisible.
- `grant.payload` = même enveloppe, `kem_ct: null` (session établie), `body` 84–243 octets
  opaques, ratio ASCII imprimable 0,42 (aléatoire), aucun résultat lisible.
- Chaînes interdites recherchées dans TOUTES les captures : `printf`, `p0-compute-ok`,
  `capture-ok`, `inj-test`, `inj2-`, `inj3-`, `final-cap-`, `late-ok`, `late2-`,
  `re-pair-ok`, `restart-ok`, `P0WAN`, `PLAINTEXT-LEAK` → **0 occurrence**.
  (Les injections hostiles circulaient sur le port 7003, hors filtre tcpdump 7000 ;
  leur contenu clair est l'attaque elle-même, pas du trafic produit.)

**Scan de secrets — deux faux positifs structurels, expliqués** :
un premier scan par préfixe `sk[:64]` a matché. Investigation :
1. ML-DSA-65 (FIPS 204) : `sk = ρ || K || tr || s1 || s2 || t0`, `pk = ρ || t1`.
   `sk[:64 hex] = ρ = pk[:64 hex]` : le préfixe « secret » est le seed **public**.
   Le match était le champ `signer` (clé publique) des trames — vérifié :
   `sk[:64] == pk[:64]` True, match dans `signer` uniquement, ni payload ni sig.
2. ML-KEM-1024 (FIPS 203) : `sk = s || ek || H(ek) || z` ; le milieu de sk tombe dans
   `ek` (clé publique embarquée). Vérifié : région matchée ⊂ `kem_pk_hex`.
Scan corrigé sur régions strictement privées (`sign_sk[64:192]`, `kem_sk[0:128]`,
`kem_sk[64:192]`) : **0 fuite** sur les 6 journaux/captures.

## 6. Instruments reproductibles

- `scripts/p0-wire-proxy.py` : proxy A↔R (local, journalise les trames des deux directions).
- `scripts/p0-ghost-proxy.py` : proxy R↔B avec injection (trames déposées dans `inject/`,
  envoyées à la connexion B courante ; INJECT_OK/INJECT_FAIL journalisés).
- Rejeu AUTH_RESP : script python inline (§3) — AUTH_INIT avec identité A, CHALLENGE frais,
  AUTH_RESP capturé de B rejoué tel quel.
- Empreinte : `blake3(sign_pk)` (venv scratch, paquet blake3) comparé aux 64 hex affichés
  par B à l'apprentissage.

## 7. Verdict

Les gates « deux hôtes distincts » du protocole P0 (messagerie §1–7 et Compute §9) sont
**satisfaits sur deux noyaux séparés par un WAN**, avec les limitations explicites du §2.
Compute reste **non validé LAN/NAT** ; la validation physique au sens strict du dépôt
(deux noyaux, deux horloges, chemin WAN — standard de `scripts/p0-modal-two-host.py`)
est acquise. La prochaine marche est un pair derrière une box résidentielle.
