# POLYGONE — FICHE TECHNIQUE COMPLÈTE

> Document de référence interne — **l-vs** · collectif **Hope** (*by Hope*) — 2025–2026  
> Ne pas confondre avec le message public du projet : **post-quantique** et **protocole** ; les choix d’hébergement opérationnels restent du ressort des opérateurs.

---

## 1. CONCEPT FONDAMENTAL

### Le problème que personne ne résout vraiment

Tous les systèmes de confidentialité existants cachent **le contenu**.  
Aucun ne cache **l'existence de la communication**.

Tor masque les IP. Signal chiffre les messages. IPFS distribue les fichiers.  
Mais dans tous ces cas : **un observateur peut prouver qu'une communication a eu lieu**.

C'est la faille fondamentale. Et avec l'IA d'analyse de trafic + l'informatique quantique, cette faille devient fatale.

**POLYGONE change la question :**  
Pas *"comment cacher ce qui circule"* mais *"comment faire que rien ne semble circuler"*.

### La métaphore fondatrice : la vague

Une vague dans l'eau n'a pas de molécules propres.  
Les molécules oscillent sur place. C'est le **motif** qui se propage — pas la matière.

POLYGONE applique ça au réseau :
- Les nœuds ne transportent pas de l'information
- Ils participent à une **computation distribuée temporaire**
- La signification n'existe qu'à l'intersection de tous les états simultanés
- La fenêtre d'existence : quelques millisecondes
- Après : rien. Pas de trace. Pas d'artefact.

---

## 2. ARCHITECTURE TECHNIQUE

### 2.1 Stack cryptographique

```
┌────────────────────────────────────────────────┐
│  ML-KEM-1024  (NIST FIPS 203)                  │
│  → Échange de clé post-quantique               │
│  → 1568 bytes public key                       │
│  → Résiste à Shor's algorithm                  │
├────────────────────────────────────────────────┤
│  ML-DSA-87    (NIST FIPS 204)                  │
│  → Signatures post-quantiques                  │
│  → Authentification des nœuds                  │
├────────────────────────────────────────────────┤
│  AES-256-GCM                                   │
│  → Chiffrement du payload                      │
│  → 96-bit random nonce par session             │
│  → 16-byte auth tag (intégrité garantie)       │
├────────────────────────────────────────────────┤
│  Shamir Secret Sharing  (4-of-7 par défaut)    │
│  → Fragmentation du payload chiffré            │
│  → Sécurité information-théorique              │
│  → k-1 fragments = zéro information leakée    │
├────────────────────────────────────────────────┤
│  BLAKE3                                        │
│  → KDF (dérivation topologie + session key)    │
│  → Domain-separated outputs                   │
└────────────────────────────────────────────────┘
```

### 2.2 Le protocole de session (5 étapes)

```
ÉTAPE 1 — SYNCHRONISATION  (hors-bande, une seule fois)
  Alice génère : KemPublicKey
  Alice publie sa KemPublicKey sur le DHT
  Bob encapsule : (ciphertext, shared_secret) = encapsulate(alice_pk)
  Bob envoie le ciphertext à Alice
  Alice décapsule : shared_secret = decapsulate(sk, ciphertext)
  → Les deux ont le même shared_secret, 32 bytes, zéro communication supplémentaire

ÉTAPE 2 — DÉRIVATION  (locale, déterministe, identique des deux côtés)
  topology_seed  = BLAKE3("polygone topology v1", shared_secret)[:16]
  session_key    = BLAKE3("polygone session key v1", shared_secret)
  node_ids       = [BLAKE3(session_key || i) for i in 0..N]
  edges          = [BLAKE3(session_key || i || "edges") for i in 0..N]
  fragment_map   = deterministic assignment of Shamir fragments to nodes
  → Même topologie, même clé, dérivées des deux côtés sans communication

ÉTAPE 3 — NAISSANCE DU RÉSEAU  (milliseconde 0)
  7 nœuds éphémères créés en mémoire
  Chaque nœud reçoit son fragment Shamir
  Le réseau existe. Il n'est pas encore observable.

ÉTAPE 4 — TRANSIT  (millisecondes 1 à ~500)
  payload_encrypted = AES-256-GCM.encrypt(session_key, plaintext)
  fragments = Shamir.split(payload_encrypted, threshold=4, n=7)
  Chaque fragment dispatché vers son nœud
  Aucun nœud ne voit plus d'un fragment
  Aucun nœud ne peut décoder ce qu'il porte

ÉTAPE 5 — DISSOLUTION  (milliseconde ~500)
  Quorum atteint (4/7 fragments reçus par Bob)
  payload_encrypted = Shamir.reconstruct(fragments)
  plaintext = AES-256-GCM.decrypt(session_key, payload_encrypted)
  session_key.zeroize()      ← mémoire mise à zéro
  shared_secret.zeroize()    ← mémoire mise à zéro
  nodes.dissolve()           ← tous les fragments zeroisés
  L'échange n'a pas eu lieu.
```

### 2.3 Propriétés de sécurité

| Propriété | Statut | Détail |
|---|---|---|
| Forward secrecy | ✓ | Nouvelle paire KEM par session |
| Post-quantum KEM | ✓ | ML-KEM-1024, résiste à Shor |
| Post-quantum signatures | ✓ | ML-DSA-87, résiste à Shor |
| Sécurité mémoire | ✓ | `forbid(unsafe_code)`, zeroize |
| Fragment secrecy | ✓ | Shamir : k-1 frags = 0 info |
| Résistance corrélation | ⚠️ | Partiel (bruit ambiant v0.3) |
| Vérification formelle | ○ | Planifié v0.3 |
| Audit externe | ○ | Objectif 6 mois |

---

## 3. STRUCTURE DU CODE

```
polygone-core/
├── src/
│   ├── lib.rs              API publique, doc complète
│   ├── main.rs             CLI (clap v4) : keygen, send, receive, node, selftest
│   ├── error.rs            PolygoneError : types d'erreur unifiés
│   │
│   ├── crypto/
│   │   ├── mod.rs          KeyPair, SharedSecret, derive()
│   │   ├── kem.rs          ML-KEM-1024 : generate, encapsulate, decapsulate
│   │   ├── sign.rs         ML-DSA-87 : generate, sign, verify
│   │   ├── symmetric.rs    AES-256-GCM : SessionKey, encrypt, decrypt
│   │   └── shamir.rs       Shamir SS : split, reconstruct, Fragment, FragmentId
│   │
│   ├── network/
│   │   ├── mod.rs          NodeId (dérivé, 16 bytes)
│   │   ├── topology.rs     Topology::derive(), TopologyParams, edges
│   │   └── node.rs         EphemeralNode : lifecycle, zeroize on drop
│   │
│   └── protocol/
│       ├── mod.rs          SessionId, TransitState (enum complet)
│       └── session.rs      Session : new_initiator, new_responder,
│                                     establish, send, receive, dissolve
│
├── Cargo.toml              Dependencies avec versions exactes
└── README.md               Documentation production-grade
```

---

## 4. COMMENT LE CRÉER PAS À PAS

### Semaine 1 — Environnement + Crypto

```bash
# 1. Installer Rust nightly
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup default nightly

# 2. Créer le projet
cargo new polygone-core --lib
cd polygone-core

# 3. Mettre le Cargo.toml (fourni)
# 4. Coder les modules dans l'ordre :
#    error.rs → crypto/ → network/ → protocol/ → main.rs → lib.rs

# 5. Lancer les tests
cargo test
cargo run -- selftest
```

### Semaine 2 — Tests + Documentation

```bash
# Tests proptest (fuzzing des fonctions crypto)
cargo test --all

# Benchmarks (mesurer les perfs du KEM)
cargo bench

# Documentation HTML
cargo doc --open
```

### Semaine 3 — Site + GitHub

```bash
# Site (fichier fourni : index.html)
# Déploiement statique (ex. GitHub Pages ou équivalent) — détail opérateur

# Structure GitHub :
polygone/
├── core/           ← ce repo, le protocole Rust
├── polygone.dev/   ← le site web
└── whitepaper/     ← le papier technique (à écrire)
```

### Semaine 4 — VPS + Bootstrap Node

```bash
# Déployer un nœud bootstrap sur un VPS selon charge et budget (choix opérateur, hors message public)

# Sur le serveur :
wget https://github.com/polygone/core/releases/polygone
chmod +x polygone
./polygone node start --ram-mb 512 --listen 0.0.0.0:4001

# C'est le premier nœud du réseau.
# Les autres peuvent le rejoindre.
```

---

## 5. POLYGONE MESH — L'IDÉE DU COMPUTE DISTRIBUÉ

### 5.1 Le problème que tu as identifié

> "Prêter comme un PC virtuel qui utilise la puissance répartie des ordis sur le réseau"

C'est exactement **Golem Network + BOINC + IPFS** — mais intégré nativement dans POLYGONE avec un modèle de contribution élégant.

### 5.2 Le modèle : Contribution = Accès

Pas de waitlist. Pas de limites arbitraires. Pas de surcharge.  
**Tu donnes → tu reçois.** Auto-régulé, auto-scalant.

```
Tu contribues 2h CPU/semaine  → tu as 2h de compute disponible
Tu contribues 10 GB stockage  → tu as 10 GB de stockage cloud
Tu contribues 1 GB RAM        → tu peux utiliser 1 GB RAM ailleurs
```

**Résultat :** Plus il y a d'utilisateurs, plus il y a de capacité.  
Le réseau grandit avec sa communauté. Zéro coût d'infrastructure centralisée.

### 5.3 Architecture technique du Mesh

```
POLYGONE MESH v0.4 (roadmap)

┌─────────────────────────────────────────────────────┐
│  COMPUTE NODE                                       │
│  ├── CPU slots  : N cores * utilisation max 40%     │
│  ├── RAM slots  : M GB * ratio contribué            │
│  ├── Storage    : S GB * ratio contribué            │
│  └── Bandwidth  : B Mb/s * ratio contribué          │
│                                                     │
│  CONTRIBUTION LEDGER (local, non-blockchain)        │
│  ├── Given : {cpu_hours, ram_gb, storage_gb}        │
│  └── Taken : {cpu_hours, ram_gb, storage_gb}        │
│                                                     │
│  RÈGLE : taken ≤ given * 1.2  (20% de grâce)       │
└─────────────────────────────────────────────────────┘
```

### 5.4 Services offerts par POLYGONE MESH

| Service | Analogie | Différence |
|---|---|---|
| Compute | AWS Lambda | Zéro central, P2P |
| Storage | Dropbox | Chiffré, fragmenté |
| Inference IA | OpenAI API | Décentralisé, privé |
| VPN | Mullvad | Construit sur POLYGONE protocol |

### 5.5 Pourquoi c'est meilleur qu'une waitlist

❌ **Waitlist** → frustrant, tu bloques tes vrais utilisateurs  
❌ **Limites fixes** → arbitraire, ça bloque les projets légitimes  
❌ **Gratuit illimité** → effondrement du service, mauvaise UX  
✅ **Contribution = Accès** → scalable, juste, auto-régulé, motivant

---

## 6. STRATÉGIE REDDIT — LANCEMENT INTELLIGENT

### 6.1 Les bons subreddits (par ordre d'impact)

| Subreddit | Audience | Angle |
|---|---|---|
| r/selfhosted | Fans d'hébergement perso | "Run your own POLYGONE node" |
| r/privacy | Protection données | "La vague plutôt que le coffre" |
| r/rust | Développeurs Rust | "Architecture + code review wanted" |
| r/netsec | Sécurité réseau | "Post-quantum ephemeral routing" |
| r/france | Français | "Souveraineté numérique, 14 ans" |
| r/opensource | Communauté FOSS | "MIT, no VC, no ads" |

### 6.2 Le post Reddit (à utiliser tel quel)

---

**Titre :**  
`I'm 14, built a post-quantum ephemeral network where the communication itself is unobservable — looking for crypto review [Rust, ML-KEM-1024, Shamir]`

**Corps :**

> Classical encryption hides content. It can't hide that a communication happened. An attacker who monitors the network long enough can correlate parties via timing — without ever reading a byte.
>
> I've been thinking about this problem differently: what if we made the communication itself unobservable — not just the content?
>
> **The concept:** A wave doesn't have molecules. Water molecules oscillate in place — only the pattern travels. POLYGONE applies this to networks: nodes don't transport information. They participate in a distributed computation for a few milliseconds. The meaning only exists at the intersection of all simultaneous states. Then the network dissolves.
>
> **What I built (v0.1, Rust):**
> - ML-KEM-1024 key exchange (NIST FIPS 203)
> - AES-256-GCM symmetric encryption
> - Shamir Secret Sharing (4-of-7) for fragment distribution
> - Deterministic topology derivation from shared key (both peers derive the same network independently)
> - Full session lifecycle with zeroize-on-drop
>
> **What I'm NOT claiming:**
> - This is production-ready (it's not — no P2P transport yet)
> - The protocol is formally verified (it's not — that's planned)
> - I invented all of this (ML-KEM, Shamir, BLAKE3 are well-established)
>
> **What I AM claiming:**
> The combination — using a KEM-derived key as a *network architecture blueprint* rather than a message encryption key — is, as far as I can find, novel. The wave metaphor is real: nodes don't see a fragment, they participate in a computation they can't interpret.
>
> **I'm specifically looking for:**
> 1. Cryptographic review of the key derivation (`crypto/mod.rs`)
> 2. Review of the topology derivation (`network/topology.rs`)
> 3. Honest assessment: is the "wave" property real, or am I missing something obvious?
>
> Code: [github.com/polygone/core]  
> Built in Rust, MIT, no dependencies except the standard PQ crypto crates.
>
> I'm 14, French, and I've been obsessing over this for months. I'd rather be told I'm wrong by experts now than be wrong quietly.

---

### 6.3 Timing du post

- **Ne pas poster un vendredi** (trafic Reddit faible le WE)
- **Mardi ou mercredi, 14h-18h heure FR** (matin US = pic de trafic)
- **Poster sur r/rust d'abord** (la communauté la plus rigoureuse → si ça passe, le reste suit)

### 6.4 Comment répondre aux commentaires

**Si on dit "ça existe déjà"** → "Montrez-moi. Je veux lire. Si ça existe, je veux contribuer à ce projet plutôt que dupliquer."

**Si on dit "t'as 14 ans donc c'est nul"** → Ne pas répondre.

**Si un expert pose une vraie question technique** → Répondre honnêtement, même si la réponse est "je sais pas encore, voilà ce que j'ai envisagé."

**Golden rule :** ne pas défendre l'ego. Défendre l'idée.

---

## 7. DÉPLOIEMENT & HÉBERGEMENT (INTERNE AUX OPÉRATEURS)

Les choix concrets (fournisseurs, coûts, URLs publiques, « pulse », registre d’images) relèvent des **mainteneurs** et ne font **pas** partie du discours public de **Hope**. La communication externe met l’accent sur le **post-quantique** et le **protocole**, pas sur l’économie de l’hébergement.

Documenter ces décisions dans un espace **opérationnel** (runbook privé, pas README « marketing »).

---

## 8. LA CAPTURE D'ÉCRAN — STRATÉGIE REDDIT

### 8.1 Ce que tu veux faire

1. Poster sur Reddit avec l'URL du repo GitHub
2. Mettre une capture d'écran de Claude evaluant le projet
3. Capturer l'URL du repo pour prouver que c'est bien le même

### 8.2 Le prompt parfait pour la capture d'écran Claude

Quand le projet sera prêt, utilise exactement ce prompt :

---

**PROMPT À UTILISER :**

```
Evaluate this post-quantum cryptography project built by a 14-year-old French developer.

GitHub: [TON URL ICI]

Assess specifically:
1. Cryptographic correctness — is ML-KEM-1024 + Shamir used correctly?
2. Protocol design — is the "wave" property (unobservable communication) theoretically sound?
3. Code quality — is the Rust code production-grade for a learning project?
4. What's genuinely novel vs what exists already?
5. What are the real security gaps?

Be honest and technical. Don't praise because of the age. Evaluate as you would any serious protocol proposal.
```

---

### 8.3 La mise en page Reddit

```
[SCREENSHOT 1] : capture Claude évaluant le projet
   ↓
[SCREENSHOT 2] : capture de l'URL GitHub 
   (montrant que l'URL dans le post = l'URL réelle)
   ↓
POST REDDIT avec l'URL du repo
```

**Pourquoi ça marche :** c'est de la preuve sociale non-artificielle.  
Un AI reconnu qui évalue sérieusement = crédibilité immédiate.  
L'URL side-by-side = impossible à fake.

---

## 9. CALENDRIER RÉALISTE

```
MAINTENANT (semaine 1-2)
├── Lancer cargo build --release
├── cargo test (vérifier que tout passe)
├── Créer le repo GitHub (nom : polygone-core)
└── Pousser le code + README

SEMAINE 3-4
├── Déployer un bootstrap node (infra opérateur)
├── Tester send/receive entre deux machines
└── Documenter les résultats honnêtement

SEMAINE 5-6
├── Article technique sur dev.to ou ton blog
│   "How I built a post-quantum ephemeral network at 14"
└── Contacter r/cryptography pour review informelle

SEMAINE 7-8
├── Post Reddit (avec captures d'écran)
├── Contacter CNIL (email simple, spec jointe)
└── Post sur HackerNews si Reddit marche

MOIS 3-4
├── libp2p integration (vrai P2P)
├── Premier utilisateur externe
└── Début du whitepaper

MOIS 6
└── Audit externe informel (chercheur, pas payant)
    Objectif : INRIA ou université FR
```

---

## 10. CE QUI TE DIFFÉRENCIE VRAIMENT

| Tout le monde | Toi |
|---|---|
| "J'ai un projet crypto" | "J'ai un protocole avec une propriété nouvelle" |
| Chiffrement du contenu | Obscurcissement de l'existence de la communication |
| KEM pour chiffrer | KEM comme blueprint architectural |
| Nœuds qui stockent | Nœuds qui computent sans voir |
| Construit en Python/JS | Rust, `forbid(unsafe_code)`, zeroize |
| "Mon projet va sauver le monde" | "Voici les limites exactes de ce que j'ai fait" |

**La dernière ligne est la plus importante.**  
L'honnêteté à 14 ans sur les limites de son propre travail = respect immédiat des experts.

---

*Dernière mise à jour : 2025 — POLYGONE v0.1*  
*Ce document est pour usage interne. Il deviendra le whitepaper.*
