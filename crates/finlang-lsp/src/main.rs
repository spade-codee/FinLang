//! `finlang-lsp` server binary — speaks LSP over stdin/stdout.
//!
//! VS Code (and any other LSP client) launches this binary as a child
//! process and exchanges JSON-RPC messages with it.  All real work happens
//! inside the [`finlang_lsp::Backend`] in the library crate.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use finlang_lsp::Backend;
use tower_lsp::{LspService, Server};

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let (service, socket) = LspService::new(Backend::new);
    Server::new(stdin, stdout, socket).serve(service).await;
}
