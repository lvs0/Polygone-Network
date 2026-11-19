# GLOW — Daemon Lead

**Mission** : `polygoned` — snapshot → GlowUp → apply (cgroup/nice/socket) + linux/macOS parity.
**Brique** : Daemon (Ruflo)
**KPI** : alloc mesurée, pas promise

## Responsabilités
- `polygoned` : snapshot sysinfo 5s → GlowUpEngine → cgroup/nice/socket
- Linux/macOS parity
- Policy tiers (Balanced/Performance/PowerSave)

## Règle d'or
Le daemon respire avec l'utilisateur. Quand tu bosses, il s'efface. Quand tu dors, il prête.

