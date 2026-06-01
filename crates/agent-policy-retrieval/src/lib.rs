//! Retrieval abstractions that are deliberately separate from policy authority.
//!
//! This crate defines contracts for optional semantic retrieval backends. It
//! does not provide an embedding implementation, call a remote service, or
//! index source code. Callers must explicitly provide a local implementation and
//! keep retrieved candidates subordinate to authoritative policy matching.

use std::error::Error;

/// Text accepted by an [`EmbeddingProvider`].
///
/// The input is intentionally plain text plus a caller-controlled purpose label.
/// Implementations should not read files, crawl repositories, or contact remote
/// APIs unless a future operator-controlled integration explicitly chooses to do
/// so outside this crate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddingInput<'a> {
    /// Text to embed.
    pub text: &'a str,
    /// Optional purpose label, such as `policy_query` or `policy_term`.
    pub purpose: Option<&'a str>,
}

impl<'a> EmbeddingInput<'a> {
    /// Create embedding input for a query or candidate text.
    pub fn new(text: &'a str) -> Self {
        Self {
            text,
            purpose: None,
        }
    }

    /// Attach a caller-defined purpose label.
    pub fn with_purpose(mut self, purpose: &'a str) -> Self {
        self.purpose = Some(purpose);
        self
    }
}

/// A dense vector produced by an [`EmbeddingProvider`].
#[derive(Debug, Clone, PartialEq)]
pub struct Embedding {
    dimensions: Vec<f32>,
}

impl Embedding {
    /// Create an embedding from vector dimensions.
    pub fn new(dimensions: Vec<f32>) -> Self {
        Self { dimensions }
    }

    /// Return the raw dimensions for backend-specific search code.
    pub fn dimensions(&self) -> &[f32] {
        &self.dimensions
    }

    /// Return true when the embedding has no dimensions.
    pub fn is_empty(&self) -> bool {
        self.dimensions.is_empty()
    }
}

/// Produces local embeddings for explicit caller-provided text.
///
/// The trait is synchronous and backend-neutral. It is a contract only; this
/// crate intentionally ships no provider that could make a network request.
pub trait EmbeddingProvider {
    /// Provider-specific error type.
    type Error: Error + Send + Sync + 'static;

    /// Embed one explicit input string.
    fn embed(&self, input: &EmbeddingInput<'_>) -> Result<Embedding, Self::Error>;
}

/// A candidate returned by a vector search backend.
///
/// Candidate identifiers should point to policy or retrieval records that the
/// caller can re-check through authoritative policy loading. A vector score is
/// only a recall signal and must not override policy status, priority, or scope.
pub trait VectorCandidate {
    /// Stable identifier of the candidate record.
    fn id(&self) -> &str;

    /// Backend-specific similarity score. Higher values should mean a better
    /// match for implementations that expose scoring.
    fn score(&self) -> f32;
}

/// Searches a local vector index for semantic candidates.
///
/// Implementations should treat the index as a derived cache. Search results are
/// candidate guidance only and must be reconciled with authoritative policy
/// metadata before they affect an instruction bundle.
pub trait VectorIndex {
    /// Candidate type returned by this index.
    type Candidate: VectorCandidate;
    /// Index-specific error type.
    type Error: Error + Send + Sync + 'static;

    /// Search using an already-created embedding.
    fn search(
        &self,
        embedding: &Embedding,
        limit: usize,
    ) -> Result<Vec<Self::Candidate>, Self::Error>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt;

    #[derive(Debug)]
    struct TestError;

    impl fmt::Display for TestError {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("test retrieval error")
        }
    }

    impl Error for TestError {}

    struct LocalProvider;

    impl EmbeddingProvider for LocalProvider {
        type Error = TestError;

        fn embed(&self, input: &EmbeddingInput<'_>) -> Result<Embedding, Self::Error> {
            Ok(Embedding::new(vec![input.text.len() as f32, 1.0]))
        }
    }

    #[derive(Debug, Clone, PartialEq)]
    struct LocalCandidate {
        id: String,
        score: f32,
    }

    impl VectorCandidate for LocalCandidate {
        fn id(&self) -> &str {
            &self.id
        }

        fn score(&self) -> f32 {
            self.score
        }
    }

    struct LocalIndex;

    impl VectorIndex for LocalIndex {
        type Candidate = LocalCandidate;
        type Error = TestError;

        fn search(
            &self,
            embedding: &Embedding,
            limit: usize,
        ) -> Result<Vec<Self::Candidate>, Self::Error> {
            if limit == 0 || embedding.is_empty() {
                return Ok(Vec::new());
            }
            Ok(vec![LocalCandidate {
                id: "policy.local".to_string(),
                score: embedding.dimensions()[0],
            }])
        }
    }

    #[test]
    fn local_provider_and_index_can_be_composed_without_backend() {
        let provider = LocalProvider;
        let embedding = provider
            .embed(&EmbeddingInput::new("refund retries").with_purpose("policy_query"))
            .expect("local embedding");
        let candidates = LocalIndex.search(&embedding, 4).expect("local search");

        assert_eq!(embedding.dimensions(), &[14.0, 1.0]);
        assert_eq!(candidates[0].id(), "policy.local");
        assert_eq!(candidates[0].score(), 14.0);
    }

    #[test]
    fn empty_or_zero_limit_search_returns_no_candidates() {
        assert!(LocalIndex
            .search(&Embedding::new(Vec::new()), 4)
            .expect("empty search")
            .is_empty());
        assert!(LocalIndex
            .search(&Embedding::new(vec![1.0]), 0)
            .expect("zero limit search")
            .is_empty());
    }
}
