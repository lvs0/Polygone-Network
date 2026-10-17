# Frontière d’intégration HYPER–Polygone — contrat candidat

> **Statut : proposition de travail non normative, non approuvée.** HYPER et Polygone-Network restent deux roadmaps distinctes; **Polygone Compute** est le point d’intégration applicative. Ce document vérifie les affirmations possibles au regard des sources disponibles et propose un contrat minimal à faire valider par les responsables de HYPER et de Compute. Il ne spécifie ni protocole, ni schéma final, ni politique d’accès, et ne vaut pas décision d’implémentation.

## Conclusion de la vérification

- La frontière produit est cohérente avec `ECOSYSTEM.md` : Polygone-Network garde transport, identité des pairs et chiffrement; Compute est le point d’intégration applicative sans devenir le planificateur global; HYPER reste une roadmap distincte d’orchestration de ressources. Le contrat versionné demeure à définir.
- Le dépôt ne contient **aucune spécification ou source HYPER faisant autorité**. La recherche dans les autres dépôts disponibles ne fournit pas non plus de contrat HYPER. Les objets, états, règles de bail et permissions ci-dessous sont donc des propositions, pas des faits sur HYPER.
- Le chemin Compute observé est un échange `req`/`grant` via le relay. La demande est signée, mais son corps JSON (tâche ou module WASM) n’est pas chiffré de bout en bout dans ce chemin (`crates/client/src/net/mod.rs`, `borrow_request`; `STAGING.md`, § Compute). Il ne faut donc pas présenter les échanges Compute actuels comme chiffrés E2E, ni faire transiter les tâches ou résultats par HYPER par défaut.
- **Écart documentaire à arbitrer :** le code `grant_for()` signe le grant et le client le vérifie (`crates/client/src/net/mod.rs`, `verify_grant`), tandis que `STAGING.md` présente les grants signés comme un travail restant. Clarifier si « reçus vérifiables » désigne une preuve distincte du grant réseau avant d’en déduire un statut de livraison.

## Responsabilités proposées

| Composant | Responsabilité à préserver | Hors de son périmètre dans cette proposition |
| --- | --- | --- |
| **Polygone-Network** | Identité des pairs, transport/relay et fonctions de chiffrement du réseau. | Décider le placement global des ressources HYPER. |
| **Polygone Compute** | Adapter la demande applicative, appliquer les règles d’exécution et présenter le résultat à l’application. | Devenir l’orchestrateur global de capacité. |
| **HYPER** | *Hypothèse à confirmer* : découverte des capacités, allocation/placement et cycle de vie des baux. | Posséder l’identité, les clés ou le transport Polygone; recevoir la charge utile applicative par défaut. |
| **Adaptateur Compute–HYPER** | Traduire demandes, états et erreurs au travers d’un contrat versionné. | Partager implicitement les bases de données, clés ou états internes, ou fusionner les roadmaps. |

Une allocation HYPER n’implique ni que l’exécution Compute a commencé, ni qu’elle a réussi, ni que son résultat a été livré. Ces faits exigent leurs propres états et preuves côté Compute/données.

## Contrat minimal candidat

Les noms ci-dessous sont indicatifs. Ils décrivent le minimum à convenir, indépendamment du choix HTTP, RPC ou autre.

### Objets et champs

| Objet | Champs minimaux proposés | Règle à valider |
| --- | --- | --- |
| **Capacité annoncée** | `resource_type`, `quantity`, `unit`, `constraints`, `schema_version` | Unités explicites, limites et sémantique des contraintes; décider si la capacité disponible est visible et à quel principal. |
| **Demande** | `contract_version`, `request_id`, `idempotency_key`, `correlation_id`, `profile`, `requested_at`, `deadline`, `state` | `request_id` est stable et attribué par HYPER; une répétition de même clé et même contenu ne crée pas une seconde allocation. La clé liée à un contenu différent doit produire un conflit. |
| **Principal** | Contexte d’authentification établi par HYPER; référence Polygone facultative, issue d’une correspondance explicite | Ne pas traiter un `node_id`, pseudonyme ou clé publique comme autorisation implicite. Ne pas mettre un secret d’authentification dans le profil de capacité. |
| **Bail** | `lease_id`, `request_id`, `resources`, `issued_at`, `expires_at`, `state` | Définir séparément renouvellement, révocation, libération, expiration et comportement à la perte de contact; préciser si le renouvellement est admis et sa limite. |
| **Échec** | `code`, `message`, `retryable`, `request_id` ou `correlation_id`; `retry_after` et `field` si applicables | Message sûr pour le client, sans détail interne ou secret; mêmes erreurs sémantiques quel que soit le transport. |

`profile` devrait exprimer les types/quantités de ressources et contraintes nécessaires, avec unités et version de schéma. La priorité, les coûts, la région, la durée demandée et les garanties de disponibilité ne sont pas présumés : les ajouter seulement après validation métier et sécurité.

### États proposés

**Demande :** `pending` → `allocated`, `rejected`, `failed`, `cancelled` ou `expired`.

- `pending` signifie acceptée pour traitement, pas garantie d’allocation.
- `allocated` signifie qu’un bail a été créé; ce n’est pas un état d’exécution ou de livraison.
- `rejected` signifie décision négative de capacité/politique; `failed` signifie échec de traitement. Ils doivent avoir une raison structurée.
- `cancelled` s’applique à une demande encore annulable; l’effet d’une annulation après allocation doit être défini explicitement.
- `expired` signifie que l’échéance de la demande a été atteinte sans allocation.

**Bail :** `active` → `released`, `revoked` ou `expired`. `renewal_pending` n’est nécessaire que si le renouvellement est asynchrone. Aucun état `running`, `result_delivered` ou `message_received` ne doit être attribué par HYPER sans responsabilité et preuve explicites correspondantes.

### Erreurs minimales proposées

| Code sémantique indicatif | Signification | Traitement attendu |
| --- | --- | --- |
| `invalid_request` / `unsupported_capability` | Profil mal formé, unité ou schéma incompatible, capacité non prise en charge. | Erreur non réessayable sans correction; fournir `field` si utile. |
| `unauthenticated` / `forbidden` | Principal absent, invalide ou non autorisé. | Ne pas révéler de détails de politique; pas de retry automatique. |
| `not_found` | Demande ou bail inconnu/inaccessible. | Terminal pour cet identifiant. |
| `idempotency_conflict` | Clé déjà utilisée avec un contenu différent. | Ne pas créer une autre demande. |
| `capacity_unavailable` | Capacité insuffisante ou refus de politique. | État terminal `rejected` ou réponse d’erreur définie de façon uniforme; toute réémission exige une nouvelle décision explicite. |
| `deadline_exceeded` / `cancelled` | Échéance atteinte ou annulation acceptée. | État de demande terminal; préciser le sort d’un bail déjà créé. |
| `rate_limited` / `temporarily_unavailable` | Limite ou dépendance temporairement indisponible. | Indiquer `retryable` et `retry_after` lorsqu’ils sont fiables; le client respecte l’idempotence. |
| `internal_error` | Échec imprévu côté service. | Identifiant de corrélation, message non sensible; ne pas suggérer que l’allocation a échoué si son résultat est inconnu. |

La liste et la correspondance entre erreurs synchrones et états terminaux restent à valider; les erreurs de transport ne doivent pas être interprétées comme preuve qu’une demande n’a pas été acceptée.

### Opérations minimales à décider

1. Décrire/lister les profils de capacité accessibles, si cette découverte est autorisée.
2. Soumettre une demande idempotente et obtenir `request_id` + état initial.
3. Lire l’état d’une demande asynchrone, par interrogation ou notification à convenir.
4. Annuler une demande encore en attente.
5. Renouveler ou libérer un bail, selon la politique confirmée.

Aucune méthode, route, sérialisation, durée par défaut ou sémantique de retry n’est fixée ici. Si le protocole choisi est HTTP, OpenAPI versionné est un candidat de description et d’outillage, pas une décision d’utiliser HTTP [1].

## Garde-fous de séparation

- Garder l’authentification/autorisation de l’API HYPER explicites. Une identité Polygone ne peut y être associée qu’au moyen d’une règle de correspondance confirmée.
- Ne transmettre à HYPER ni clé privée Polygone ni tâche, module WASM, résultat ou charge utile applicative par défaut. HYPER reçoit au plus les données de contrôle nécessaires à l’allocation.
- Garder distincts le bail HYPER, le TTL d’une session réseau, l’état d’exécution et l’accusé de livraison applicatif.
- Ne pas déplacer dans HYPER le relay, l’identité des pairs ou le protocole réseau; ne pas déplacer dans Compute le placement global attribué à HYPER.
- Un résultat d’allocation n’atteste pas qu’un pair a reçu ou exécuté une tâche. L’API de données Compute doit conserver sa propre sécurité et ses propres états.

## Hypothèses non validées — décisions bloquantes avant intégration

1. Le modèle HYPER réel : ressources, granularité, unités, capacité annoncée et contraintes.
2. API déclarative d’état souhaité ou opérations impératives suivies par identifiant; traitement synchrone/asynchrone.
3. Protocole, hébergement, versionnement, limites, authentification et autorisation.
4. Principal reconnu par HYPER et correspondance éventuelle vers `node_id`/identité Polygone; règles d’accès et de révocation.
5. Sémantique de la demande : idempotence, priorités, deadline, annulation, erreurs, retries et compatibilité.
6. Sémantique des baux : TTL, renouvellement, libération, révocation, expiration et perte de connectivité.
7. Rôle exact de Compute après allocation, destinataire d’exécution et canal de données/résultats; protection des tâches/résultats et preuve de livraison.
8. Sens de « grant signé » versus « reçu vérifiable » dans Compute, au vu de l’écart entre code et `STAGING.md`.

Les sources externes ci-dessous sont des **comparateurs de conception seulement** : Kubernetes explique le choix entre modèle déclaratif et API impérative à identifiant d’opération [2]; FINOS Open Resource Broker illustre demande, suivi d’état, restitution et adaptateurs fournisseurs [3]. Aucun ne spécifie HYPER.

## Sources vérifiées

- `ECOSYSTEM.md`, § « Product boundary: Network, Compute and HYPER » : frontière produit et contrat versionné encore à définir.
- `ARCHITECTURE.md`, §§ 3, 5, 7 et 8 : transport client–relay, couche d’exécution RES, daemon et catégories d’erreur Polygone; ce n’est pas le contrat HYPER.
- `STAGING.md`, § « Service : compute » : périmètre Compute annoncé et réserve sur le chiffrement E2E des requêtes.
- `crates/client/src/net/mod.rs`, fonctions `borrow_request`, `verify_grant` et `grant_for` : enveloppe `req` signée, corps JSON non chiffré dans ce parcours, grant signé/vérifié côté client.
- `RELEASE-STATUS.md`, § 103–106 : proposition HYPER non normative et décisions à confirmer.
- [1] [OpenAPI Specification](https://spec.openapis.org/oas/latest.html) — format neutre pour décrire une interface HTTP; ne prescrit pas HTTP pour ce projet.
- [2] [Kubernetes — Custom Resources](https://kubernetes.io/docs/concepts/extend-kubernetes/api-extension/custom-resources/) — distinction API déclarative/impérative et suivi d’opération.
- [3] [FINOS Open Resource Broker](https://github.com/finos/open-resource-broker) — exemple externe de demandes, états, restitution et adaptateurs fournisseurs.
