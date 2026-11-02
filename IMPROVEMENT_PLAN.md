# 🎯 Plan d'Amélioration Complet — Polygone

> *Document d'action concret et priorisé.*
> *Dernière mise à jour : 2026-06-06*
> *Statut : En cours de rédaction*

---

## Table des matières

1. [État Actuel du Projet](#1-état-actuel-du-projet)
2. [Améliorations Visuelles](#2-améliorations-visuelles)
3. [Stratégie Cross-Platform](#3-stratégie-cross-platform)
4. [Système de Power Lending](#4-système-de-power-lending)
5. [Moteur de Recherche Privé](#5-moteur-de-recherche-privé)
6. [Fonctionnalités Révolutionnaires](#6-fonctionnalités-révolutionnaires)
7. [Priorisation et Roadmap](#7-priorisation-et-roadmap)

---

## 1. État Actuel du Projet

### Ce qui existe

| Composant | Status | Notes |
|-----------|--------|-------|
| **Crypto** | ✅ Fonctionnel | ML-KEM-1024, AES-256-GCM, Shamir 4-of-7, BLAKE3, Ed25519 |
| **TUI Dashboard** | ⚠️ Partiel | 4 onglets (Accueil, Favoris, Services, Paramètres), stats live |
| **Web Dashboard** | ⚠️ Partiel | Landing page complète, node dashboard basique |
| **Compute Daemon** | ⚠️ Linux-only | Idle detection via /proc/*, PID check Linux-only |
| **Search Engine** | ⚠️ Squelette | Index local, pas de requêtes P2P réelles |
| **Economy** | ✅ Fonctionnel | POLY tokens, ledger TOML local, ticker |
| **P2P Network** | ⚠️ Partiel | libp2p intégré mais pas testé à grande échelle |

### Architecture du workspace

```
Polygone-Fresh/
├── polygone/          # Crate principal (TUI, web, crypto, compute)
├── crates/
│   ├── common/        # Types partagés (identity, packet)
│   ├── app/           # Point d'entrée alternatif
│   ├── polygone-msg/  # Messagerie E2E
│   ├── polygone-drive/# Stockage distribué
│   ├── polygone-hide/ # Proxy SOCKS5
│   ├── polygone-mesh/ # Réseau local
│   ├── polygone-brain/# IA locale
│   ├── polygone-search/# Moteur de recherche
│   ├── polygone-compute/# Power lending
│   └── polygone-nodeos/# Système d'exploitation nœud
└── web/               # Assets web (doublon de polygone/web/)
```

### Leçons apprises (vision doc)

- Docker/Render = trous noirs de temps. Rester en local.
- Complexeité creep = dette technique inutile.
- Le TUI est le cœur — sans TUI fonctionnel, pas de produit.
- Un produit à la fois — pas 5 modules en parallèle.
- Constance > vitesse.

---

## 2. Améliorations Visuelles

### 2.1 TUI — Le Cœur de l'Expérience

#### 2.1.1 Améliorations immédiates (Semaine 1-2)

**Objectif** : Le TUI doit être l'interface principale, pas un squelette.

- [ ] **Widget de bienvenue interactif**
  - Animation d'entrée (fade-in des couleurs)
  - Message de statut clair : "Votre nœud est actif depuis X minutes"
  - Indicateur visuel de santé (vert = ok, jaune = warning, rouge = erreur)

- [ ] **Grille de modules améliorée**
  - Cartes avec bordures colorées dynamiques (pulse pour "Running")
  - Icônes plus grandes et plus visibles
  - Barre de progression mini pour chaque module
  - Tooltip de statut au survol (info détaillée)

- [ ] **Barre de statut enrichie**
  - Solde POLY en temps réel avec couleur dynamique (vert = bien, rouge = bas)
  - Indicateur de trafic (↑↓) avec sparkline
  - Nom du nœud raccourci visible

- [ ] **Log d'activité en temps réel**
  - Scroll automatique avec highlight des nouveaux messages
  - Code couleur par type (info=bleu, success=vert, error=rouge, warn=jaune)
  - Timestamp relatif ("il y a 5s", "maintenant")

#### 2.1.2 Nouvelles vues (Semaine 3-4)

- [ ] **Vue "Réseau" (onglet 5)**
  - Topologie visuelle du réseau (ASCII art des nœuds connectés)
  - Latence par nœud (ms)
  - Carte de chaleur des connexions

- [ ] **Vue "Économie" (onglet 6)**
  - Graphique ASCII du solde POLY dans le temps
  - Taux de consommation par service
  - Historique des transactions (earn/spend)
  - Projection : "À ce rythme, votre solde durera X heures"

- [ ] **Vue "Recherche" (onglet 7)**
  - Champ de recherche interactif
  - Résultats en temps réel avec scoring
  - Filtres par source (Anna's Archive, arXiv, PubMed, Wiki)

#### 2.1.3 Design System TUI

```rust
// Palette de couleurs standardisée
const PALETTE: Palette = Palette {
    background: SLATE_900,    // #0f172a
    surface: SLATE_800,       // #1e293b
    border: SLATE_700,        // #334155
    text: SLATE_50,           // #f8fafc
    text_dim: SLATE_400,      // #94a3b8
    accent: CYBER,            // #22d3ee
    success: GREEN,           // #22c55e
    warning: AMBER,           // #fbbd24
    error: ROSE,              // #fb7185
    info: VIOLET,             // #a78bfa
};
```

### 2.2 Web Dashboard — Refonte Complète

#### 2.2.1 Landing Page (existante, à améliorer)

- [ ] **Animations CSS**
  - Particules de fond animées (réseau de nœuds)
  - Scroll-triggered animations pour les sections
  - Hover effects améliorés sur les cartes modules

- [ ] **Section "En direct"**
  - Compteur de nœuds actifs en temps réel
  - Dernier message chiffré (anonymisé)
  - Graphique de trafic du réseau

- [ ] **Section "Sécurité"**
  - Visualisation du flux de chiffrement (schéma animé)
  - Comparaison avec les autres solutions (Tor, Signal, etc.)
  - Audit logique : "Vos données transitent par N nœuds"

#### 2.2.2 Node Dashboard (exister, à refaire)

- [ ] **Design moderne**
  - Dark mode par défaut (compatible avec la palette TUI)
  - Layout responsive (mobile-first)
  - Composants réutilisables (cards, badges, progress bars)

- [ ] **Fonctionnalités**
  - Contrôle des modules (toggle on/off)
  - Visualisation du réseau (graphe interactif)
  - Console de logs en temps réel (WebSocket)
  - Paramètres avancés (ports, seuils, thresholds)

- [ ] **Performance**
  - Lazy loading des composants
  - Cache HTTP approprié
  - Compression gzip/brotli

---

## 3. Stratégie Cross-Platform

### 3.1 Problème Actuel

Le compute daemon (`idle.rs`) utilise des appels Linux-spécifiques :
- `/proc/stat` pour CPU
- `/proc/meminfo` pour RAM
- `/proc/{pid}` pour la vérification du daemon
- `xprintidle` pour l'activité utilisateur

**Impact** : Le power lending ne fonctionne que sur Linux.

### 3.2 Solution : Abstraction Cross-Platform

#### 3.2.1 Module `system-info` (Semaine 1-2)

Créer un crate `polygone-system` avec :

```rust
// polygone-system/src/lib.rs
pub trait SystemMonitor {
    fn cpu_usage(&self) -> f32;
    fn ram_usage(&self) -> RamInfo;
    fn idle_time(&self) -> Duration;
    fn is_user_active(&self) -> bool;
}

pub struct RamInfo {
    pub used: u64,
    pub total: u64,
    pub available: u64,
}

// Implémentations par plateforme
#[cfg(target_os = "linux")]
pub struct LinuxMonitor { /* /proc/* */ }

#[cfg(target_os = "macos")]
pub struct MacosMonitor { /* sysctl, IOKit */ }

#[cfg(target_os = "windows")]
pub struct WindowsMonitor { /* WMI, Performance Counters */ }
```

#### 3.2.2 Implémentation par OS (Semaine 3-4)

**Linux** (existante) :
- `/proc/stat` → CPU
- `/proc/meminfo` → RAM
- `/proc/uptime` + CPU ratio → idle
- `xprintidle` ou `loginctl` → activité utilisateur

**macOS** :
- `sysctl hw.logicalcpu` → CPU cores
- `vm_stat` → pages mémoire
- `ioreg` → batterie, thermique
- `HIDServiceClient` → dernière activité clavier/souris

**Windows** :
- WMI `Win32_Processor` → CPU usage
- WMI `Win32_OperatingSystem` → RAM
- `GetLastInputInfo()` → activité utilisateur
- Performance Counters → métriques détaillées

#### 3.2.3 Détection automatique au runtime

```rust
pub fn create_monitor() -> Box<dyn SystemMonitor> {
    #[cfg(target_os = "linux")]
    { Box::new(LinuxMonitor::new()) }
    #[cfg(target_os = "macos")]
    { Box::new(MacosMonitor::new()) }
    #[cfg(target_os = "windows")]
    { Box::new(WindowsMonitor::new()) }
}
```

### 3.3 Installation Cross-Platform

#### 3.3.1 Linux
```bash
curl -fsSL https://raw.githubusercontent.com/lvs0/Polygone/main/install.sh | bash
```
- Détection : apt/yum/pacman
- Service : systemd
- Binaire : statique musl

#### 3.3.2 macOS
```bash
curl -fsSL https://raw.githubusercontent.com/lvs0/Polygone/main/install-macos.sh | bash
```
- Détection : Homebrew
- Service : launchd
- Binaire : universal binary (x86_64 + aarch64)

#### 3.3.3 Windows
```powershell
irm https://raw.githubusercontent.com/lvs0/Polygone/main/install.ps1 | iex
```
- Détection : winget/chocolatey/scoop
- Service : Windows Service (SCM)
- Binaire : .exe signé (code signing)

### 3.4 Binaire Unique Cross-Platform

Objectif : Un seul binaire `polygone` qui :
1. Détecte l'OS au démarrage
2. Charge le bon module système
3. Affiche le TUI (le même sur tous les OS)
4. Lance le daemon compute (avec la bonne implémentation)

```toml
# Cargo.toml
[target.'cfg(target_os = "linux")'.dependencies]
nix = "0.29"

[target.'cfg(target_os = "macos")'.dependencies]
core-foundation = "0.10"
mach2 = "0.4"

[target.'cfg(target_os = "windows")'.dependencies]
windows-sys = { version = "0.59", features = ["Win32_System_SystemInformation"] }
```

---

## 4. Système de Power Lending

### 4.1 Concept

Le power lending est le système où les utilisateurs prêtent leurs ressources inutilisées (RAM, CPU, GPU) au réseau Polygone en échange de tokens POLY.

**Principe** :
1. Le daemon detecte l'inactivité (5+ minutes sans interaction)
2. Il alloue une fraction des ressources (max 50% RAM, 80% CPU)
3. Les tâches sont chiffrées et distribuées via le réseau P2P
4. L'utilisateur gagne des tokens POLY
5. Dès que l'utilisateur revient, le lending s'arrête instantanément

### 4.2 Architecture du Système

#### 4.2.1 Compute Daemon (existant, à améliorer)

```
┌─────────────────────────────────────────┐
│           Compute Daemon                 │
├─────────────────────────────────────────┤
│  ┌─────────────┐  ┌─────────────────┐  │
│  │ Idle        │  │ Resource        │  │
│  │ Detector    │  │ Monitor         │  │
│  │ (cross-plat)│  │ (CPU/RAM/GPU)   │  │
│  └──────┬──────┘  └────────┬────────┘  │
│         │                  │            │
│         ▼                  ▼            │
│  ┌─────────────────────────────────┐   │
│  │        Decision Engine          │   │
│  │  - Seuil d'inactivité (5min)   │   │
│  │  - Max RAM (50%)               │   │
│  │  - Max CPU (80%)               │   │
│  │  - Priorité utilisateur         │   │
│  └──────────────┬──────────────────┘   │
│                 │                       │
│                 ▼                       │
│  ┌─────────────────────────────────┐   │
│  │      Task Scheduler             │   │
│  │  - Reçoit les tâches chiffrées  │   │
│  │  - Les exécute en sandbox       │   │
│  │  - Retourne les résultats       │   │
│  └──────────────┬──────────────────┘   │
│                 │                       │
│                 ▼                       │
│  ┌─────────────────────────────────┐   │
│  │      POLY Token Ledger          │   │
│  │  - Calcule les gains            │   │
│  │  - Met à jour le solde          │   │
│  │  - Persiste sur disque          │   │
│  └─────────────────────────────────┘   │
└─────────────────────────────────────────┘
```

#### 4.2.2 Types de Tâches Lendables

| Type | Description | Gain POLY/min | Risque |
|------|-------------|---------------|--------|
| **Inference IA** | Exécution de modèles Ollama/Notch | 0.5 | Faible |
| **Calcul scientifique** | Simulations, rendu | 0.3 | Faible |
| **Stockage temporaire** | Cache de fragments | 0.1 | Très faible |
| **Relais réseau** | Transit de messages chiffrés | 0.2 | Faible |
| **Indexation** | Index de recherche distribué | 0.15 | Très faible |

#### 4.2.3 Mécanisme de Sécurité

```rust
// Séparation stricte entre ressources utilisateur et lending
pub struct ResourceIsolation {
    /// RAM maximale pour le lending (défaut: 50%)
    pub max_ram_fraction: f32,
    /// CPU maximal pour le lending (défaut: 80%)
    pub max_cpu_fraction: f32,
    /// Seuil de priorité utilisateur (au-dessus = lending pause)
    pub user_priority_threshold: f32,
    /// Sandbox pour les tâches exécutées
    pub sandbox_enabled: bool,
    /// Timeout par tâche (défaut: 30s)
    pub task_timeout: Duration,
}

impl ResourceIsolation {
    pub fn can_lend(&self, current: &SystemMetrics) -> bool {
        // Règle 1: L'utilisateur doit être inactif
        if !current.is_idle {
            return false;
        }
        
        // Règle 2: RAM disponible suffisante
        if current.ram_fraction() > self.max_ram_fraction {
            return false;
        }
        
        // Règle 3: CPU disponible suffisant
        if current.cpu_usage > self.max_cpu_fraction {
            return false;
        }
        
        true
    }
}
```

### 4.3 Intégration Ollama

#### 4.3.1 Détection automatique

```rust
pub struct OllamaIntegration {
    binary_path: String,
    models: Vec<String>,
    is_running: bool,
}

impl OllamaIntegration {
    pub fn detect() -> Self {
        // 1. Vérifier si ollama est installé
        // 2. Lister les modèles disponibles
        // 3. Vérifier le serveur Ollama (port 11434)
        // 4. Si activer, partager les modèles via le réseau
    }
    
    pub async fn serve_model(&self, model: &str, request: InferenceRequest) -> InferenceResponse {
        // 1. Reçoit la requête chiffrée
        // 2. Déchiffre localement
        // 3. Exécute l'inférence via Ollama
        // 4. Chiffre la réponse
        // 5. Retourne au demandeur
    }
}
```

#### 4.3.2 Modèles supportés (Phase 1)

- **Légers** (< 2GB) : phi-3-mini, gemma-2b, qwen2-1.5b
- **Moyens** (2-7GB) : llama-3-8b, mistral-7b, mixtral-8x7b
- **Lourds** (> 7GB) : llama-3-70b (via GPU uniquement)

### 4.4 Système de Récompenses

#### 4.4.1 Calcul des gains

```rust
pub fn calculate_earnings(
    lent_duration: Duration,
    resource_type: ResourceType,
    network_demand: f32, // 0.0 à 1.0
) -> f64 {
    let base_rate = match resource_type {
        ResourceType::Cpu => 0.3,    // POLY/min
        ResourceType::Ram => 0.1,    // POLY/min
        ResourceType::Gpu => 1.0,    // POLY/min
        ResourceType::Inference => 0.5, // POLY/min
        ResourceType::Relay => 0.2,  // POLY/min
    };
    
    // Multiplicateur de demande réseau
    let demand_multiplier = 1.0 + (network_demand * 0.5);
    
    // Multiplicateur de durée (bonus pour les longues sessions)
    let duration_multiplier = if lent_duration > Duration::from_secs(3600) {
        1.2 // +20% après 1 heure
    } else if lent_duration > Duration::from_secs(1800) {
        1.1 // +10% après 30 minutes
    } else {
        1.0
    };
    
    let minutes = lent_duration.as_secs_f64() / 60.0;
    base_rate * minutes * demand_multiplier * duration_multiplier
}
```

#### 4.4.2 Distribution des tokens

- **Émission** : Les tokens POLY sont émis localement (pas de blockchain)
- **Persistance** : Ledger TOML dans `~/.polygone/poly.toml`
- **Validation** : Chaque nœud vérifie ses propres gains
- **Transfert** : Via le réseau P2P (signé Ed25519)

### 4.5 Interface Utilisateur

#### 4.5.1 TUI - Onglet "Power"

Nouveau onglet dédié au power lending :

```
┌─────────────────────────────────────────────────────────────┐
│ ⚡ Power Lending                                            │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  État: ● Lending actif                                      │
│  Depuis: 45 minutes                                         │
│                                                             │
│  ┌─ Ressources ──────────────────────────────────────────┐ │
│  │ CPU   ████████░░░░░░░░░░░░  40% (max 80%)            │ │
│  │ RAM   ██████░░░░░░░░░░░░░░  30% (max 50%)            │ │
│  │ GPU   ░░░░░░░░░░░░░░░░░░░░  0% (non détecté)        │ │
│  └───────────────────────────────────────────────────────┘ │
│                                                             │
│  ┌─ Gains ───────────────────────────────────────────────┐ │
│  │ Aujourd'hui:     +12.50 POLY                          │ │
│  │ Cette semaine:   +87.30 POLY                           │ │
│  │ Total cumulé:    245.80 POLY                           │ │
│  └───────────────────────────────────────────────────────┘ │
│                                                             │
│  ┌─ Tâches en cours ─────────────────────────────────────┐ │
│  │ 1. Inference phi-3-mini (32s restantes)                │ │
│  │ 2. Cache fragment #4a2f (permanent)                    │ │
│  │ 3. Relais réseau (3 pairs)                             │ │
│  └───────────────────────────────────────────────────────┘ │
│                                                             │
│  [S]top lending  [P]ause  [M]odèles Ollama  [R]afraîchir  │
└─────────────────────────────────────────────────────────────┘
```

#### 4.5.2 Web Dashboard - Section Power

- Graphique temps réel de l'utilisation des ressources
- Historique des gains (24h, 7j, 30j)
- Liste des modèles Ollama partagés
- Paramètres avancés (seuils, priorités, exclusions)

---

## 5. Moteur de Recherche Privé

### 5.1 Concept

Un moteur de recherche décentralisé qui fonctionne entirely sur le réseau Polygone. Pas de serveur central, pas de tracking, pas de publicité.

**Principe** :
1. Chaque nœud indexe localement une partie du web
2. Les requêtes sont routées via le réseau P2P (k-anonymat)
3. Les résultats sont assemblés à partir de plusieurs nœuds
4. Aucun nœud ne voit la requête complète

### 5.2 Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Utilisateur                               │
│                  (TUI ou Web)                                │
└──────────────────────┬──────────────────────────────────────┘
                       │ Requête chiffrée
                       ▼
┌─────────────────────────────────────────────────────────────┐
│              Search Coordinator (local)                      │
├─────────────────────────────────────────────────────────────┤
│  1. Parse la requête                                        │
│  2. Chiffre avec ML-KEM-1024                               │
│  3. Fragmente en 7 parts (Shamir)                          │
│  4. Envoie à 4-7 pairs aléatoires                          │
│  5. Attend les résultats (timeout 5s)                      │
│  6. Assemble et classe les résultats                        │
│  7. Affiche à l'utilisateur                                 │
└──────────────────────┬──────────────────────────────────────┘
                       │
          ┌────────────┼────────────┐
          ▼            ▼            ▼
    ┌──────────┐ ┌──────────┐ ┌──────────┐
    │ Nœud A   │ │ Nœud B   │ │ Nœud C   │
    │ (arXiv)  │ │ (Wiki)   │ │ (PubMed) │
    └──────────┘ └──────────┘ └──────────┘
```

### 5.3 Sources de Données

#### 5.3.1 Sources Fédérées (Phase 1)

| Source | Type | Taille index | Mise à jour |
|--------|------|--------------|-------------|
| **Anna's Archive** | Livres, articles | ~50GB | Hebdomadaire |
| **arXiv** | Preprints scientifiques | ~100GB | Quotidien |
| **PubMed** | Biomédical | ~30GB | Quotidien |
| **Wikipedia** | Encyclopédie | ~20GB | Mensuel |
| **PolyMesh** | Contenu partagé | Variable | Temps réel |

#### 5.3.2 Sources Personnalisables (Phase 2)

```toml
# ~/.polygone/search.toml
[sources]
custom = [
    { name = "Mon Blog", url = "https://monblog.com/feed", type = "rss" },
    { name = "Docs Rust", url = "https://doc.rust-lang.org", type = "web" },
]

[settings]
max_results = 20
timeout_ms = 5000
safe_search = true
language = "fr"
```

### 5.4 Indexation Distribuée

#### 5.4.1 Stratégie d'indexation

Chaque nœud indexe une partie du web selon sa capacité :

```rust
pub struct IndexStrategy {
    /// Capacité de stockage dédiée (en GB)
    pub storage_capacity: u64,
    /// Sources assignées
    pub assigned_sources: Vec<DataSource>,
    /// Priorité d'indexation
    pub priority: IndexPriority,
}

pub enum IndexPriority {
    /// Index en arrière-plan (faible priorité)
    Background,
    /// Index quand le système est idle
    IdleOnly,
    /// Index en continu (haute priorité)
    Continuous,
}
```

#### 5.4.2 Format d'index

```rust
pub struct SearchIndex {
    /// Index inversé (terme → liste de documents)
    inverted_index: HashMap<String, Vec<DocumentRef>>,
    /// Métadonnées des documents
    documents: HashMap<DocumentId, DocumentMeta>,
    /// Cache LRU pour les requêtes fréquentes
    query_cache: LruCache<String, Vec<SearchResult>>,
    /// Statistiques
    stats: IndexStats,
}

pub struct DocumentRef {
    pub doc_id: DocumentId,
    pub score: f32,
    pub positions: Vec<usize>, // positions du terme dans le doc
    pub frequency: u32,
}
```

### 5.5 Protocole de Recherche

#### 5.5.1 Format des requêtes

```rust
#[derive(Serialize, Deserialize)]
pub struct SearchRequest {
    /// ID unique de la requête
    pub request_id: [u8; 32],
    /// Termes de recherche
    pub terms: Vec<String>,
    /// Sources demandées
    pub sources: Vec<DataSource>,
    /// Nombre max de résultats
    pub max_results: usize,
    /// Timeout en ms
    pub timeout_ms: u64,
    /// Signature Ed25519 (pour la réputation)
    pub signature: [u8; 64],
}

#[derive(Serialize, Deserialize)]
pub struct SearchResponse {
    /// ID de la requête correspondante
    pub request_id: [u8; 32],
    /// Résultats trouvés
    pub results: Vec<SearchResult>,
    /// Score de pertinence global
    pub relevance_score: f32,
    /// Temps de traitement
    pub processing_ms: u64,
}
```

#### 5.5.2 Routeur de requêtes

```rust
pub struct SearchRouter {
    /// Nœuds connus avec leurs capacités
    peers: HashMap<PeerId, PeerCapabilities>,
    /// Table de routage (terme → nœuds compétents)
    routing_table: HashMap<String, Vec<PeerId>>,
    /// Cache des résultats récents
    result_cache: LruCache<[u8; 32], Vec<SearchResult>>,
}

impl SearchRouter {
    pub async fn route(&mut self, request: &SearchRequest) -> Vec<SearchResult> {
        // 1. Sélectionner les nœuds compétents
        let candidates = self.select_candidates(&request.terms);
        
        // 2. Fragmenter la requête (Shamir)
        let fragments = self.fragment_request(request);
        
        // 3. Envoyer aux nœuds (k-anonymat)
        let responses = self.send_to_peers(fragments, &candidates).await;
        
        // 4. Assembler les résultats
        self.assemble_results(responses)
    }
    
    fn select_candidates(&self, terms: &[String]) -> Vec<PeerId> {
        // Sélectionner les nœuds qui ont les sources pertinentes
        // et une bonne réputation
        self.peers.iter()
            .filter(|(_, caps)| caps.has_relevant_sources(terms))
            .filter(|(_, caps)| caps.reputation > 0.7)
            .map(|(id, _)| *id)
            .take(7)
            .collect()
    }
}
```

### 5.6 Interface Utilisateur

#### 5.6.1 TUI - Vue Recherche

```
┌─────────────────────────────────────────────────────────────┐
│ 🔍 Recherche Privée                                         │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  Rechercher: [post-quantum cryptography____________]        │
│                                                             │
│  Sources: [✓] Anna's Archive  [✓] arXiv  [✓] PubMed       │
│           [✓] Wiki  [ ] PolyMesh  [ ] Custom               │
│                                                             │
│  ─────────────────────────────────────────────────────────  │
│                                                             │
│  1. [arXiv] ML-KEM: A Practical Guide to Post-Quantum...   │
│     Score: 0.95 | 2024-03-15 | 1.2 MB                     │
│                                                             │
│  2. [Wiki] Post-quantum cryptography - Wikipedia           │
│     Score: 0.88 | 2024-01-20 | 45 KB                      │
│                                                             │
│  3. [PubMed] Quantum-resistant key exchange for medical... │
│     Score: 0.82 | 2024-02-10 | 890 KB                     │
│                                                             │
│  ─────────────────────────────────────────────────────────  │
│  Résultats: 3 trouvés | Temps: 1.2s | Nœuds: 4 contactés  │
│                                                             │
│  [Enter] Ouvrir  [F]iltrer  [S]auvegarder  [Q] Requête    │
└─────────────────────────────────────────────────────────────┘
```

### 5.7 Sécurité et Vie Privée

#### 5.7.1 Protections

- **k-anonymat** : Chaque requête est envoyée à k nœuds minimum
- **Chiffrement** : ML-KEM-1024 pour les échanges de clés
- **Fragmentation** : Shamir 4-of-7 pour les réponses
- **Pas de logs** : Aucun nœud ne garde de trace des requêtes
- **DNS over Polygone** : Résolution de noms via le réseau P2P

#### 5.7.2 Contre-mesures

```rust
pub struct AntiTracking {
    /// Bruit aléatoire ajouté aux requêtes
    pub noise_level: f32,
    /// Délai aléatoire avant envoi
    pub random_delay: Duration,
    /// Rotation des identifiants de session
    pub session_rotation: bool,
    /// Obfuscation des patterns de requêtes
    pub pattern_obfuscation: bool,
}
```

---

## 6. Fonctionnalités Révolutionnaires

### 6.1 Polygone Brain — IA Locale Distribuée

#### 6.1.1 Concept

Un assistant IA qui fonctionne entirely offline, avec la possibilité de "prêter" son cerveau à d'autres nœuds du réseau.

**Révolution** : L'IA n'est pas dans le cloud. Elle est chez toi. Et tu peux la partager.

#### 6.1.2 Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Polygone Brain                            │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌─────────────────┐    ┌─────────────────┐               │
│  │   Modèle Local  │    │  Modèle Distribué│               │
│  │   (Ollama)      │    │  (Petals)        │               │
│  │                 │    │                  │               │
│  │  - phi-3-mini   │    │  - llama-3-70b   │               │
│  │  - gemma-2b     │    │  - mixtral-8x22b │               │
│  │  - qwen2-1.5b   │    │  - command-r+    │               │
│  └────────┬────────┘    └────────┬────────┘               │
│           │                      │                         │
│           └──────────┬───────────┘                         │
│                      ▼                                     │
│           ┌─────────────────────┐                          │
│           │   Routeur d'Inférence│                          │
│           │                     │                          │
│           │  1. Tenter local    │                          │
│           │  2. Si échec → dist │                          │
│           │  3. Chiffrer les    │                          │
│           │     prompts/réponses│                          │
│           │  4. Cache des       │                          │
│           │     conversations   │                          │
│           └─────────────────────┘                          │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

#### 6.1.3 Fonctionnalités

- [ ] **Chat local** : Conversation avec un modèle IA 100% offline
- [ ] **Traduction** : Modèles de traduction légers (NLLB, MADLAD)
- [ ] **Résumé** : Résumé automatique de documents
- [ ] **Code** : Aide au codage (CodeLlama, DeepSeek-Coder)
- [ ] **Recherche sémantique** : Compréhension de la requête
- [ ] **Pétition distribuée** : Demander une inférence à plusieurs nœuds

#### 6.1.4 Protocole msh (Model Sharing Hub)

```rust
pub struct MshProtocol {
    /// Handshake : échange de capacités
    pub async fn handshake(&self, peer: &PeerId) -> MshCapabilities,
    /// Request : demander une inférence
    pub async fn infer(&self, request: InferenceRequest) -> InferenceResponse,
    /// Share : partager un modèle
    pub async fn share_model(&self, model: &ModelInfo) -> ModelHandle,
    /// Stream : inférence en streaming
    pub async fn stream_infer(&self, request: StreamRequest) -> StreamResponse,
}
```

### 6.2 Polygone Hide 2.0 — Proxy Invisible

#### 6.2.1 Améliorations

Le proxy SOCKS5 actuel peut devenir un véritable système d'anonymat :

- [ ] **Multi-hop routing** : 3-5 sauts aléatoires
- [ ] **Domain fronting** : Cacher le trafic Polygone derrière des CDNs
- [ ] **Obfuscation** : Camoufler le trafic Polygone en HTTPS normal
- [ ] **Kill switch** : Bloquer tout trafic non-Polygone si le proxy tombe
- [ ] **DNS over Polygone** : Résolution de noms sans DNS public

#### 6.2.2 Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Polygone Hide 2.0                         │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  Navigateur                                                 │
│     │                                                       │
│     ▼ SOCKS5                                                │
│  ┌─────────────────┐                                       │
│  │ Proxy Local     │ ← Obfuscation layer                   │
│  │ (port 9050)     │                                       │
│  └────────┬────────┘                                       │
│           │ Chiffré (AES-256-GCM)                          │
│           ▼                                                 │
│  ┌─────────────────┐                                       │
│  │ Nœud Sortie 1   │ ← Premier saut                       │
│  │ (aléatoire)     │                                       │
│  └────────┬────────┘                                       │
│           │                                                 │
│           ▼                                                 │
│  ┌─────────────────┐                                       │
│  │ Nœud Sortie 2   │ ← Deuxième saut                      │
│  │ (aléatoire)     │                                       │
│  └────────┬────────┘                                       │
│           │                                                 │
│           ▼                                                 │
│  ┌─────────────────┐                                       │
│  │ Nœud Sortie 3   │ ← Sortie finale                      │
│  │ (aléatoire)     │                                       │
│  └────────┬────────┘                                       │
│           │                                                 │
│           ▼                                                 │
│     Internet (IP du nœud de sortie)                        │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### 6.3 Polygone Drive 2.0 — Stockage Souverain

#### 6.3.1 Concepts Révolutionnaires

- [ ] **Liens éphémères** : Les fichiers disparaissent après consultation
- [ ] **Accès basé sur le temps** : "Ce fichier est accessible pendant 5 minutes"
- [ ] **Fragmentation vivante** : Les fragments se déplacent entre nœuds
- [ ] **Preuve de destruction** : Confirmation cryptographique qu'un fichier a été supprimé
- [ ] **Sauvegarde croisée** : Tes fichiers sont sur les ordinateurs de tes amis

#### 6.3.2 Interface

```
┌─────────────────────────────────────────────────────────────┐
│ 📁 Polygone Drive                                           │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  Espace utilisé: 2.3 GB / 10.0 GB                          │
│  ████████████░░░░░░░░░░░░░░░░░░░░░░░░░  23%                │
│                                                             │
│  ┌─ Mes fichiers ─────────────────────────────────────────┐│
│  │ 📄 document.pdf      2.1 MB   [🔗 Lien] [⏱ Timer] [🗑] ││
│  │ 🎵 musique.mp3      4.5 MB   [🔗 Lien] [⏱ Timer] [🗑] ││
│  │ 📸 photo.jpg        1.8 MB   [🔗 Lien] [⏱ Timer] [🗑] ││
│  │ 📁 dossier/         12.3 MB  [🔗 Lien] [⏱ Timer] [🗑] ││
│  └────────────────────────────────────────────────────────┘│
│                                                             │
│  ┌─ Liens actifs ─────────────────────────────────────────┐│
│  │ 🔗 /s/a1b2c3d4... → document.pdf                      ││
│  │    Expiré dans: 2h 15min | Consultations: 3/10        ││
│  │ 🔗 /s/e5f6g7h8... → musique.mp3                      ││
│  │    Expiré dans: 45min | Consultations: 1/1            ││
│  └────────────────────────────────────────────────────────┘│
│                                                             │
│  [⬆ Upload] [🔗 Nouveau lien] [🗑 Nettoyer] [⚙ Paramètres]│
└─────────────────────────────────────────────────────────────┘
```

### 6.4 Polygone Mesh 2.0 — Réseau Local Autonome

#### 6.4.1 Concepts

Un réseau local qui fonctionne sans internet :

- [ ] **WiFi Direct** : Connexion directe entre appareils
- [ ] **Bluetooth LE** : Pour les petits transferts
- [ ] **Ad-hoc** : Réseau sans routeur
- [ ] **Mesh local** : Chaque appareil est un routeur
- [ ] **Partage de ressources** : CPU, RAM, stockage entre appareils

#### 6.4.2 Cas d'usage

- **Café cyber** : Partager une connexion internet via Polygone
- **Zone de disaster** : Réseau local sans infrastructure
- **École** : Partager des ressources pédagogiques
- **Famille** : Partager fichiers et IA entre appareils

---

## 7. Priorisation et Roadmap

### 7.1 Phase 1 : Fondation (Semaines 1-4)

**Objectif** : Un produit qui marche sur les 3 OS.

| Semaine | Tâche | Priorité | Effort |
|---------|-------|----------|--------|
| 1 | Module `polygone-system` (cross-platform) | 🔴 Critique | 3 jours |
| 1 | Fix idle detection (macOS + Windows) | 🔴 Critique | 2 jours |
| 2 | Fix PID check (cross-platform) | 🔴 Critique | 1 jour |
| 2 | Améliorer TUI (widgets, couleurs) | 🟡 Haute | 3 jours |
| 3 | Test sur macOS (CI) | 🟡 Haute | 2 jours |
| 3 | Test sur Windows (CI) | 🟡 Haute | 2 jours |
| 4 | Binaire release pour les 3 OS | 🔴 Critique | 2 jours |
| 4 | Documentation cross-platform | 🟡 Haute | 1 jour |

### 7.2 Phase 2 : Power Lending (Semaines 5-8)

**Objectif** : Le système de power lending fonctionne et gagne de l'argent.

| Semaine | Tâche | Priorité | Effort |
|---------|-------|----------|--------|
| 5 | Resource isolation améliorée | 🔴 Critique | 3 jours |
| 5 | Intégration Ollama complète | 🟡 Haute | 2 jours |
| 6 | Système de récompenses POLY | 🟡 Haute | 3 jours |
| 6 | TUI "Power" onglet | 🟡 Haute | 2 jours |
| 7 | Task scheduler sécurisé | 🔴 Critique | 3 jours |
| 7 | Monitoring temps réel | 🟢 Moyenne | 2 jours |
| 8 | Tests de charge (10 nœuds) | 🟡 Haute | 2 jours |
| 8 | Documentation power lending | 🟢 Moyenne | 1 jour |

### 7.3 Phase 3 : Recherche Privée (Semaines 9-12)

**Objectif** : Le moteur de recherche fonctionne et est utilisable.

| Semaine | Tâche | Priorité | Effort |
|---------|-------|----------|--------|
| 9 | Indexation locale (arXiv + Wiki) | 🔴 Critique | 3 jours |
| 9 | Protocole de requête P2P | 🔴 Critique | 2 jours |
| 10 | Routeur de requêtes | 🟡 Haute | 3 jours |
| 10 | Interface TUI recherche | 🟡 Haute | 2 jours |
| 11 | k-anonymat et chiffrement | 🔴 Critique | 3 jours |
| 11 | Cache et performance | 🟡 Haute | 2 jours |
| 12 | Tests à grande échelle | 🟡 Haute | 2 jours |
| 12 | Documentation recherche | 🟢 Moyenne | 1 jour |

### 7.4 Phase 4 : Révolution (Semaines 13-16)

**Objectif** : Les fonctionnalités qui font la différence.

| Semaine | Tâche | Priorité | Effort |
|---------|-------|----------|--------|
| 13 | Polygone Brain (chat local) | 🟡 Haute | 3 jours |
| 13 | Protocole msh | 🟡 Haute | 2 jours |
| 14 | Hide 2.0 (multi-hop) | 🟡 Haute | 3 jours |
| 14 | Drive 2.0 (liens éphémères) | 🟡 Haute | 2 jours |
| 15 | Mesh 2.0 (WiFi Direct) | 🟢 Moyenne | 3 jours |
| 15 | Intégration cross-module | 🟡 Haute | 2 jours |
| 16 | Polish final | 🟡 Haute | 3 jours |
| 16 | Release v2.0 | 🔴 Critique | 2 jours |

### 7.5 Métriques de Succès

| Métrique | Objectif Phase 1 | Objectif Phase 2 | Objectif Phase 4 |
|----------|------------------|------------------|------------------|
| **OS support** | Linux + macOS + Windows | Idem | Idem |
| **TUI frames/sec** | 30 fps | 60 fps | 60 fps |
| **Temps de démarrage** | < 2s | < 1s | < 1s |
| **Taille binaire** | < 20 MB | < 15 MB | < 15 MB |
| **Nœuds testés** | 5 locaux | 10 locaux | 50+ publics |
| **Tests unitaires** | 80% coverage | 90% | 95% |
| **Latence recherche** | N/A | < 3s | < 1s |
| **Tokens gagnés/jour** | N/A | 10+ POLY | 50+ POLY |

---

## Annexe A : Structure des Crates

```
Polygone-Fresh/
├── polygone/                    # Crate principal
│   ├── src/
│   │   ├── main.rs             # Point d'entrée CLI
│   │   ├── lib.rs              # Library root
│   │   ├── tui/                # Interface TUI
│   │   │   ├── app.rs          # Application state
│   │   │   ├── views.rs        # Vues (Dashboard, etc.)
│   │   │   ├── widgets.rs      # Widgets réutilisables
│   │   │   └── ...
│   │   ├── web/                # Dashboard web
│   │   ├── crypto/             # Chiffrement
│   │   ├── network/            # P2P networking
│   │   ├── compute/            # Power lending daemon
│   │   ├── economy/            # Tokens POLY
│   │   └── identity/           # Identité nœud
│   └── web/                    # Assets web
│       ├── index.html
│       ├── style.css
│       └── ...
├── crates/
│   ├── common/                 # Types partagés
│   ├── polygone-system/        # [NOUVEAU] Abstraction cross-platform
│   ├── polygone-msg/           # Messagerie
│   ├── polygone-drive/         # Stockage
│   ├── polygone-hide/          # Proxy
│   ├── polygone-mesh/          # Réseau local
│   ├── polygone-brain/         # IA locale
│   ├── polygone-search/        # Moteur recherche
│   ├── polygone-compute/       # Power lending
│   └── polygone-nodeos/        # Système nœud
└── Cargo.toml                  # Workspace
```

## Annexe B : Dépendances Cross-Platform

```toml
# Linux
[target.'cfg(target_os = "linux")'.dependencies]
nix = { version = "0.29", features = ["user", "signal"] }
procfs = "0.17"

# macOS
[target.'cfg(target_os = "macos")'.dependencies]
core-foundation = "0.10"
mach2 = "0.4"
security-framework = "3"

# Windows
[target.'cfg(target_os = "windows")'.dependencies]
windows-sys = { version = "0.59", features = [
    "Win32_System_SystemInformation",
    "Win32_System_Threading",
    "Win32_Foundation",
]}
winapi = { version = "0.3", features = ["winbase", "sysinfoapi"] }
```

## Annexe C : CI/CD Cross-Platform

```yaml
# .github/workflows/ci.yml
name: CI

on: [push, pull_request]

jobs:
  test:
    strategy:
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
        rust: [stable, nightly]
    
    runs-on: ${{ matrix.os }}
    
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@master
        with:
          toolchain: ${{ matrix.rust }}
      
      - name: Test
        run: cargo test --all
      
      - name: Clippy
        run: cargo clippy --all -- -D warnings
      
      - name: Build release
        run: cargo build --release --all

  release:
    needs: test
    runs-on: ubuntu-latest
    if: startsWith(github.ref, 'refs/tags/')
    
    steps:
      - uses: actions/checkout@v4
      
      - name: Build Linux
        run: cargo build --release --target x86_64-unknown-linux-musl
      
      - name: Build macOS
        run: cargo build --release --target aarch64-apple-darwin
      
      - name: Build Windows
        run: cargo build --release --target x86_64-pc-windows-msvc
      
      - name: Create release
        uses: softprops/action-gh-release@v2
        with:
          files: |
            target/*/release/polygone
```

---

## Conclusion

Ce plan transforme Polygone d'un projet technique en un **produit réel et utilisable**. Les priorités sont :

1. **Cross-platform d'abord** : Sans ça, pas de produit.
2. **TUI comme cœur** : L'interface qui fait la différence.
3. **Power lending comme business model** : Les tokens POLY donnent une raison d'utiliser Polygone.
4. **Recherche privée comme killer feature** : Ce qui rend Polygone unique.
5. **Brain comme révolution** : L'IA qui reste chez toi.

**L'erreur à ne pas faire** : Tout faire en même temps. Suivre les phases. Un module à la fois. Constance > vitesse.

---

*Ce document est vivant. Mettez-le à jour à chaque avancée.*
*Fait par Hermes, le 2026-06-06, à partir de l'analyse du code et de la vision de Lévy.*
