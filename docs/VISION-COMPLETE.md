# Polygone — Vision complète unifiée

> *Synthèse exhaustive depuis les conversations et notes.*
> *Objectif : produit prêt pour adoption publique, sécurisé, sans compromis.*

---

## 1. Serverless sur Polygone

- Exécution de fonctions/tâches sur les nœuds du réseau.
- Modèle : demande → fragmentation Shamir → exécution sur ≥4 nœuds → résultat reconstruit.
- Backends : Ollama local (petals), WASM (wasmi), sandbox Linux (`exec.rs`).
- Réputation des nœuds (`reputation.rs`) pour préférer les pairs fiables.
- Kill-switch : arrêt immédiat si compromission détectée.

## 2. Gateway domaine public

- Un domaine → front web statique + API JSON.
- Le client web parle à un proxy local (`polygone-gateway`) qui se connecte au réseau.
- Pas de navigateur spécial : HTTPS + Service Worker + WebCrypto côté client.
- Le téléphone mobile peut rejoindre comme nœud via le même domaine.

## 3. Site officiel

- Design sobre, honnête, anti-AI-slop.
- Sections : pitch, crypto, démo, téléchargement, documentation, état du réseau.
- Généré depuis les docs existantes (`README.md`, `STRATEGIE.md`, `docs/*`).
- Hébergement statique : `docs/site/` + CI deploy vers Pages/Vercel/Cloudflare.

## 4. Mobile comme nœud

- PWA installable avec consentement explicite ("Acceptez-vous de participer comme nœud ?").
- Background sync via Service Worker + Wake Lock API.
- Contribution optionnelle : messaging, relais local, compute mineur.
- Batterie/network aware : pauses auto si <20% batterie ou mobile data only.

## 5. État du réseau

- `/api/v1/network/status` : nœuds actifs, latence moyenne, charge.
- Carte des pairs anonymisée (pas d’IP, seulement NodeId + pays estimé).
- Historique 24h agrégé.

## 6. Comptes utilisateurs

- Authentification par clé locale : pas de mot de passe, pas d’email.
- `polygone login` génère une paire locale + enrollment token signé.
- Le site accepte le même token pour lier un appareil.
- Multi-appareils : chaque appareil a sa clé, le compte agrège les appareils.

## 7. QR / Deep link / Passkey

- `polygone pair --qr` : affiche un QR avec `polygone://pair/<token>`.
- iPhone : ouvre directement l’app Polygone ou propose “Ajouter au trousseau”.
- Android : intent filter sur `polygone://`.
- Passkey WebAuthn : le site propose “Ajouter Polygone Passkey”.

## 8. Sécurité

- Zero trust : chaque message signé ML-DSA-65, chaque fragment chiffré AES-256-GCM.
- Kill-switch : USB + clavier + GPIO.
- Auditabilité : logs locaux uniquement, pas de serveur central.
- Transparence : code AGPL-3.0, bounty program, security.txt.

## 9. Activité de mes nœuds

- Dashboard local (`polygone status`) + distant via compte.
- Métriques : messages envoyés/reçus, fichiers transférés, temps de disponibilité.
- Réputation locale + globale.

## 10. Accès aux services

- CLI unifié (`polygone`) avec sous-commandes pour chaque service.
- Web : interface statique + API gateway.
- Mobile : PWA avec les mêmes services.

---

## Priorités d’implémentation

1. **P0** : Finaliser `drive.rs` + TUI honnête + `brain.rs` (cette session)
2. **P1** : Serverless module + gateway HTTP minimal
3. **P2** : Site officiel statique
4. **P3** : Mobile PWA + QR/pairing
5. **P4** : Comptes + dashboard réseau
6. **P5** : Release GitHub + CI + P8 binaires

---

*Document vivant — mis à jour au fur et à mesure de l’implémentation.*
