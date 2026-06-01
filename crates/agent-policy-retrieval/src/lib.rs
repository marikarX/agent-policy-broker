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

#[cfg(feature = "sqlite-vec")]
pub mod local {
    //! Deterministic local vector prototype.
    //!
    //! This module is exposed behind the `sqlite-vec` feature as the current
    //! local vector-backend prototype. It does not yet link sqlite-vec; the
    //! implementation is an in-memory deterministic backend used to validate
    //! feature gating, privacy boundaries, and candidate-only bundle behavior
    //! until sqlite-vec integration is stable enough for this project.

    use std::collections::{BTreeMap, BTreeSet};
    use std::error::Error;
    use std::fmt;

    use crate::{Embedding, EmbeddingInput, EmbeddingProvider, VectorCandidate, VectorIndex};

    const DIMENSIONS: usize = 16;

    /// Error returned by the deterministic local vector prototype.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct LocalVectorError {
        message: String,
    }

    impl LocalVectorError {
        fn new(message: impl Into<String>) -> Self {
            Self {
                message: message.into(),
            }
        }
    }

    impl fmt::Display for LocalVectorError {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str(&self.message)
        }
    }

    impl Error for LocalVectorError {}

    /// Deterministic local embedder with no network or file-system behavior.
    #[derive(Debug, Default, Clone, Copy)]
    pub struct DeterministicEmbeddingProvider;

    impl EmbeddingProvider for DeterministicEmbeddingProvider {
        type Error = LocalVectorError;

        fn embed(&self, input: &EmbeddingInput<'_>) -> Result<Embedding, Self::Error> {
            Ok(Embedding::new(embed_text(input.text)))
        }
    }

    /// Explicit text accepted by the local vector prototype.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct VectorDocument {
        id: String,
        text: String,
    }

    impl VectorDocument {
        /// Create a document from caller-provided text.
        pub fn new(id: impl Into<String>, text: impl Into<String>) -> Self {
            Self {
                id: id.into(),
                text: text.into(),
            }
        }

        /// Stable identifier returned by searches.
        pub fn id(&self) -> &str {
            &self.id
        }
    }

    #[derive(Debug, Clone)]
    struct IndexedVectorDocument {
        id: String,
        embedding: Embedding,
    }

    /// Candidate identifier and score returned by the local vector prototype.
    ///
    /// The raw indexed document text is intentionally not part of this type.
    #[derive(Debug, Clone, PartialEq)]
    pub struct LocalVectorCandidate {
        id: String,
        score: f32,
    }

    impl VectorCandidate for LocalVectorCandidate {
        fn id(&self) -> &str {
            &self.id
        }

        fn score(&self) -> f32 {
            self.score
        }
    }

    /// In-memory local vector index for deterministic tests and prototypes.
    #[derive(Debug, Default, Clone)]
    pub struct InMemoryVectorIndex {
        documents: Vec<IndexedVectorDocument>,
    }

    impl InMemoryVectorIndex {
        /// Build an index from explicit synthetic or policy/retrieval text.
        pub fn from_documents<I>(documents: I) -> Result<Self, LocalVectorError>
        where
            I: IntoIterator<Item = VectorDocument>,
        {
            let provider = DeterministicEmbeddingProvider;
            let mut indexed = Vec::new();
            let mut seen = BTreeSet::new();

            for document in documents {
                if document.id.trim().is_empty() {
                    return Err(LocalVectorError::new("vector document id is empty"));
                }
                if !seen.insert(document.id.clone()) {
                    return Err(LocalVectorError::new(format!(
                        "duplicate vector document id `{}`",
                        document.id
                    )));
                }
                let embedding = provider.embed(&EmbeddingInput::new(&document.text))?;
                indexed.push(IndexedVectorDocument {
                    id: document.id,
                    embedding,
                });
            }

            Ok(Self { documents: indexed })
        }

        /// Number of indexed documents.
        pub fn len(&self) -> usize {
            self.documents.len()
        }

        /// Return true when no documents are indexed.
        pub fn is_empty(&self) -> bool {
            self.documents.is_empty()
        }

        /// Embed and search a plain-text query.
        pub fn search_text(
            &self,
            query: &str,
            limit: usize,
        ) -> Result<Vec<LocalVectorCandidate>, LocalVectorError> {
            let provider = DeterministicEmbeddingProvider;
            let embedding =
                provider.embed(&EmbeddingInput::new(query).with_purpose("policy_query"))?;
            self.search(&embedding, limit)
        }
    }

    impl VectorIndex for InMemoryVectorIndex {
        type Candidate = LocalVectorCandidate;
        type Error = LocalVectorError;

        fn search(
            &self,
            embedding: &Embedding,
            limit: usize,
        ) -> Result<Vec<Self::Candidate>, Self::Error> {
            if limit == 0 || embedding.is_empty() {
                return Ok(Vec::new());
            }

            let mut candidates = self
                .documents
                .iter()
                .filter_map(|document| {
                    let score =
                        cosine_similarity(embedding.dimensions(), document.embedding.dimensions());
                    (score > 0.0).then(|| LocalVectorCandidate {
                        id: document.id.clone(),
                        score,
                    })
                })
                .collect::<Vec<_>>();

            candidates.sort_by(|left, right| {
                right
                    .score
                    .total_cmp(&left.score)
                    .then_with(|| left.id.cmp(&right.id))
            });
            candidates.truncate(limit);
            Ok(candidates)
        }
    }

    fn embed_text(text: &str) -> Vec<f32> {
        let mut vector = vec![0.0; DIMENSIONS];
        for token in normalized_tokens(text) {
            let bucket = token_hash(&token) % DIMENSIONS;
            vector[bucket] += 1.0;
        }
        normalize(&mut vector);
        vector
    }

    fn normalized_tokens(text: &str) -> Vec<String> {
        let mut tokens = Vec::new();
        for raw in text
            .split(|character: char| !character.is_ascii_alphanumeric())
            .filter(|part| !part.is_empty())
        {
            let normalized = normalize_token(raw);
            if normalized.len() >= 3 {
                tokens.push(normalized);
            }
        }
        tokens.sort();
        tokens
    }

    fn normalize_token(raw: &str) -> String {
        let lower = raw.to_ascii_lowercase();
        let without_suffix = lower
            .strip_suffix("ies")
            .map(|prefix| format!("{prefix}y"))
            .or_else(|| lower.strip_suffix("ing").map(ToOwned::to_owned))
            .or_else(|| lower.strip_suffix("ed").map(ToOwned::to_owned))
            .or_else(|| lower.strip_suffix('s').map(ToOwned::to_owned))
            .unwrap_or(lower);
        synonym(&without_suffix).to_string()
    }

    fn synonym(token: &str) -> &str {
        let synonyms = BTreeMap::from([
            ("callback", "webhook"),
            ("callbacks", "webhook"),
            ("reconcile", "settlement"),
            ("reconciliation", "settlement"),
            ("reimbursement", "refund"),
            ("reimburse", "refund"),
            ("repeat", "retry"),
            ("repeated", "retry"),
            ("duplicate", "idempotency"),
            ("duplicated", "idempotency"),
        ]);
        synonyms.get(token).copied().unwrap_or(token)
    }

    fn token_hash(token: &str) -> usize {
        token.bytes().fold(0usize, |hash, byte| {
            hash.wrapping_mul(31).wrapping_add(byte as usize)
        })
    }

    fn normalize(vector: &mut [f32]) {
        let magnitude = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
        if magnitude == 0.0 {
            return;
        }
        for value in vector {
            *value /= magnitude;
        }
    }

    fn cosine_similarity(left: &[f32], right: &[f32]) -> f32 {
        left.iter()
            .zip(right.iter())
            .map(|(left, right)| left * right)
            .sum()
    }
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

#[cfg(all(test, feature = "sqlite-vec"))]
mod local_vector_tests {
    use super::local::{InMemoryVectorIndex, VectorDocument};
    use super::VectorCandidate;

    #[test]
    fn indexes_small_synthetic_docs_and_finds_semantic_like_match() {
        let index = InMemoryVectorIndex::from_documents([
            VectorDocument::new(
                "policy.refunds",
                "Refund webhooks must preserve idempotency during settlement retry handling.",
            ),
            VectorDocument::new(
                "policy.frontend",
                "React components should keep form state accessible.",
            ),
        ])
        .expect("build local vector index");

        let candidates = index
            .search_text("repeated reimbursement callback reconciliation", 2)
            .expect("search local vector index");

        assert_eq!(index.len(), 2);
        assert_eq!(candidates[0].id(), "policy.refunds");
        assert!(candidates[0].score() > 0.5);
    }

    #[test]
    fn search_returns_candidate_ids_not_raw_chunks() {
        let raw_chunk = "Secret-like raw vector chunk text should not be returned directly.";
        let index = InMemoryVectorIndex::from_documents([VectorDocument::new(
            "policy.raw-hidden",
            raw_chunk,
        )])
        .expect("build local vector index");

        let candidates = index
            .search_text("secret raw vector chunk", 1)
            .expect("search local vector index");
        let rendered = format!("{:?}", candidates[0]);

        assert_eq!(candidates[0].id(), "policy.raw-hidden");
        assert!(!rendered.contains(raw_chunk));
        assert!(!rendered.contains("Secret-like"));
    }
}
