# msh — Protocole d’échange de poids et neurones pour IA

## 1. Vue d’ensemble
msh définit un format standardisé et sécurisé pour publier, découvrir, demander et transférer des paramètres de modèles (poids, gradients, neurones) sur le réseau Polygone.

## 2. Format des messages (sérialisation)
- Encodage : MessagePack (ou CBOR) pour efficacité binaire.
- Enveloppe :
  ```
  {
    "t": "announce|request|transfer|ack|error",   # type
    "id": "<uuid>",                                 # identifiant unique du transfert
    "model_id": "<hash>",                           # identifiant du modèle (BLAKE3)
    "chunk": {"offset":0,"size":N,"total":T},     # pour les transferts fragmentés
    "payload": <binaire|hex>,
    "sig": "<signature_ed25519>",                   # signé par l’émetteur
    "nonce": "<uuid>",                              # anti-rejeu
    "timestamp": 1234567890
  }
  ```

## 3. Découverte et annonce
- Chaque nœud maintient un index local (DHT) des modèles disponibles.
- Un nœud publie une annonce périodiquement sur le topic gossipsub `msh/announce`:
  ```
  { "model_id": "<hash>", "size": 12345678, "chunks": 12, "labels": ["vision","fr"] }
  ```
- Les pairs écoutent ce topic pour découvrir les modèles.

## 4. Processus de transfert
1. **Annonce** (broadcast) — émetteur publie `announce`.
2. **Requête** — pair envoie `request` (unicast) pour un `model_id` (optionnellement un intervalle d’offset).
3. **Transfert** — émetteur fragmente en chunks (ex. 1 MiB) et envoie `transfer` séquentiellement ou via flux fiable (stream libp2p).
4. **Validation** — récepteur vérifie chaque chunk (hash, offset) et envoie `ack` ou `error`.
5. **Finalisation** — une fois total recomposé, vérification du hash global (BLAKE3). Si OK, enregistrement dans `models_store/`.

## 5. Sécurité et validation
- **Intégrité** : chaque chunk inclut un hash BLAKE3. Le modèle global a un hash racine.
- **Authenticité** : signature Ed25519 de l’annonce et des messages critiques (request, transfer).
- **Anti-rejeu** : nonce + timestamp + fenêtre de validité (ex. ±5 min).
- **Limitation de débit** : politique par pair (token bucket) ; mise en liste grise si excès.
- **Poids malveillants** : sandbox optionnelle pour exécution ; signature des producteurs de confiance (web-of-trust).

## 6. Découverte des capacités (capabilities)
- Un nœud peut publier `msh/capabilities` :
  ```
  { "compute": "cpu|gpu", "memory_mb": 8192, "frameworks": ["gguf","safetensors"], "max_parallel": 4 }
  ```

## 7. Rôles
- **Fournisseur** (Provider) : publie des modèles.
- **Consommateur** (Consumer) : requiert des modèles.
- **Relais** (Relay) : aide au transfert (chunk) entre pairs.

## 8. Notes futures
- Prendre en charge la reprise (range requests).
- Chiffrement de bout en bout optionnel (clés partagées).
- Incitations (tokens) pour le partage de modèles.
