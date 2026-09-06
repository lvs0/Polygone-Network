//! Model registry and management

use crate::types::{ModelInfo, ModelSource, ModelRequirements, DeviceType};
use std::collections::HashMap;

/// Registry of available models
#[derive(Debug, Default)]
pub struct ModelRegistry {
    models: HashMap<String, ModelInfo>,
}

impl ModelRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Update registry with new models
    pub fn update(&mut self, models: Vec<ModelInfo>) {
        self.models.clear();
        for model in models {
            self.models.insert(model.name.clone(), model);
        }
    }

    /// Add a single model
    pub fn add(&mut self, model: ModelInfo) {
        self.models.insert(model.name.clone(), model);
    }

    /// Remove a model
    pub fn remove(&mut self, name: &str) -> Option<ModelInfo> {
        self.models.remove(name)
    }

    /// Check if model exists
    pub fn has_model(&self, name: &str) -> bool {
        self.models.contains_key(name)
    }

    /// Get model info
    pub fn get_model(&self, name: &str) -> Option<&ModelInfo> {
        self.models.get(name)
    }

    /// List all models
    pub fn list_models(&self) -> Vec<ModelInfo> {
        self.models.values().cloned().collect()
    }

    /// Get models by source
    pub fn by_source(&self, source: ModelSource) -> Vec<ModelInfo> {
        self.models
            .values()
            .filter(|m| m.source == source)
            .cloned()
            .collect()
    }

    /// Get models that can run on a device
    pub fn for_device(&self, device: DeviceType) -> Vec<ModelInfo> {
        self.models
            .values()
            .filter(|m| m.supports_device(device))
            .cloned()
            .collect()
    }

    /// Get total estimated requirements for all models
    pub fn total_requirements(&self, device: DeviceType) -> ModelRequirements {
        let mut total = ModelRequirements {
            cpu_cores: 0,
            ram_gb: 0.0,
            gpu_vram_gb: None,
            disk_gb: 0.0,
            est_tokens_per_sec: None,
        };

        for model in self.models.values() {
            let req = model.estimate_requirements(device);
            total.cpu_cores = total.cpu_cores.max(req.cpu_cores);
            total.ram_gb += req.ram_gb;
            total.disk_gb += req.disk_gb;
            if let Some(vram) = req.gpu_vram_gb {
                total.gpu_vram_gb = Some(total.gpu_vram_gb.unwrap_or(0.0) + vram);
            }
        }

        total
    }
}