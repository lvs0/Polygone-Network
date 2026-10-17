# Accusé de réception de bout en bout — proposition P2

> **Brouillon non normatif; aucun changement de protocole ou de comportement n’est appliqué.** Cette proposition vise le transfert de fichiers Polygone-Network/Drive. Elle ne vaut ni décision de Lévy ni approbation d’un propriétaire de protocole.

## Portée et sémantique recommandée

Limiter la première version aux fichiers transmis par le chemin réseau Polygone-Network. Ne pas assimiler l’accusé à un accusé TCP/relay, à la lecture d’un message humain, ni à une preuve d’intégration Compute ou HYPER.

Définir l’événement positif comme **`file_published`** : le client destinataire a reconstitué tous les blocs Drive cohérents, vérifié leurs métadonnées, puis `save_received_in` a réussi à publier le fichier dans `~/.polygone/received/`. L’accusé doit être émis **après** cette réussite, jamais après la seule réception des fragments ou l’acceptation par le relay.

Cette sémantique atteste le résultat rapporté par le client destinataire honnête. Elle ne prouve ni que l’utilisateur a ouvert le fichier, ni qu’un destinataire malveillant dit vrai, ni qu’un fichier survivra à toute panne matérielle ultérieure.

Corréler l’accusé avec le `transfer_id` Drive existant, la taille exacte et le BLAKE3 des octets originaux. L’état côté expéditeur reste **non confirmé** tant qu’un accusé valide et correspondant n’a pas été reçu. Un délai dépassé, une déconnexion ou un accusé invalide ne doit jamais être affiché comme une livraison confirmée.

## Esquisse de protocole

1. Pour un transfert, l’expéditeur crée en mémoire une clé KEM de retour à usage unique et un jeton aléatoire imprévisible. Il les lie au `transfer_id`, au `node_id` expéditeur et au `node_id` destinataire.
2. Il transporte ces paramètres dans une extension facultative chiffrée/authentifiée par la clé de session du bloc correspondant (par exemple `receipt_ct` dans l’enveloppe KEM). Chaque bloc d’un même transfert porte les mêmes valeurs. Le jeton et la clé privée de retour ne sont jamais envoyés en clair; la clé privée de retour ne persiste pas sur disque.
3. Le destinataire déchiffre et valide les paramètres, vérifie que le `transfer_id` correspond au bloc Drive, puis conserve l’état de reçu avec l’état borné du transfert. Les entrées de reçu doivent partager les limites mémoire, l’expiration et le nettoyage du transfert; ne pas créer de cache non borné.
4. Après publication réussie du fichier, le destinataire émet un reçu signé et chiffré vers la clé KEM de retour. Le reçu lie au minimum la version, les deux `node_id`, le `transfer_id`, l’état `file_published`, la taille, le BLAKE3 et le jeton.
5. Le destinataire envoie le reçu par sa connexion relay déjà enregistrée. L’expéditeur garde sa connexion relay ouverte et lit les réponses pendant une durée bornée. Il n’accepte que le reçu lié au transfert attendu, émis par le `node_id` visé, dont le jeton, la signature, la taille et le BLAKE3 correspondent.

Le relay actuel ne conserve aucune file durable et abandonne les fragments si la destination est hors ligne. Un accusé ne peut donc être remis que si le destinataire et la connexion de l’expéditeur sont encore joignables. Sans accusé dans le délai, l’interface doit distinguer **« transmis, accusé absent »** de **« fichier publié chez le destinataire »**.

## Compatibilité et sécurité

- L’extension doit être facultative et ignorable par les anciennes versions afin qu’un nouveau client puisse encore transmettre le fichier à un ancien destinataire. Une ancienne version ne renverra pas d’accusé : le nouveau client affichera alors l’état non confirmé après son délai d’attente.
- La suppression ou la corruption de l’extension peut faire perdre l’accusé, mais ne doit pas permettre d’en fabriquer un. Le chiffrement authentifié par la clé de session du bloc doit protéger cette extension contre la falsification par le relay.
- Une signature du destinataire apporte une attribution cryptographique à sa clé ML-DSA. La vérification d’une clé déjà épinglée dans `peers.json` doit être appliquée. Si le destinataire est nouveau, le jeton de transfert à usage unique lie le reçu à un pair capable de déchiffrer le transfert ciblé; l’interface ne doit pas transformer cette seule preuve en confiance générale durable sans suivre la règle TOFU existante.
- Les reçus rejoués doivent être idempotents : ils ne créent pas un second fichier et ne changent pas l’état d’un transfert différent. Toute logique de reçu doit rester indépendante du TTL de sessions pré-authentification.
- Un récepteur peut être compromis ou mentir sur une écriture locale; aucun protocole réseau ne prouve à distance l’état physique du disque. Le nom de l’état doit rester une attestation du logiciel, pas une garantie matérielle.

## Tests d’acceptation à ajouter avant clôture P2

1. Transfert complet : le reçu n’est créé qu’après assemblage de tous les blocs et réussite de la publication; taille et BLAKE3 correspondent aux deux extrémités.
2. Fragmentation incomplète, bloc manquant, en-têtes incohérents ou échec d’écriture : aucun reçu positif n’est émis.
3. Mauvais `transfer_id`, mauvais pair, jeton erroné, hash/taille modifiés, signature invalide ou extension AEAD corrompue : reçu rejeté et état expéditeur non confirmé.
4. Accusé perdu, retardé, dupliqué ou reçu après expiration : résultat borné et idempotent; aucun faux succès ni fichier dupliqué.
5. Récepteur ancien/sans extension : transfert toujours lisible, mais l’expéditeur rapporte « transmis, accusé absent » après son délai.
6. Le relay ne reçoit ni clé privée de retour ni jeton en clair, ne stocke pas de reçu, et n’est pas nécessairement capable de confirmer la publication.
7. Smoke loopback bout en bout, puis essai réel entre deux hôtes avec versions, timeout, état, tailles et hashes consignés séparément.

## Décisions requises avant implémentation

- Choisir le comportement CLI : attente d’accusé par défaut pour `envoyer --fichier --via`, option explicite, ou délai configurable.
- Choisir la durée d’attente et le code de sortie lorsque le fichier a été transmis mais qu’aucun accusé n’est reçu.
- Définir si une reprise/réémission d’un reçu est nécessaire, et sa politique d’idempotence après reconnexion, alors que le relay ne stocke rien.
- Confirmer si le premier jalon couvre uniquement les fichiers Drive ou également les messages.
- Nommer le propriétaire de la revue protocole et de l’interopérabilité.

**Recommandation de départ :** implémenter d’abord l’accusé facultatif pour les transferts de fichiers uniquement, avec état CLI explicite, corrélation `transfer_id + node_id + jeton + taille + BLAKE3`, attente bornée et tests de compatibilité ancienne version. Ne modifier ni le TTL pré-authentification ni les responsabilités de Polygone Compute/HYPER dans cette passe.
