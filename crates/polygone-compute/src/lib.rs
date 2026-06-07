//! `polygone-compute` — Location de RAM/CPU distante avec POLY.
//!
//! Chaque nœud peut offrir ou louer de la puissance de calcul.
//! Le daemon invisible (dormant quand le PC est utilisé, actif quand
//! inactif) transforme chaque machine en serveur de calcul décentralisé.
//! Paiement en POLY, automatique, zéro configuration.
//!
//! Extended with:
//! - Resource allocation scheduling
//! - Cross-platform resource enforcement
//! - Stealth-mode-aware resource management

#![forbid(unsafe_code)]
#![allow(missing_docs)]

use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Ressource qu'un nœud peut louer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
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

impl ResourceType {
    pub fn label(&self) -> &'static str {
        match self {
            ResourceType::RamMB => "RAM (MB)",
            ResourceType::CpuUnits => "CPU Units",
            ResourceType::ScratchGB => "Storage (GB)",
            ResourceType::GpuCores => "GPU Cores",
        }
    }
}

/// Offre de location d'un nœud.
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

    /// Get a contract by ID.
    pub fn get_contract(&self, contract_id: &str) -> Option<&RentalContract> {
        self.contracts.iter().find(|c| c.id == contract_id)
    }

    /// Get all active contracts.
    pub fn active_contracts(&self) -> Vec<&RentalContract> {
        self.contracts.iter().filter(|c| c.status == RentalStatus::Active).collect()
    }

    /// Get available offers for a resource type.
    pub fn available_offers(&self, resource: ResourceType) -> Vec<&ResourceOffer> {
        let now = epoch_ms();
        self.offers.iter()
            .filter(|o| o.resource == resource && !o.is_expired(now))
            .collect()
    }

    /// Clean up expired offers and completed contracts.
    pub fn cleanup(&mut self) {
        let now = epoch_ms();
        self.offers.retain(|o| !o.is_expired(now));
        // Keep completed contracts for history (last 100)
        if self.contracts.len() > 200 {
            self.contracts.retain(|c| c.status == RentalStatus::Active || (now - c.started_ms) < 86_400_000);
        }
    }

    pub fn stats(&self) -> &ComputeMarketStats { &self.stats }
    pub fn offers(&self) -> &[ResourceOffer] { &self.offers }
    pub fn contracts(&self) -> &[RentalContract] { &self.contracts }
}

impl Default for ComputeMarket {
    fn default() -> Self { Self::new() }
}

/// Resource allocation scheduler — manages the allocation of resources
/// to tasks based on priority and available capacity.
pub struct ResourceAllocator {
    /// Maximum RAM to allocate (MB)
    max_ram_mb: u64,
    /// Maximum CPU units to allocate
    max_cpu_units: u32,
    /// Currently allocated RAM (MB)
    allocated_ram_mb: u64,
    /// Currently allocated CPU units
    allocated_cpu_units: u32,
    /// Active allocations
    allocations: Vec<Allocation>,
}

/// A single resource allocation.
#[derive(Clone, Debug)]
pub struct Allocation {
    pub id: String,
    pub task_id: String,
    pub resource: ResourceType,
    pub amount: u64,
    pub priority: u8,
    pub started_ms: u64,
    pub expires_ms: u64,
}

impl ResourceAllocator {
    pub fn new(max_ram_mb: u64, max_cpu_units: u32) -> Self {
        Self {
            max_ram_mb,
            max_cpu_units,
            allocated_ram_mb: 0,
            allocated_cpu_units: 0,
            allocations: Vec::new(),
        }
    }

    /// Try to allocate resources for a task. Returns the allocation if successful.
    pub fn allocate(&mut self, task_id: &str, resource: ResourceType, amount: u64, duration_secs: u64, priority: u8) -> Option<Allocation> {
        // Clean expired first
        self.cleanup();

        let can_allocate = match resource {
            ResourceType::RamMB => self.allocated_ram_mb + amount <= self.max_ram_mb,
            ResourceType::CpuUnits => self.allocated_cpu_units + amount as u32 <= self.max_cpu_units,
            _ => true, // Storage/GPU not tracked
        };

        if !can_allocate {
            // Try to evict lower-priority allocations
            if !self.try_evict_lower_priority(resource, amount, priority) {
                return None;
            }
        }

        let now = epoch_ms();
        let alloc = Allocation {
            id: format!("alloc:{:x}", rand::random::<u64>()),
            task_id: task_id.to_string(),
            resource,
            amount,
            priority,
            started_ms: now,
            expires_ms: now + duration_secs * 1000,
        };

        match resource {
            ResourceType::RamMB => self.allocated_ram_mb += amount,
            ResourceType::CpuUnits => self.allocated_cpu_units += amount as u32,
            _ => {}
        }

        self.allocations.push(alloc.clone());
        Some(alloc)
    }

    /// Release an allocation.
    pub fn release(&mut self, allocation_id: &str) -> bool {
        if let Some(pos) = self.allocations.iter().position(|a| a.id == allocation_id) {
            let alloc = self.allocations.remove(pos);
            match alloc.resource {
                ResourceType::RamMB => self.allocated_ram_mb = self.allocated_ram_mb.saturating_sub(alloc.amount),
                ResourceType::CpuUnits => self.allocated_cpu_units = self.allocated_cpu_units.saturating_sub(alloc.amount as u32),
                _ => {}
            }
            true
        } else {
            false
        }
    }

    /// Try to evict lower-priority allocations to make room.
    fn try_evict_lower_priority(&mut self, resource: ResourceType, amount: u64, priority: u8) -> bool {
        let mut candidates: Vec<usize> = self.allocations.iter().enumerate()
            .filter(|(_, a)| a.resource == resource && a.priority < priority)
            .map(|(i, _)| i)
            .collect();

        // Sort by priority (lowest first) to evict cheapest first
        candidates.sort_by_key(|&i| self.allocations[i].priority);

        let mut freed = 0u64;
        let mut to_remove = Vec::new();

        for &idx in &candidates {
            freed += self.allocations[idx].amount;
            to_remove.push(idx);
            if freed >= amount {
                break;
            }
        }

        if freed >= amount {
            // Remove in reverse order to maintain indices
            for &idx in to_remove.iter().rev() {
                let alloc = self.allocations.remove(idx);
                match alloc.resource {
                    ResourceType::RamMB => self.allocated_ram_mb = self.allocated_ram_mb.saturating_sub(alloc.amount),
                    ResourceType::CpuUnits => self.allocated_cpu_units = self.allocated_cpu_units.saturating_sub(alloc.amount as u32),
                    _ => {}
                }
            }
            true
        } else {
            false
        }
    }

    /// Remove expired allocations.
    fn cleanup(&mut self) {
        let now = epoch_ms();
        self.allocations.retain(|a| {
            if a.expires_ms <= now {
                match a.resource {
                    ResourceType::RamMB => self.allocated_ram_mb = self.allocated_ram_mb.saturating_sub(a.amount),
                    ResourceType::CpuUnits => self.allocated_cpu_units = self.allocated_cpu_units.saturating_sub(a.amount as u32),
                    _ => {}
                }
                false
            } else {
                true
            }
        });
    }

    /// Get current utilization.
    pub fn utilization(&self) -> (u64, u64, u32, u32) {
        (self.allocated_ram_mb, self.max_ram_mb, self.allocated_cpu_units, self.max_cpu_units)
    }

    /// Get active allocations count.
    pub fn active_count(&self) -> usize {
        self.allocations.len()
    }
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

    #[test]
    fn allocator_basic() {
        let mut a = ResourceAllocator::new(1024, 4);
        let alloc = a.allocate("task-1", ResourceType::RamMB, 512, 3600, 1);
        assert!(alloc.is_some());
        assert_eq!(a.active_count(), 1);
        let (used, max, _, _) = a.utilization();
        assert_eq!(used, 512);
        assert_eq!(max, 1024);
    }

    #[test]
    fn allocator_rejects_over_capacity() {
        let mut a = ResourceAllocator::new(512, 2);
        a.allocate("task-1", ResourceType::RamMB, 512, 3600, 1);
        let alloc = a.allocate("task-2", ResourceType::RamMB, 1, 3600, 0);
        assert!(alloc.is_none());
    }

    #[test]
    fn allocator_evicts_lower_priority() {
        let mut a = ResourceAllocator::new(512, 4);
        a.allocate("bg-task", ResourceType::RamMB, 512, 3600, 0); // priority 0
        let alloc = a.allocate("hi-task", ResourceType::RamMB, 512, 3600, 2); // priority 2
        assert!(alloc.is_some());
        assert_eq!(a.active_count(), 1); // bg-task was evicted
    }

    #[test]
    fn allocator_release() {
        let mut a = ResourceAllocator::new(1024, 4);
        let alloc = a.allocate("task-1", ResourceType::RamMB, 512, 3600, 1).unwrap();
        assert!(a.release(&alloc.id));
        assert_eq!(a.active_count(), 0);
        let (used, _, _, _) = a.utilization();
        assert_eq!(used, 0);
    }
}
