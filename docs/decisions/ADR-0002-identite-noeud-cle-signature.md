# ADR-0002 — Identité de nœud : dérivation depuis la clé de signature ML-DSA-65 (D2)

- **Statut** : acceptée (2026-11-19, mandat opérateur « finir la production en toute autonomie »)
- **Remplace** : `DEC-SEC-03` du registre de consolidation (dérivation depuis la clé ML-KEM-1024, session S19)
- **Confirme** : `docs/adr/0001-authentification-du-relais.md` (2026-11-09, autorité) qui avait déjà
  acté « identité dérivée de la clé de signature » au niveau du relais ; le présent ADR aligne
  le registre de consolidation et documente la migration du magasin de confiance.
- **Preuve** : `crates/core/src/sign.rs:207-230` ; `crates/client/src/net/mod.rs:103-160` (migration) ;
  test `net::tests::key_rotation_changes_the_node_id_and_old_grants_stay_with_the_old_key`

## Contexte

Le registre `DEC-SEC-03` (confirmé, session S19) définissait le `NodeId` comme le préfixe
hexadécimal (16 caractères) de la clé publique **ML-KEM-1024**. Le code issu des sessions 9-10
(P0 Compute, 2026-11-14/15) dérive au contraire le `node_id` de la clé de **signature ML-DSA-65**.
L'écart était signalé comme décision ouverte D2 ; il doit être tranché, pas « corrigé »
silencieusement dans un sens ou dans l'autre (§9.6 de `project/AUTONOMOUS_EXECUTION.md`).

Deux propriétés s'opposent :

1. **Stabilité d'adresse** (variante KEM) : la rotation de la clé de signature ne change pas
   l'adresse ; les pairs n'ont rien à réapprendre.
2. **Liaison identité ↔ auteur** (variante ML-DSA) : les grants et résultats sont signés avec la
   clé ML-DSA. Si l'identité survivait à la rotation de cette clé, une clé révoquée continuerait
   de produire des résultats acceptés pour la même adresse — la révocation serait cosmétique.

## Décision

Le `node_id` est dérivé de la clé publique de signature :

```text
node_id = hex(BLAKE3-256("polygone node id v1" || sign_pk)[..16])   → 32 caractères hex
```

(`NODE_ID_CONTEXT = "polygone node id v1"`, `crates/core/src/sign.rs:207`.)

La clé KEM reste liée à l'identité par l'offre `kem_offer` **signée** : un pair ne peut pas
présenter une clé KEM orpheline d'une identité ML-DSA.

**Conséquence assumée** : une rotation de clé de signature change l'adresse sur le relais. Les
grants de l'ancienne clé restent avec l'ancienne clé ; les pairs doivent réapprendre la nouvelle
adresse (TOFU). La révocation est **locale** : l'ancienne clé reste mathématiquement valide pour
l'ancien `node_id` — limite documentée dans `docs/P0-COMPUTE-SECURITY-2026-11-14.md`, pas une
révocation distribuée.

## Migration

Le magasin de confiance pré-S1 (entrées 16 hex, dérivées KEM) est **ré-encodé sans rien
supprimer** : la nouvelle entrée 32 hex est ajoutée à côté de l'ancienne, et le fichier intact
est préservé dans `peers.json.v1.bak` au premier passage (`net::peers_v1_backup_path`). Un
binaire ancien relit encore ses identifiants ; un binaire neuf ne dégrade pas un pair connu en
« premier contact ».

## Critère de tranchage

Le critère technique prédominant est la propriété de sécurité testée : un résultat doit être
signé par la clé **dont le nœud est dérivé** (`task.rs`, erreurs `ProducerKeyMismatch`,
`InvalidSignature`). La variante KEM rendrait cette vérification incohérente après rotation.
La stabilité d'adresse est un confort ; la liaison identité ↔ auteur est une exigence.

## Alternatives écartées

- **Conserver H(KEM) + ré-encoder à la rotation** : deux identifiants pour une même identité,
  grants ambiguës, aucune propriété gagnée.
- **Double dérivation (KEM pour l'adresse, ML-DSA pour l'auteur)** : deux notions d'identité à
  maintenir synchronisées ; la migration montre qu'un seul identifiant suffit.
