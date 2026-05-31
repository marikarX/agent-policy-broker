//! Instruction source discovery scaffolding.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiscoveryResult {
    pub roots: Vec<String>,
}

impl DiscoveryResult {
    pub fn empty() -> Self {
        Self { roots: Vec::new() }
    }
}

#[cfg(test)]
mod tests {
    use super::DiscoveryResult;

    #[test]
    fn empty_result_has_no_roots() {
        assert!(DiscoveryResult::empty().roots.is_empty());
    }
}
