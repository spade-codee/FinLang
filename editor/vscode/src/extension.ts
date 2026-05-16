// FinLang VS Code extension entry point.
//
// On activation the extension spawns the `finlang-lsp` binary (configurable
// via `finlang.server.path`) and forwards every `*.fin` document to it via
// the LSP protocol over stdio.

import * as vscode from "vscode";
import {
    LanguageClient,
    LanguageClientOptions,
    ServerOptions,
    TransportKind,
} from "vscode-languageclient/node";

let client: LanguageClient | undefined;

export function activate(context: vscode.ExtensionContext): void {
    const config = vscode.workspace.getConfiguration("finlang");
    const serverPath = config.get<string>("server.path", "finlang-lsp");

    const serverOptions: ServerOptions = {
        run: { command: serverPath, transport: TransportKind.stdio },
        debug: { command: serverPath, transport: TransportKind.stdio },
    };

    const clientOptions: LanguageClientOptions = {
        documentSelector: [{ scheme: "file", language: "finlang" }],
        synchronize: {
            fileEvents: vscode.workspace.createFileSystemWatcher("**/*.fin"),
        },
    };

    client = new LanguageClient(
        "finlang",
        "FinLang Language Server",
        serverOptions,
        clientOptions
    );

    client.start();
    context.subscriptions.push({
        dispose: () => {
            client?.stop();
        },
    });
}

export function deactivate(): Thenable<void> | undefined {
    if (!client) {
        return undefined;
    }
    return client.stop();
}
