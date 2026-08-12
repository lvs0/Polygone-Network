# Deployment — Polygone Ghost Node

> *Basé sur les patterns r-labs/polygone (ZAB QUARTZ) et les meilleures pratiques Docker/Render/Railway/Fly.io.*

---

## Concept

Un **Ghost Node** est un nœud Polygone qui tourne en permanence sur un serveur gratuit (Render, Railway, Fly.io) pour :

- **Maintenir la présence réseau** (heartbeat au bootstrap)
- **Recevoir des messages** quand votre machine locale est éteinte
- **Servir de relay** si votre réseau local est NATé/firewallé

L'anti-veille n'est PAS du faux trafic : chaque tick est une véritable annonce (heartbeat) au bootstrap, qui maintient le nœud dans le registre des nœuds vivants.

---

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Ghost Node (Render)                       │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────┐  ┌─────────────────┐  ┌────────────────┐ │
│  │ Bootstrap   │  │ Message Listen  │  │ Heartbeat      │ │
│  │ :4243       │  │ :4242           │  │ (120s loop)    │ │
│  └─────────────┘  └─────────────────┘  └────────────────┘ │
└─────────────────────────────────────────────────────────────┘
           ▲                    ▲                    ▲
           │                    │                    │
           └────────────────────┴────────────────────┘
                              │
                    ┌─────────┴─────────┐
                    │   Your Computer   │
                    │   (polygone)      │
                    └───────────────────┘
```

---

## Déploiement

### Option 1 : Docker (recommandé)

```bash
# Build
docker build -f docs/deployment/Dockerfile -t polygone-ghost .

# Run
docker run -d \
  --name polygone-ghost \
  -e ZAB_ALIAS="mon-ghost" \
  -e ZAB_PUB_ADDR="ghost.example.com:4242" \
  -e BOOTSTRAP="bootstrap.polygone.network:4243" \
  -v polygone-data:/data \
  polygone-ghost
```

### Option 2 : Render (free tier)

1. Fork ce repo
2. New → Web Service
3. Build Command : `docker build -f docs/deployment/Dockerfile .`
4. Start Command : `docker run -e ZAB_ALIAS=$RENDER_SERVICE_NAME ...`
5. Variables d'environnement :
   - `ZAB_ALIAS` : nom du nœud (ex: `ghost-paris`)
   - `ZAB_PUB_ADDR` : adresse publique (fournie par Render)
   - `BOOTSTRAP` : adresse du bootstrap (défaut : `127.0.0.1:4243`)

### Option 3 : Railway / Fly.io

Similaire à Render — voir leurs docs respectives pour Docker deployment.

---

## Configuration

| Variable | Défaut | Description |
|----------|--------|-------------|
| `ZAB_HOME` | `/data` | Répertoire d'état du nœud |
| `ZAB_PORT` | `4242` | Port d'écoute des messages |
| `ZAB_ALIAS` | `ghost` | Alias du nœud (visible dans le réseau) |
| `ZAB_PUB_ADDR` | `127.0.0.1:4242` | Adresse publique pour les pairs |
| `BOOTSTRAP` | `127.0.0.1:4243` | Adresse du bootstrap |
| `HEARTBEAT_SECS` | `120` | Intervalle de heartbeat (secondes) |

---

## Sécurité

- **Identité persistante** : la clé du nœud ne change jamais entre redémarrages (`$ZAB_HOME/node.key`)
- **Pas de faux trafic** : le heartbeat est une annonce réelle au bootstrap
- **Isolation** : le nœud ne peut pas exécuter de code arbitraire (pas de shell exposé)

---

## Monitoring

```bash
# Logs
docker logs -f polygone-ghost

# Status
docker exec polygone-ghost ps aux

# Messages reçus
docker exec polygone-ghost ls -la /data/inbox/
```

---

## Références

- [r-labs/polygone](https://github.com/lvs0/r-labs/tree/main/polygone) — patterns ZAB QUARTZ originaux
- [Render Docs](https://render.com/docs/docker)
- [Railway Docs](https://docs.railway.app/deploy/dockerfiles)
- [Fly.io Docs](https://fly.io/docs/languages-and-frameworks/dockerfile/)
