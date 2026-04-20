import * as vscode from 'vscode';
import which from 'which';

function findIrisDev(): string | null {
  const cfg = vscode.workspace.getConfiguration('iris-dev');
  const override = cfg.get<string>('serverPath');
  if (override) { return override; }
  try { return which.sync('iris-dev'); } catch { return null; }
}

interface ObjectScriptConn {
  active: boolean;
  host?: string;
  port?: number;
  ns?: string;
  username?: string;
  password?: string;
  server?: string;
}

interface NamedServer {
  webServer: {
    host?: string;
    port?: number;
    scheme?: string;
    pathPrefix?: string;
  };
  superServer?: { host?: string; port: number; };
  ns?: string;
  username?: string;
  password?: string;
}

export class IrisDevMcpProvider
  implements vscode.McpServerDefinitionProvider<vscode.McpStdioServerDefinition>, vscode.Disposable
{
  private readonly emitter = new vscode.EventEmitter<void>();
  private readonly watcher: vscode.Disposable;

  public readonly onDidChangeMcpServerDefinitions = this.emitter.event;

  constructor() {
    this.watcher = vscode.workspace.onDidChangeConfiguration(e => {
      if (
        e.affectsConfiguration('objectscript.conn') ||
        e.affectsConfiguration('iris-dev.nativePort') ||
        e.affectsConfiguration('iris-dev.serverPath') ||
        e.affectsConfiguration('intersystems.servers')
      ) {
        this.emitter.fire();
      }
    });
  }

  dispose() {
    this.watcher.dispose();
    this.emitter.dispose();
  }

  refresh() { this.emitter.fire(); }

  public provideMcpServerDefinitions(
    _token: vscode.CancellationToken
  ): vscode.ProviderResult<vscode.McpStdioServerDefinition[]> {
    const conn = vscode.workspace
      .getConfiguration('objectscript', null)
      .get<ObjectScriptConn>('conn');

    if (!conn || conn.active === false) {
      vscode.window.showWarningMessage(
        'iris-dev: ObjectScript connection is not configured or inactive.'
      );
      return [];
    }

    const command = findIrisDev();
    if (!command) {
      vscode.window.showErrorMessage(
        'iris-dev: binary not found. ' +
        'Download from https://github.com/intersystems-community/iris-dev/releases ' +
        'or set iris-dev.serverPath in VS Code settings.'
      );
      return [];
    }

    // Resolve named server if using intersystems.servers
    let named: NamedServer | null = null;
    if (conn.server) {
      const servers = vscode.workspace
        .getConfiguration('intersystems', null)
        .get<Record<string, NamedServer>>('servers');
      if (!servers) {
        vscode.window.showWarningMessage(
          `iris-dev: named connection "${conn.server}" not found in intersystems.servers.`
        );
        return [];
      }
      named = servers[conn.server] ?? null;
    }

    const host = conn.host ?? 'localhost';
    const webPort = conn.port ?? 52773;
    const namespace = conn.ns ?? 'USER';

    const mcpCfg = vscode.workspace.getConfiguration('iris-dev', null);
    const nativePort = named?.superServer?.port ?? mcpCfg.get<number>('nativePort') ?? 1972;

    const resolvedHost = (named?.superServer?.host ?? named?.webServer?.host) ?? host;
    const webPrefix = named?.webServer?.pathPrefix ?? null;

    const isIsfs = vscode.workspace.workspaceFolders?.some(
      f => f.uri.scheme === 'isfs' || f.uri.scheme === 'isfs-readonly'
    ) ?? false;

    if (resolvedHost !== 'localhost' && resolvedHost !== '127.0.0.1' && resolvedHost !== '::1') {
      vscode.window.showWarningMessage(
        `iris-dev: connected to remote IRIS host "${resolvedHost}". ` +
        'Recommended: use a local or dedicated dev instance.'
      );
    }

    const env: Record<string, string | number | null> = {
      IRIS_HOST: resolvedHost,
      IRIS_PORT: nativePort,
      IRIS_WEB_PORT: named?.webServer?.port ?? webPort,
      IRIS_WEB_PREFIX: webPrefix,
      IRIS_USERNAME: named?.username ?? conn.username ?? null,
      IRIS_PASSWORD: named?.password ?? conn.password ?? null,
      IRIS_NAMESPACE: named?.ns ?? namespace,
      IRIS_ISFS: isIsfs ? 'true' : null,
      OBJECTSCRIPT_LEARNING: 'true',
      OBJECTSCRIPT_SKILLMCP_NAMESPACE: 'USER',
    };

    const definition = new vscode.McpStdioServerDefinition(
      'iris-dev (IRIS)',
      command,
      ['mcp']           // iris-dev requires the "mcp" subcommand
    );
    definition.env = env;
    return [definition];
  }

  public async resolveMcpServerDefinition(
    server: vscode.McpStdioServerDefinition,
    token: vscode.CancellationToken
  ): Promise<vscode.McpStdioServerDefinition | undefined> {
    if (token.isCancellationRequested || !(server instanceof vscode.McpStdioServerDefinition)) {
      return server;
    }
    const env: Record<string, string | number | null> = { ...(server.env ?? {}) };
    if (!env.IRIS_PASSWORD) {
      const pw = await vscode.window.showInputBox({ prompt: 'IRIS password', password: true });
      if (pw !== undefined) { env.IRIS_PASSWORD = pw; server.env = env; }
    }
    return server;
  }
}

export function activate(context: vscode.ExtensionContext): void {
  const provider = new IrisDevMcpProvider();
  context.subscriptions.push(provider);
  if (typeof vscode.lm?.registerMcpServerDefinitionProvider === 'function') {
    context.subscriptions.push(
      vscode.lm.registerMcpServerDefinitionProvider('iris-dev', provider)
    );
    provider.refresh();
  } else {
    vscode.window.showWarningMessage(
      'iris-dev: MCP server registration requires VS Code 1.99+.'
    );
  }
}

export function deactivate(): void {}
