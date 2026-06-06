//! `polygone-compute` — Location de RAM/CPU distante avec POLY.
//!
//! Chaque nœud peut offrir ou louer de la puissance de calcul.
//! Le daemon invisible (dormant quand le PC est utilisé, actif quand
//! inactif) transforme chaque machine en serveur de calcul décentralisé.
//! Paiement en POLY, automatique, zéro configuration.

#![forbid(unsafe_code)]
#![allow(missing_docs)]

use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Ressource qu'un nœud peut louer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceType {
    /// RAM en Mo
    RamMB,
    /// CPU en unités (1 unité = 1 thread à 1GHz/eq)
    CpuUnits,
    /// Stockage temporaire en Go
    ScratchGB,
    /// GPU (nombre de coeurs partagés)
    GpuCores,
}

/// Offre de location d'un nôud.
#[derive(Clone, Debug)]
pub struct ResourceOffer {
    pub node_id: String,
    pub resource: ResourceType,
    pub amount: u64,       // quantité offerte
    pub price_per_hour: f32, // en POLY
    pub since_ms: u64,
    pub ttl: Duration,
}

impl ResourceOffer {
    pub fn is_expired(&self, now_ms: u64) -> bool {
        now_ms.saturating_sub(self.since_ms) > self.ttl.as_millis() as u64
    }
    pub fn cost_for(&self, hours: f32) -> f32 {
        self.price_per_hour * hours
    }
}

/// Contrat de location actif entre deux nœuds.
#[derive(Clone, Debug)]
pub struct RentalContract {
    pub id: String,
    pub lessor: String,     // propriétaire de la ressource
    pub lessee: String,     // locataire
    pub resource: ResourceType,
    pub amount: u64,
    pub started_ms: u64,
    pub duration_ms: u64,
    pub total_poly: f32,
    pub status: RentalStatus,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RentalStatus {
    Active,
    Completed,
    Defaulted,
}

/// Statistiques de la bourse de calcul.
#[derive(Clone, Debug, Default)]
pub struct ComputeMarketStats {
    pub offers: usize,
    pub active_contracts: usize,
    pub total_poly_traded: f32,
    pub total_hours_rented: f32,
}

/// Marché décentralisé de location de puissance.
pub struct ComputeMarket {
    offers: Vec<ResourceOffer>,
    contracts: Vec<RentalContract>,
    stats: ComputeMarketStats,
}

impl ComputeMarket {
    pub fn new() -> Self {
        Self { offers: Vec::new(), contracts: Vec::new(), stats: ComputeMarketStats::default() }
    }

    /// Un nœud propose une ressource à la location.
    pub fn list_offer(&mut self, node_id: String, resource: ResourceType, amount: u64, price_per_hour: f32) {
        let offer = ResourceOffer {
            node_id,
            resource,
            amount,
            price_per_hour,
            since_ms: epoch_ms(),
            ttl: Duration::from_secs(3600),
        };
        self.offers.push(offer);
        self.stats.offers += 1;
    }

    /// Un nœud loue une ressource. Retourne le contrat ou None.
    pub fn rent(&mut self, lessee: String, resource: ResourceType, amount: u64, max_hours: f32) -> Option<RentalContract> {
        let now = epoch_ms();
        // Sweep expired first
        self.offers.retain(|o| !o.is_expired(now));

        // Find best offer
        let idx = self.offers.iter().enumerate()
            .filter(|(_, o)| o.resource == resource && o.amount >= amount)
            .min_by(|(_, a), (_, b)| a.price_per_hour.partial_cmp(&b.price_per_hour).unwrap())?.0;

        let offer = self.offers.remove(idx);
        let hours = max_hours.min(24.0); // max 24h rental
        let total_poly = offer.cost_for(hours);
        let duration_ms = (hours * 3600.0) as u64 * 1000;

        let contract = RentalContract {
            id: format!("contract:{:x}", rand::random::<u64>()),
            lessor: offer.node_id.clone(),
            lessee,
            resource,
            amount,
            started_ms: now,
            duration_ms,
            total_poly,
            status: RentalStatus::Active,
        };
        self.stats.active_contracts += 1;
        self.stats.total_poly_traded += total_poly;
        self.stats.total_hours_rented += hours;
        self.contracts.push(contract.clone());
        Some(contract)
    }

    /// Marquer un contrat comme terminé.
    pub fn complete(&mut self, contract_id: &str) -> bool {
        if let Some(c) = self.contracts.iter_mut().find(|c| c.id == contract_id) {
            c.status = RentalStatus::Completed;
            self.stats.active_contracts = self.stats.active_contracts.saturating_sub(1);
            true
        } else { false }
    }

    pub fn stats(&self) -> &ComputeMarketStats { &self.stats }
    pub fn offers(&self) -> &[ResourceOffer] { &self.offers }
    pub fn contracts(&self) -> &[RentalContract] { &self.contracts }
}

impl Default for ComputeMarket {
    fn default() -> Self { Self::new() }
}

fn epoch_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_and_rent() {
        let mut m = ComputeMarket::new();
        m.list_offer("node-a".into(), ResourceType::RamMB, 1024, 0.5);
        m.list_offer("node-b".into(), ResourceType::RamMB, 2048, 0.3);
        let c = m.rent("node-c".into(), ResourceType::RamMB, 1024, 2.0).expect("rent");
        assert_eq!(c.lessor, "node-b"); // cheapest
        assert!(c.total_poly > 0.0);
        assert_eq!(m.stats().active_contracts, 1);
    }

    #[test]
    fn no_offer_returns_none() {
        let mut m = ComputeMarket::new();
        assert!(m.rent("x".into(), ResourceType::GpuCores, 1, 1.0).is_none());
    }

    #[test]
    fn complete_contract() {
        let mut m = ComputeMarket::new();
        m.list_offer("n".into(), ResourceType::CpuUnits, 4, 1.0);
        let c = m.rent("o".into(), ResourceType::CpuUnits, 4, 1.0).unwrap();
        assert!(m.complete(&c.id));
        assert_eq!(m.stats().active_contracts, 0);
    }

    #[test]
    fn expired_offer_not_rented() {
        let mut m = ComputeMarket::new();
        m.list_offer("n".into(), ResourceType::RamMB, 1024, 1.0);
        // Manually expire
        m.offers[0].since_ms = 0;
        m.offers[0].ttl = Duration::from_secs(1);
        assert!(m.rent("o".into(), ResourceType::RamMB, 1024, 1.0).is_none());
    }
}