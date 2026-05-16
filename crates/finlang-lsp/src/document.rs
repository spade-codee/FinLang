//! In-memory document store used by the LSP backend.
//!
//! Documents are keyed by [`Url`] and stored in a [`DashMap`] so concurrent
//! request handlers (`hover`, `completion`, …) can read them without blocking
//! `didChange` writers.  Each entry carries:
//!
//! * a [`Rope`] of the current text — used by `convert.rs` for byte/UTF-16
//!   position arithmetic, and re-rendered to `&str` for analysis on change;
//! * the LSP `version` reported by the client, so stale `publishDiagnostics`
//!   notifications can be suppressed.

use dashmap::DashMap;
use ropey::Rope;
use tower_lsp::lsp_types::Url;

/// A single open document tracked by the backend.
#[derive(Debug, Clone)]
pub struct Document {
    /// Rope-backed source text (cheap O(log n) edits and slicing).
    pub rope: Rope,
    /// The most recent LSP version number from the client.
    pub version: i32,
}

impl Document {
    /// Construct a new document from full text.
    #[must_use]
    pub fn new(text: &str, version: i32) -> Self {
        Self {
            rope: Rope::from_str(text),
            version,
        }
    }

    /// Replace the entire document text (used for `TextDocumentSyncKind::FULL`).
    pub fn replace(&mut self, text: &str, version: i32) {
        self.rope = Rope::from_str(text);
        self.version = version;
    }

    /// Materialise the rope back into a `String` for the analyzer.
    #[must_use]
    pub fn text(&self) -> String {
        self.rope.to_string()
    }
}

/// Concurrent map of open documents.
#[derive(Debug, Default)]
pub struct DocumentStore {
    inner: DashMap<Url, Document>,
}

impl DocumentStore {
    /// Construct an empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert or replace a document.
    pub fn open(&self, uri: Url, text: &str, version: i32) {
        self.inner.insert(uri, Document::new(text, version));
    }

    /// Replace the text of an already-open document.  No-op if absent.
    pub fn replace(&self, uri: &Url, text: &str, version: i32) {
        if let Some(mut entry) = self.inner.get_mut(uri) {
            entry.replace(text, version);
        }
    }

    /// Remove a document from the store.
    pub fn close(&self, uri: &Url) {
        self.inner.remove(uri);
    }

    /// Run `f` on the document for `uri`, returning its result.
    ///
    /// Returns `None` when the document is not in the store.
    pub fn with<F, R>(&self, uri: &Url, f: F) -> Option<R>
    where
        F: FnOnce(&Document) -> R,
    {
        self.inner.get(uri).map(|entry| f(&entry))
    }
}
