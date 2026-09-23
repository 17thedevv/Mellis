import * as path from 'path';
import { workspace, ExtensionContext } from 'vscode';
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
  Executable
} from 'vscode-languageclient/node';

let client: LanguageClient;

export function activate(context: ExtensionContext) {
  // Path to luna.exe. 
  // Extension directory is in `editors/vscode`, step back 2 levels to repo `bin`.
  const serverPath = context.asAbsolutePath(path.join('..', '..', 'bin', 'luna.exe'));

  const run: Executable = {
    command: serverPath,
    args: ['--lsp'],
    options: { cwd: workspace.workspaceFolders?.[0]?.uri.fsPath }
  };

  const serverOptions: ServerOptions = {
    run,
    debug: run
  };

  const clientOptions: LanguageClientOptions = {
    documentSelector: [{ scheme: 'file', language: 'luna' }],
  };

  client = new LanguageClient(
    'lunaLsp',
    'Luna Language Server',
    serverOptions,
    clientOptions
  );

  client.start();
}

export function deactivate(): Thenable<void> | undefined {
  if (!client) {
    return undefined;
  }
  return client.stop();
}
