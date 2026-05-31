//! Configuration loading and validation scaffolding.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct BrokerConfig {
    pub enabled: bool,
}

#[cfg(test)]
mod tests {
    use super::BrokerConfig;

    #[test]
    fn default_is_disabled() {
        let cfg = BrokerConfig::default();
        assert!(!cfg.enabled);
    }
}
