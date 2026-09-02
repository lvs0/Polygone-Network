//! Polygone Brain — local orchestration / compute brain.
//!
//! MVP placeholder: orchestrate msg, drive, hide, petals, compute.

use crate::Result;

#[derive(Clone, Debug)]
pub struct Brain {
    pub node_id: String,
    pub services: Vec<String>,
}

impl Brain {
    pub fn new(node_id: &str) -> Self {
        Self {
            node_id: node_id.to_string(),
            services: vec!["msg".into(), "drive".into()],
        }
    }

    pub fn status(&self) -> Vec<(&'static str, &'static str)> {
        vec![("msg", "live"), ("drive", "stub"), ("petals", "stub"), ("compute", "live")]
    }
}
