# Polygone Gateway

Le composant Gateway permet d’exposer des ressources du réseau Polygone via HTTP classique. Il agit comme un pont entre HTTP et le réseau P2P.

## Déploiement rapide (5 min)

### Render

1. New ▸ Web Service.
2. Connecter ce dépôt.
3. Runtime : Docker (Dockerfile à la racine).
4. Plan : Free.
5. Variables :
   - `PORT` = 3000 (Render le définit automatiquement si non spécifié)
6. Create Web Service.

### Fly.io

```bash
fly launch
# Choisir le Dockerfile existant
fly secrets set PORT=3000
fly deploy
```

### Railway

1. New Project ▸ Deploy from repo.
2. Choisir ce dépôt.
3. Railway détectera le Dockerfile et déploiera.
4. Définir la variable `PORT=3000` dans les settings.

## Utilisation

Une fois déployée, la gateway répond sur le port 3000 (ou `PORT`).

- `GET /` — informations de base
- `GET /sites/<path>` — récupérer une ressource du réseau (simulé pour l’instant)

Exemple : `https://votre-gateway.fly.dev/sites/index.html`

## Architecture future

La gateway contactera le nœud Polygone local via HTTP (port 8080) ou un socket IPC pour demander la ressource réelle sur le réseau P2P (DHT, blocs chiffrés). Elle fera office de proxy traduisant HTTP ↔ Protocole Polygone.

## Exemple d'utilisation complet

1. Démarrez un nœud Polygone avec le Drive activé (fichiers stockés localement).
2. Uploadez un fichier `demo.txt` via l'interface Drive (ou l'API).
3. Récupérez son identifiant (CID), par exemple `demo.txt`.
4. Depuis la gateway (ex: https://votre-gateway.fly.dev), accédez-y via :

```
https://votre-gateway.fly.dev/sites/demo.txt
```

La gateway interroge le nœud via `GET /p2p/get/demo.txt` et renvoie le contenu au navigateur.
