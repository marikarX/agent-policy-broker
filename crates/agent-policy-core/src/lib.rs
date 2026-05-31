//! Core data models for Agent Policy Broker.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Policy {
    pub id: String,
    pub version: PolicyVersion,
    pub status: PolicyStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<u32>,
    pub applies_when: AppliesWhen,
    pub instructions: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_checks: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocked_actions: Vec<BlockedAction>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retrieval: Option<PolicyRetrieval>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<BTreeMap<String, Value>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PolicyVersion {
    Integer(u64),
    Text(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyStatus {
    Draft,
    Active,
    Deprecated,
    Disabled,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct AppliesWhen {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub repos: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub paths: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub languages: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub frameworks: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub package_managers: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub task_types: Vec<TaskType>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub risk_flags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskIntent {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repo: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<TaskDetails>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detected: Option<DetectedContext>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub risk_flags: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub expected_commands: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub expected_check_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_budget: Option<OutputBudget>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskDetails {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub task_type: Option<TaskType>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct DetectedContext {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub languages: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub frameworks: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package_manager: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TaskType(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct OutputBudget {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_instructions: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_required_checks: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_blocked_actions: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_examples: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_explanations: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstructionCandidate {
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<SourceRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstructionBundle {
    pub status: String,
    pub bundle_id: String,
    pub policy_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    pub context_budget: ContextBudgetReport,
    pub instructions: Vec<BundleInstruction>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_checks: Vec<RequiredCheck>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocked_actions: Vec<BlockedAction>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<SourceRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub explanations: Vec<BundleExplanation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BundleInstruction {
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<SourceRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequiredCheck {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<SourceRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BlockedAction(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SourceRef(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ContextBudgetReport {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimated_tokens: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimate_method: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_policies_considered: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_policies_omitted: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct PolicyRetrieval {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub semantic_terms: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub related_docs: Vec<String>,
    #[serde(flatten, default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BundleExplanation {
    pub instruction: String,
    pub source: SourceRef,
    pub reason: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_POLICY_YAML: &str = include_str!("../../../examples/policies/typescript.yaml");
    const SAMPLE_PAYMENTS_POLICY_YAML: &str =
        include_str!("../../../examples/policies/payments.yaml");
    const SAMPLE_INTENT_JSON: &str = r#"{
  "repo": "billing-api",
  "branch": "feature/refund-retries",
  "task": {
    "summary": "Fix refund retry handling",
    "type": "fix_bug"
  },
  "files": [
    "src/payments/refunds.ts",
    "tests/payments/refunds.test.ts"
  ],
  "detected": {
    "languages": ["typescript"],
    "frameworks": ["jest"],
    "package_manager": "npm"
  },
  "risk_flags": ["payments"],
  "expected_commands": ["npm test"],
  "expected_check_ids": ["typescript.unit_tests"],
  "output_budget": {
    "max_tokens": 900,
    "max_instructions": 8,
    "max_required_checks": 4,
    "max_blocked_actions": 4,
    "include_explanations": "compact"
  }
}"#;

    const SAMPLE_BUNDLE_JSON: &str = r#"{
  "status": "ok",
  "bundle_id": "apb_2026-05-31_001",
  "policy_version": "2026-05-31.1",
  "summary": "Instructions for a TypeScript payment bug fix.",
  "context_budget": {
    "max_tokens": 900,
    "estimated_tokens": 420,
    "estimate_method": "approx_words",
    "candidate_policies_considered": 14,
    "candidate_policies_omitted": 9,
    "reason": "Lower priority or duplicate non-mandatory guidance excluded by context budget."
  },
  "instructions": [
    {
      "text": "Preserve refund idempotency semantics.",
      "priority": "critical",
      "source": "domain.payments.refunds@7"
    },
    {
      "text": "Add tests for provider retry and repeated refund request handling.",
      "priority": "high",
      "source": "domain.payments.testing@2"
    }
  ],
  "required_checks": [
    {
      "id": "typescript.lint",
      "source": "lang.typescript.base@1",
      "resolved": false
    },
    {
      "id": "payments.unit_tests",
      "source": "domain.payments.testing@2",
      "resolved": false
    }
  ],
  "blocked_actions": [
    "Do not edit production payment credentials."
  ],
  "sources": [
    "domain.payments.refunds@7",
    "domain.payments.testing@2",
    "lang.typescript.base@1"
  ],
  "explanations": [
    {
      "instruction": "Preserve refund idempotency semantics.",
      "source": "domain.payments.refunds@7",
      "reason": "Matched risk flag `payments`, path `src/payments/**`, and semantic terms related to refund retries."
    }
  ]
}"#;

    #[test]
    fn version_is_non_empty() {
        assert!(!version().is_empty());
    }

    #[test]
    fn policy_yaml_deserializes() {
        let policy: Policy = serde_yaml::from_str(SAMPLE_POLICY_YAML).unwrap();

        assert_eq!(policy.id, "lang.typescript.base");
        assert_eq!(policy.version, PolicyVersion::Integer(1));
        assert_eq!(policy.status, PolicyStatus::Active);
        assert_eq!(
            policy.applies_when.task_types,
            vec![
                TaskType("fix_bug".into()),
                TaskType("add_feature".into()),
                TaskType("refactor".into()),
                TaskType("test".into()),
            ]
        );
        assert_eq!(policy.required_checks.len(), 2);
        assert_eq!(
            policy.blocked_actions,
            vec![BlockedAction("Do not edit generated files directly.".into())]
        );
    }

    #[test]
    fn policy_yaml_serializes() {
        let policy: Policy = serde_yaml::from_str(SAMPLE_POLICY_YAML).unwrap();
        let rendered = serde_yaml::to_string(&policy).unwrap();

        assert!(rendered.contains("id: lang.typescript.base"));
        assert!(rendered.contains("status: active"));
        assert!(rendered.contains("- typescript.typecheck"));
        assert!(rendered.contains("- Do not edit generated files directly."));
    }

    #[test]
    fn payments_policy_yaml_deserializes() {
        let policy: Policy = serde_yaml::from_str(SAMPLE_PAYMENTS_POLICY_YAML).unwrap();

        assert_eq!(policy.id, "domain.payments.refunds");
        assert_eq!(policy.status, PolicyStatus::Active);
        assert_eq!(
            policy.applies_when.paths,
            vec![
                "src/payments/**".to_string(),
                "src/refunds/**".to_string(),
                "tests/payments/**".to_string(),
            ]
        );
        assert_eq!(
            policy.applies_when.task_types,
            vec![
                TaskType("fix_bug".into()),
                TaskType("add_feature".into()),
                TaskType("test".into()),
            ]
        );
        assert_eq!(policy.blocked_actions.len(), 2);
    }

    #[test]
    fn task_intent_json_deserializes() {
        let intent: TaskIntent = serde_json::from_str(SAMPLE_INTENT_JSON).unwrap();

        assert_eq!(intent.repo.as_deref(), Some("billing-api"));
        assert_eq!(intent.branch.as_deref(), Some("feature/refund-retries"));
        assert_eq!(
            intent.task.as_ref().and_then(|task| task.task_type.as_ref()),
            Some(&TaskType("fix_bug".into()))
        );
        assert_eq!(intent.files.len(), 2);
        assert_eq!(
            intent
                .detected
                .as_ref()
                .and_then(|detected| detected.package_manager.as_deref()),
            Some("npm")
        );
        assert_eq!(
            intent.output_budget.as_ref().and_then(|budget| budget.max_tokens),
            Some(900)
        );
    }

    #[test]
    fn bundle_json_deserializes() {
        let bundle: InstructionBundle = serde_json::from_str(SAMPLE_BUNDLE_JSON).unwrap();

        assert_eq!(bundle.status, "ok");
        assert_eq!(bundle.instructions.len(), 2);
        assert_eq!(bundle.required_checks.len(), 2);
        assert_eq!(
            bundle.sources[0],
            SourceRef("domain.payments.refunds@7".into())
        );
        assert_eq!(bundle.context_budget.max_tokens, Some(900));
    }

    #[test]
    fn bundle_json_serializes() {
        let bundle = InstructionBundle {
            status: "ok".into(),
            bundle_id: "apb_2026-05-31_001".into(),
            policy_version: "2026-05-31.1".into(),
            summary: Some("Instructions for a TypeScript payment bug fix.".into()),
            context_budget: ContextBudgetReport {
                max_tokens: Some(900),
                estimated_tokens: Some(420),
                estimate_method: Some("approx_words".into()),
                candidate_policies_considered: Some(14),
                candidate_policies_omitted: Some(9),
                reason: Some(
                    "Lower priority or duplicate non-mandatory guidance excluded by context budget."
                        .into(),
                ),
            },
            instructions: vec![
                BundleInstruction {
                    text: "Preserve refund idempotency semantics.".into(),
                    priority: Some("critical".into()),
                    source: Some(SourceRef("domain.payments.refunds@7".into())),
                },
                BundleInstruction {
                    text: "Add tests for provider retry and repeated refund request handling.".into(),
                    priority: Some("high".into()),
                    source: Some(SourceRef("domain.payments.testing@2".into())),
                },
            ],
            required_checks: vec![
                RequiredCheck {
                    id: "typescript.lint".into(),
                    source: Some(SourceRef("lang.typescript.base@1".into())),
                    resolved: Some(false),
                },
                RequiredCheck {
                    id: "payments.unit_tests".into(),
                    source: Some(SourceRef("domain.payments.testing@2".into())),
                    resolved: Some(false),
                },
            ],
            blocked_actions: vec![BlockedAction(
                "Do not edit production payment credentials.".into(),
            )],
            sources: vec![
                SourceRef("domain.payments.refunds@7".into()),
                SourceRef("domain.payments.testing@2".into()),
                SourceRef("lang.typescript.base@1".into()),
            ],
            explanations: vec![BundleExplanation {
                instruction: "Preserve refund idempotency semantics.".into(),
                source: SourceRef("domain.payments.refunds@7".into()),
                reason: "Matched risk flag `payments`, path `src/payments/**`, and semantic terms related to refund retries.".into(),
            }],
        };

        let value = serde_json::to_value(&bundle).unwrap();

        assert_eq!(value["status"], "ok");
        assert_eq!(value["bundle_id"], "apb_2026-05-31_001");
        assert_eq!(value["instructions"][0]["priority"], "critical");
        assert_eq!(value["required_checks"][0]["resolved"], false);
        assert_eq!(
            value["blocked_actions"][0],
            "Do not edit production payment credentials."
        );
    }
}
