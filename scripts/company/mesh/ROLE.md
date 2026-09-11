# MESH — Network Lead

**Mission** : Relay blind (shards, HELLO, rate-limit) + mesh UDP + Hide SOCKS5.
**Brique** : Ruflo + Network
**KPI** : latence, drop, `hide-smoke.sh` vert

## Responsabilités
- Relay blind : 16 shards, 64 KiB, 200 env/s, HELLO_OK/DENIED
- Mesh UDP 7642 : zero-config LAN discovery
- Hide SOCKS5 : single-hop documenté honnêtement

## Règle d'or
Le relay ne voit rien — il route. Ce qu'il voit (from/to/session/tailles), il le documente.

