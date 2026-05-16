//! FinLang Language Server Protocol implementation.
//!
//! Built on [`tower_lsp`].  Reuses the same lexer / parser / type-checker
//! pipeline as the CLI — there is exactly one source of truth for FinLang
//! semantics.  The server speaks LSP over stdin/stdout (see `bin/finlang-lsp`)
//! and supports:
//!
//! * `textDocument/didOpen` / `didChange` / `didClose`
//! * `textDocument/publishDiagnostics` (push model, on every change)
//! * `textDocument/hover` — shows the inferred [`finlang_types::FinType`].
//! * `textDocument/completion` — keywords, types, stdlib, in-scope idents.
//! * `textDocument/definition` — jumps to the matching `let` / `fn` /
//!   `portfolio` declaration.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod analyze;
pub mod complete;
pub mod convert;
pub mod document;
pub mod goto_def;
pub mod hover;

use std::sync::Arc;

use tower_lsp::jsonrpc::Result as RpcResult;
use tower_lsp::lsp_types::{
    CompletionOptions, CompletionParams, CompletionResponse, Diagnostic, DiagnosticSeverity,
    DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    GotoDefinitionParams, GotoDefinitionResponse, HoverParams, HoverProviderCapability,
    InitializeParams, InitializeResult, InitializedParams, Location, MessageType, OneOf,
    PositionEncodingKind, ServerCapabilities, ServerInfo, TextDocumentSyncCapability,
    TextDocumentSyncKind, Url, WorkDoneProgressOptions,
};
use tower_lsp::{Client, LanguageServer};

use crate::analyze::{analyze, Analysis};
use crate::convert::{position_to_byte_offset, span_to_range};
use crate::document::DocumentStore;

/// The LSP backend.
///
/// One instance is shared by every connection.  The internal document store
/// is wrapped in [`Arc`] so handler futures can capture it cheaply.
pub struct Backend {
    /// Tower-LSP client handle for sending notifications back to the editor.
    client: Client,
    /// All open documents.
    documents: Arc<DocumentStore>,
}

impl Backend {
    /// Construct a new backend bound to `client`.
    #[must_use]
    pub fn new(client: Client) -> Self {
        Self {
            client,
            documents: Arc::new(DocumentStore::new()),
        }
    }

    /// Run the front-end on the current text and publish diagnostics.
    async fn refresh_diagnostics(&self, uri: Url, source: &str, version: i32) {
        let analysis = analyze(source);
        let diagnostics = build_diagnostics(&analysis, source);
        self.client
            .publish_diagnostics(uri, diagnostics, Some(version))
            .await;
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> RpcResult<InitializeResult> {
        Ok(InitializeResult {
            server_info: Some(ServerInfo {
                name: "finlang-lsp".into(),
                version: Some(env!("CARGO_PKG_VERSION").into()),
            }),
            capabilities: ServerCapabilities {
                position_encoding: Some(PositionEncodingKind::UTF16),
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                completion_provider: Some(CompletionOptions {
                    resolve_provider: Some(false),
                    trigger_characters: Some(vec![".".into(), ":".into()]),
                    work_done_progress_options: WorkDoneProgressOptions::default(),
                    all_commit_characters: None,
                    completion_item: None,
                }),
                definition_provider: Some(OneOf::Left(true)),
                ..Default::default()
            },
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "finlang-lsp ready")
            .await;
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let doc = params.text_document;
        self.documents.open(doc.uri.clone(), &doc.text, doc.version);
        self.refresh_diagnostics(doc.uri, &doc.text, doc.version).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        // Server declared FULL sync so each change carries the full new text in
        // changes[0].text.
        let Some(change) = params.content_changes.into_iter().next() else {
            return;
        };
        let uri = params.text_document.uri;
        let version = params.text_document.version;
        self.documents.replace(&uri, &change.text, version);
        self.refresh_diagnostics(uri, &change.text, version).await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        self.documents.close(&uri);
        self.client.publish_diagnostics(uri, Vec::new(), None).await;
    }

    async fn hover(&self, params: HoverParams) -> RpcResult<Option<tower_lsp::lsp_types::Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        let response = self.documents.with(&uri, |doc| {
            let source = doc.text();
            let analysis = analyze(&source);
            let offset = position_to_byte_offset(&doc.rope, position);
            hover::hover_at(&analysis, &doc.rope, offset)
        });
        Ok(response.flatten())
    }

    async fn completion(
        &self,
        params: CompletionParams,
    ) -> RpcResult<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri;
        let items = self.documents.with(&uri, |doc| {
            let source = doc.text();
            let analysis = analyze(&source);
            complete::completions(&analysis)
        });
        Ok(items.map(CompletionResponse::Array))
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> RpcResult<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        let response = self.documents.with(&uri, |doc| {
            let source = doc.text();
            let analysis = analyze(&source);
            let offset = position_to_byte_offset(&doc.rope, position);
            let span = goto_def::definition_at(&analysis, &source, offset)?;
            Some(Location {
                uri: uri.clone(),
                range: span_to_range(&doc.rope, span),
            })
        });
        Ok(response.flatten().map(GotoDefinitionResponse::Scalar))
    }

    async fn shutdown(&self) -> RpcResult<()> {
        Ok(())
    }
}

/// Translate parse + type errors into LSP diagnostics.
fn build_diagnostics(analysis: &Analysis, source: &str) -> Vec<Diagnostic> {
    let rope = ropey::Rope::from_str(source);
    let mut out = Vec::with_capacity(analysis.parse_errors.len() + analysis.type_errors.len());
    for e in &analysis.parse_errors {
        out.push(Diagnostic {
            range: span_to_range(&rope, e.span()),
            severity: Some(DiagnosticSeverity::ERROR),
            source: Some("finlang".into()),
            message: e.to_string(),
            ..Default::default()
        });
    }
    for e in &analysis.type_errors {
        out.push(Diagnostic {
            range: span_to_range(&rope, e.span()),
            severity: Some(DiagnosticSeverity::ERROR),
            source: Some("finlang".into()),
            message: e.to_string(),
            ..Default::default()
        });
    }
    out
}
