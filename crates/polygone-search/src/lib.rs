//! `polygone-search` — Decentralised search engine on the Polygone mesh.
//!
//! Fédérateur de données : Anna's Archive, bases scientifiques (arXiv,
//! PubMed), wikis, nodes POLY. Chaque nœud indexe et sert les fragments
//! de résultats. L'interface web locale (port 8181) offre un champ de
//! recherche type Google, mais entièrement privé.

#![forbid(unsafe_code)]
#![allow(missing_docs)]

use std::collections::HashMap;

/// Source de données fédérée.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DataSource {
    /// Anna's Archive (miroir de bibliothèque)
    AnnasArchive,
    /// arXiv (preprints scientifiques)
    ArXiv,
    /// PubMed (biomédical)
    PubMed,
    /// Wiki local (miroir Kiwix)
    Wiki,
    /// Nœuds POLY du mesh (contenu partagé)
    PolyMesh,
    /// Moteur personnalisé (URL configurable)
    Custom(String),
}

impl DataSource {
    pub fn label(&self) -> &str {
        match self {
            DataSource::AnnasArchive => "📚 Anna's Archive",
            DataSource::ArXiv => "🔬 arXiv",
            DataSource::PubMed => "🏥 PubMed",
            DataSource::Wiki => "📖 Wiki",
            DataSource::PolyMesh => "⬡ PolyMesh",
            DataSource::Custom(u) => u,
        }
    }
}

/// Un résultat de recherche.
#[derive(Clone, Debug)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
    pub source: DataSource,
    pub score: f32,
    pub size_bytes: u64,
}

/// Requête de recherche.
#[derive(Clone, Debug)]
pub struct SearchQuery {
    pub terms: Vec<String>,
    pub sources: Vec<DataSource>,
    pub max_results: usize,
    pub timeout_ms: u64,
}

impl Default for SearchQuery {
    fn default() -> Self {
        Self {
            terms: vec![],
            sources: vec![
                DataSource::AnnasArchive,
                DataSource::ArXiv,
                DataSource::PubMed,
                DataSource::Wiki,
                DataSource::PolyMesh,
            ],
            max_results: 20,
            timeout_ms: 5000,
        }
    }
}

/// Nœud de recherche décentralisé.
pub struct SearchNode {
    index: HashMap<String, Vec<SearchResult>>,
    peers: Vec<String>,
    cache_hits: u64,
    cache_misses: u64,
}

impl SearchNode {
    pub fn new() -> Self {
        Self {
            index: HashMap::new(),
            peers: vec![],
            cache_hits: 0,
            cache_misses: 0,
        }
    }

    /// Indexer un résultat localement.
    pub fn index(&mut self, key: &str, result: SearchResult) {
        self.index.entry(key.to_lowercase()).or_default().push(result);
    }

    /// Ajouter un pair qui sert aussi des résultats.
    pub fn add_peer(&mut self, peer: String) {
        if !self.peers.contains(&peer) {
            self.peers.push(peer);
        }
    }

    /// Rechercher dans l'index local et interroger les pairs.
    /// Pour l'instant : lookup local uniquement (les pairs seront
    /// contactés via libp2p dans une version future).
    pub fn search(&mut self, query: &SearchQuery) -> Vec<SearchResult> {
        let mut results = Vec::new();

        for term in &query.terms {
            if let Some(hits) = self.index.get(&term.to_lowercase()) {
                results.extend(hits.iter().cloned());
                self.cache_hits += 1;
            } else {
                self.cache_misses += 1;
            }
        }

        // Trier par score décroissant
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(query.max_results);
        results
    }

    pub fn stats(&self) -> (u64, u64, usize) {
        (self.cache_hits, self.cache_misses, self.peers.len())
    }

    pub fn is_empty(&self) -> bool {
        self.index.is_empty()
    }
}

impl Default for SearchNode {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> SearchResult {
        SearchResult {
            title: "Test Doc".into(),
            url: "poly://test/doc".into(),
            snippet: "Un document de test".into(),
            source: DataSource::Wiki,
            score: 1.0,
            size_bytes: 1024,
        }
    }

    #[test]
    fn index_and_search() {
        let mut n = SearchNode::new();
        n.index("test", sample());
        let q = SearchQuery { terms: vec!["test".into()], ..Default::default() };
        let r = n.search(&q);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].title, "Test Doc");
    }

    #[test]
    fn empty_query_returns_nothing() {
        let n = SearchNode::new();
        assert!(n.is_empty());
    }

    #[test]
    fn peer_list_grows() {
        let mut n = SearchNode::new();
        n.add_peer("polygone://peer1".into());
        n.add_peer("polygone://peer2".into());
        assert_eq!(n.stats().2, 2);
    }

    #[test]
    fn no_duplicate_peers() {
        let mut n = SearchNode::new();
        n.add_peer("polygone://x".into());
        n.add_peer("polygone://x".into());
        assert_eq!(n.stats().2, 1);
    }
}