//! Bus d'événements borné de la gateway (F1-EVT-01, domaine Activité).
//!
//! Choix d'architecture assumé : un curseur de polling plutôt qu'un flux
//! SSE/WebSocket. La couche HTTP de la gateway est volontairement minimale
//! (une requête, une réponse, fermeture) ; un flux long-lived la complexifierait
//! pour un gain nul sur un tableau de bord local. Le client interroge
//! `GET /api/v1/events?since=<id>` à son rythme :
//!
//! - **ordre** : identifiants monotones croissants, attribués à l'émission ;
//! - **perte** : anneau borné (`EVENT_CAPACITY`). Quand il déborde, les plus
//!   anciens sont évincés et `dropped_total` compte les évincions. Un client
//!   dont le curseur est plus vieux que la fenêtre retenue reçoit `gap: true`
//!   — la perte est SIGNALÉE, jamais masquée par un faux « tout va bien » ;
//! - **backpressure** : le serveur n'attend jamais un client lent. C'est le
//!   client qui choisit sa cadence ; s'il décroche, il voit le gap et se
//!   resynchronise sur `latest_id`.
//!
//! Seuls des événements RÉELS sont émis : transitions de tâches, appariements,
//! battements de cœur, RTT mesurés, et instantanés de ressources quand le
//! daemon change de `seq`. Aucune simulation, aucun événement décoratif.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use serde::Serialize;
use serde_json::Value;

/// Nombre maximal d'événements retenus. Au-delà, les plus anciens partent.
pub const EVENT_CAPACITY: usize = 512;
/// Taille de page par défaut d'une requête `events`.
pub const EVENTS_DEFAULT_LIMIT: usize = 100;
/// Plafond dur d'une page : un client ne peut pas exiger plus.
pub const EVENTS_MAX_LIMIT: usize = 500;

/// Un événement publié. `at` est l'heure Unix en secondes (horloge locale,
/// non synchronisée : c'est une heure d'observation, pas une preuve).
#[derive(Debug, Clone, Serialize)]
pub struct Event {
    pub id: u64,
    pub at: u64,
    pub kind: String,
    pub data: Value,
}

/// Page de réponse du bus : les événements demandés, le curseur à rejouer,
/// et l'état honnête de la fenêtre (gap, évincions cumulées, capacité).
#[derive(Debug, Clone, Serialize)]
pub struct EventsPage {
    pub events: Vec<Event>,
    /// Curseur à renvoyer au prochain appel : dernier id servi (ou dernier
    /// id connu si la page est vide).
    pub next_since: u64,
    /// `true` si des événements postérieurs à `since` ont été évincés avant
    /// d'être servis : le client a manqué une partie du flux.
    pub gap: bool,
    /// Évincions cumulées depuis le démarrage (compteur monotone).
    pub dropped_total: u64,
    /// Capacité de l'anneau, publiée pour que le client sache dimensionner
    /// sa cadence de polling.
    pub capacity: usize,
    /// Dernier identifiant attribué (0 si rien n'a encore été émis).
    pub latest_id: u64,
}

#[derive(Debug, Default)]
struct BusInner {
    next_id: u64,
    ring: VecDeque<Event>,
    dropped_total: u64,
}

/// Handle partageable du bus. Clone = même anneau (Arc).
#[derive(Debug, Clone, Default)]
pub struct EventBus {
    inner: Arc<Mutex<BusInner>>,
}

impl EventBus {
    pub fn new() -> Self {
        Self::default()
    }

    /// Émet un événement réel. Rend l'identifiant attribué.
    ///
    /// Section critique courte, sans I/O ni await : le bus se verrouille le
    /// temps d'un push, jamais pendant une réponse réseau.
    pub fn emit(&self, kind: &str, data: Value) -> u64 {
        let mut bus = self.lock();
        bus.next_id += 1;
        let id = bus.next_id;
        bus.ring.push_back(Event {
            id,
            at: crate::now_secs(),
            kind: kind.to_string(),
            data,
        });
        if bus.ring.len() > EVENT_CAPACITY {
            bus.ring.pop_front();
            bus.dropped_total += 1;
        }
        id
    }

    /// Page d'événements postérieurs à `since`, filtrée par `kind` si donné.
    ///
    /// `gap` est vrai quand le curseur demande des événements qui n'existent
    /// plus dans l'anneau : `since + 1 < plus_ancien_retenu`. Un curseur à 0
    /// sur un anneau qui a déjà débordé est donc un gap — le client le voit.
    pub fn page(&self, since: u64, limit: usize, kind: Option<&str>) -> EventsPage {
        let bus = self.lock();
        // `next_id` porte le DERNIER id attribué (l'émission l'incrémente
        // puis l'utilise) : c'est lui le dernier événement connu.
        let latest_id = bus.next_id;
        let oldest = bus.ring.front().map(|e| e.id);
        let gap = match oldest {
            // Rien n'a jamais été émis : rien à manquer.
            None => false,
            Some(oldest) => since.saturating_add(1) < oldest,
        };
        let limit = limit.clamp(1, EVENTS_MAX_LIMIT);
        let events: Vec<Event> = bus
            .ring
            .iter()
            .filter(|e| e.id > since)
            .filter(|e| kind.is_none_or(|k| e.kind == k))
            .take(limit)
            .cloned()
            .collect();
        let next_since = events.last().map(|e| e.id).unwrap_or(latest_id);
        EventsPage {
            events,
            next_since,
            gap,
            dropped_total: bus.dropped_total,
            capacity: EVENT_CAPACITY,
            latest_id,
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, BusInner> {
        // Même politique que le daemon : un panic ne doit pas rendre le bus
        // inutilisable ; l'anneau reste cohérent (mutations courtes).
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn ids_strictement_croissants_et_ordre_stable() {
        let bus = EventBus::new();
        let mut ids = Vec::new();
        for i in 0..10 {
            ids.push(bus.emit("test", json!({"i": i})));
        }
        assert!(ids.windows(2).all(|w| w[1] == w[0] + 1));
        let page = bus.page(0, 100, None);
        assert_eq!(page.events.len(), 10);
        assert!(page.events.windows(2).all(|w| w[0].id < w[1].id));
        assert!(!page.gap);
        assert_eq!(page.latest_id, 10);
        assert_eq!(page.next_since, 10);
    }

    #[test]
    fn rejeu_depuis_le_curseur_ne_rend_que_du_neuf() {
        let bus = EventBus::new();
        bus.emit("a", json!({}));
        bus.emit("b", json!({}));
        let page = bus.page(1, 100, None);
        assert_eq!(page.events.len(), 1);
        assert_eq!(page.events[0].kind, "b");
        // Curseur à jour : page vide, next_since reste au dernier id.
        let page = bus.page(2, 100, None);
        assert!(page.events.is_empty());
        assert_eq!(page.next_since, 2);
        assert!(!page.gap);
    }

    #[test]
    fn debordement_evince_et_signale_le_gap() {
        let bus = EventBus::new();
        let total = EVENT_CAPACITY + 88;
        for i in 0..total {
            bus.emit("overflow", json!({"i": i}));
        }
        let page = bus.page(0, EVENTS_MAX_LIMIT, None);
        // Une page est plafonnée à EVENTS_MAX_LIMIT, même si l'anneau retient
        // plus : le client pagine avec son curseur.
        assert_eq!(page.events.len(), EVENTS_MAX_LIMIT);
        assert_eq!(page.dropped_total, 88);
        assert!(page.gap, "un curseur 0 sur anneau débordé doit voir le gap");
        assert_eq!(page.events[0].id, 89);
        // Un client resynchronisé sur latest_id ne voit plus de gap.
        let page = bus.page(page.latest_id, 100, None);
        assert!(!page.gap);
        assert!(page.events.is_empty());
    }

    #[test]
    fn filtre_par_kind_et_plafond_de_page() {
        let bus = EventBus::new();
        for _ in 0..5 {
            bus.emit("task", json!({}));
            bus.emit("node", json!({}));
        }
        let page = bus.page(0, 100, Some("task"));
        assert_eq!(page.events.len(), 5);
        assert!(page.events.iter().all(|e| e.kind == "task"));
        // Le plafond dur s'applique même si le client demande plus.
        let page = bus.page(0, 10_000, None);
        assert_eq!(page.events.len(), 10); // tout tient, mais clamp testé ailleurs
        let bus2 = EventBus::new();
        for _ in 0..(EVENTS_MAX_LIMIT + 50) {
            bus2.emit("bulk", json!({}));
        }
        let page = bus2.page(0, usize::MAX, None);
        assert_eq!(page.events.len(), EVENTS_MAX_LIMIT);
    }

    #[test]
    fn bus_vide_rend_une_page_vide_sans_gap() {
        let bus = EventBus::new();
        let page = bus.page(0, 100, None);
        assert!(page.events.is_empty());
        assert!(!page.gap);
        assert_eq!(page.latest_id, 0);
        assert_eq!(page.next_since, 0);
        assert_eq!(page.dropped_total, 0);
    }
}
