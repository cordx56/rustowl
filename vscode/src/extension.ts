import * as vscode from "vscode";

import { zInfer, zLspCursorResponse, zLspRange } from "./schemas";
import { bootstrapRustowl } from "./bootstrap";
import {
  LanguageClient,
  ServerOptions,
  Executable,
  TransportKind,
  LanguageClientOptions,
} from "vscode-languageclient/node";

export let client: LanguageClient | undefined = undefined;

let decoTimer: NodeJS.Timeout | null = null;
let enabled: boolean = true;

export function activate(context: vscode.ExtensionContext) {
  console.log("rustowl activated");

  const lspExec: Executable = {
    command: "rustowl",
    transport: TransportKind.stdio,
  };
  const serverOptions: ServerOptions = lspExec;
  const clientOptions: LanguageClientOptions = {
    documentSelector: [{ scheme: "file", language: "rust" }],
  };

  (async () => {
    try {
      const exec = await bootstrapRustowl(context.globalStorageUri.fsPath);
      serverOptions.command = exec;

      client = new LanguageClient(
        "rustowl",
        "RustOwl",
        serverOptions,
        clientOptions,
      );
      client.start();
    } catch (e) {
      vscode.window.showErrorMessage(`Failed to start RustOwl\n${e}`);
    }
  })();

  let activeEditor: vscode.TextEditor | undefined =
    vscode.window.activeTextEditor;

  const statusBar = vscode.window.createStatusBarItem(
    vscode.StatusBarAlignment.Left,
    0,
  );
  statusBar.text = "RustOwl";
  statusBar.command = {
    command: "rustowl.toggle",
    title: "toggle",
    tooltip: "Toggle RustOwl on hover",
  };
  statusBar.show();

  let lifetimeDecorationType = vscode.window.createTextEditorDecorationType({});
  let moveDecorationType = vscode.window.createTextEditorDecorationType({});
  let imBorrowDecorationType = vscode.window.createTextEditorDecorationType({});
  let mBorrowDecorationType = vscode.window.createTextEditorDecorationType({});
  let outLiveDecorationType = vscode.window.createTextEditorDecorationType({});
  let emptyDecorationType = vscode.window.createTextEditorDecorationType({});

  // update decoration
  const updateDecoration = (
    editor: vscode.TextEditor,
    data: zInfer<typeof zLspCursorResponse>,
  ) => {
    const rangeToRange = (range: zInfer<typeof zLspRange>) => {
      return new vscode.Range(
        new vscode.Position(range.start.line, range.start.character),
        new vscode.Position(range.end.line, range.end.character),
      );
    };

    const {
      underlineThickness,
      lifetimeColor,
      moveCallColor,
      immutableBorrowColor,
      mutableBorrowColor,
      outliveColor,
      highlightBackground,
      defaultEnabled,
    } = vscode.workspace.getConfiguration("rustowl");
    enabled = defaultEnabled;

    function createDecorationType(
      color: string,
      highlightBackground: boolean,
    ): vscode.TextEditorDecorationType {
      return highlightBackground
        ? vscode.window.createTextEditorDecorationType({
            backgroundColor: color,
          })
        : vscode.window.createTextEditorDecorationType({
            textDecoration: `underline solid ${underlineThickness}px ${color}`,
          });
    }

    lifetimeDecorationType = createDecorationType(
      lifetimeColor,
      highlightBackground,
    );
    moveDecorationType = createDecorationType(
      moveCallColor,
      highlightBackground,
    );
    imBorrowDecorationType = createDecorationType(
      immutableBorrowColor,
      highlightBackground,
    );
    mBorrowDecorationType = createDecorationType(
      mutableBorrowColor,
      highlightBackground,
    );
    outLiveDecorationType = createDecorationType(
      outliveColor,
      highlightBackground,
    );
    emptyDecorationType = vscode.window.createTextEditorDecorationType({});

    const lifetime: vscode.DecorationOptions[] = [];
    const immut: vscode.DecorationOptions[] = [];
    const mut: vscode.DecorationOptions[] = [];
    const moveCall: vscode.DecorationOptions[] = [];
    const outlive: vscode.DecorationOptions[] = [];
    const messages: vscode.DecorationOptions[] = [];
    for (const deco of data.decorations) {
      const range = rangeToRange(deco.range);
      if (!deco.overlapped) {
        if (deco.type === "lifetime") {
          lifetime.push({
            range,
          });
        } else if (deco.type === "imm_borrow") {
          immut.push({ range });
        } else if (deco.type === "mut_borrow") {
          mut.push({ range });
        } else if (deco.type === "call" || deco.type === "move") {
          moveCall.push({ range });
        } else if (deco.type === "shared_mut" || deco.type === "outlive") {
          outlive.push({ range });
        }
      }
      if ("hover_text" in deco && deco.hover_text) {
        messages.push({ range, hoverMessage: deco.hover_text });
      }
    }
    editor.setDecorations(lifetimeDecorationType, lifetime);
    editor.setDecorations(imBorrowDecorationType, immut);
    editor.setDecorations(mBorrowDecorationType, mut);
    editor.setDecorations(moveDecorationType, moveCall);
    editor.setDecorations(outLiveDecorationType, outlive);
    editor.setDecorations(emptyDecorationType, messages);
  };
  const resetDecoration = () => {
    lifetimeDecorationType.dispose();
    moveDecorationType.dispose();
    imBorrowDecorationType.dispose();
    mBorrowDecorationType.dispose();
    outLiveDecorationType.dispose();
    emptyDecorationType.dispose();
  };

  const rustowlHoverRequest = async (
    textEditor: vscode.TextEditor,
    select: vscode.Position,
    uri: vscode.Uri,
  ) => {
    const req = client?.sendRequest("rustowl/cursor", {
      position: {
        line: select.line,
        character: select.character,
      },
      document: { uri: uri.toString() },
    });
    const resp = await req;
    const data = zLspCursorResponse.safeParse(resp);
    if (data.success) {
      console.log(data.data);
      if (data.data.status === "finished") {
        statusBar.text = "$(check) RustOwl";
        statusBar.tooltip = "analyze finished";
      } else if (data.data.status === "analyzing") {
        statusBar.text = "$(loading~spin) RustOwl";
        statusBar.tooltip = "analyzing";
      } else {
        statusBar.text = "$(error) RustOwl";
        statusBar.tooltip = "analyze failed";
      }
      statusBar.show();
      updateDecoration(textEditor, data.data);
    }
  };

  context.subscriptions.push(
    vscode.commands.registerCommand("rustowl.hover", async (_args) => {
      if (activeEditor) {
        await rustowlHoverRequest(
          activeEditor,
          activeEditor?.selection.active,
          activeEditor?.document.uri,
        );
      }
    }),
    vscode.commands.registerCommand("rustowl.toggle", async (_args) => {
      enabled = !enabled;
      if (enabled) {
        statusBar.text = "RustOwl";
        statusBar.show();
      } else {
        statusBar.text = "$(debug-pause) RustOwl";
        statusBar.show();
      }
    }),
  );

  // events
  vscode.window.onDidChangeActiveTextEditor(
    (editor) => {
      activeEditor = editor;
    },
    null,
    context.subscriptions,
  );
  //let timeout: NodeJS.Timeout | undefined = undefined;
  vscode.workspace.onDidSaveTextDocument(
    async (ev) => {
      if (enabled && ev.languageId === "rust") {
        await client?.sendRequest("rustowl/analyze", {});
      }
    },
    null,
    context.subscriptions,
  );
  vscode.window.onDidChangeTextEditorSelection(
    (ev) => {
      const { displayDelay } = vscode.workspace.getConfiguration("rustowl");
      if (enabled && ev.textEditor === activeEditor) {
        resetDecoration();
        if (decoTimer) {
          clearTimeout(decoTimer);
          decoTimer = null;
        }
        decoTimer = setTimeout(async () => {
          const select = ev.textEditor.selection.active;
          const uri = ev.textEditor.document.uri;
          rustowlHoverRequest(ev.textEditor, select, uri);
        }, displayDelay);
      }
    },
    null,
    context.subscriptions,
  );
}

export function deactivate() {
  if (client) {
    client.stop();
  }
}
