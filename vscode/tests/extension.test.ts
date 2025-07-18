import * as bootstrap from "../src/bootstrap.js";
import sinon from "sinon";
import { describe, it, beforeEach, afterEach } from "mocha";
import * as vscode from "vscode";
import * as extension from "../src/extension.js";
import { context } from "./mocks.js";
import assert from "node:assert/strict";
import { EventEmitter } from "node:events";
import { LanguageClient } from "vscode-languageclient/node";

describe("Extension Test Suite", () => {
  let sandbox: sinon.SinonSandbox;
  let commandStub: sinon.SinonStub;
  let activeEditor: any;
  let onDidChangeActiveTextEditor: EventEmitter;
  let onDidSaveTextDocument: EventEmitter;
  let onDidChangeTextEditorSelection: EventEmitter;
  let clientStartStub: sinon.SinonStub;
  let clientStopStub: sinon.SinonStub;
  let sendRequestStub: sinon.SinonStub;
  let decorationType: any;

  beforeEach(() => {
    sandbox = sinon.createSandbox();
    commandStub = sandbox.stub(vscode.commands, "registerCommand");

    activeEditor = {
      document: { uri: vscode.Uri.parse("file:///test.rs") },
      selection: { active: new vscode.Position(1, 1) },
      setDecorations: sandbox.stub(),
    };
    sandbox.stub(vscode.window, "activeTextEditor").value(activeEditor);

    onDidChangeActiveTextEditor = new EventEmitter();
    onDidSaveTextDocument = new EventEmitter();
    onDidChangeTextEditorSelection = new EventEmitter();

    sandbox
      .stub(vscode.window, "onDidChangeActiveTextEditor")
      .callsFake((fn) => {
        onDidChangeActiveTextEditor.on("change", fn);
        return { dispose: () => {} };
      });
    sandbox.stub(vscode.workspace, "onDidSaveTextDocument").callsFake((fn) => {
      onDidSaveTextDocument.on("change", fn);
      return { dispose: () => {} };
    });
    sandbox
      .stub(vscode.window, "onDidChangeTextEditorSelection")
      .callsFake((fn) => {
        onDidChangeTextEditorSelection.on("change", fn);
        return { dispose: () => {} };
      });

    decorationType = { key: "test-decoration", dispose: sandbox.stub() };
    sandbox
      .stub(vscode.window, "createTextEditorDecorationType")
      .returns(decorationType);

    clientStartStub = sandbox.stub(LanguageClient.prototype, "start");
    clientStopStub = sandbox.stub(LanguageClient.prototype, "stop");
    sendRequestStub = sandbox.stub(LanguageClient.prototype, "sendRequest");
  });

  afterEach(() => {
    sandbox.restore();
  });

  /* ---------- activation ---------- */
  it("activates and boots successfully", async () => {
    const bootstrapStub = sandbox
      .stub(bootstrap, "bootstrapRustowl")
      .resolves("rustowl");
    await extension.activate(context);
    assert.equal(bootstrapStub.callCount, 1);
    assert.equal(clientStartStub.callCount, 1);
  });

  it("deactivates cleanly", () => {
    extension.deactivate();
    assert.equal(clientStopStub.callCount, 1);
  });

  it("handles hover request", async () => {
    sendRequestStub.resolves({
      is_analyzed: true,
      status: "finished",
      decorations: [],
    });
    await extension.activate(context);
    const command = commandStub.getCall(0).args[1];
    await command();
    assert.equal(sendRequestStub.callCount, 1);
  });

  /* ---------- decorations ---------- */
  it("creates decoration types with user config", async () => {
    const getConfigStub = sandbox.stub(vscode.workspace, "getConfiguration");
    getConfigStub.returns({
      get: sandbox.stub().returns(2),
      has: sandbox.stub().returns(true),
      inspect: sandbox.stub(),
      update: sandbox.stub().resolves(),
      underlineThickness: 2,
      lifetimeColor: "#ff0000",
      moveCallColor: "#00ff00",
      immutableBorrowColor: "#0000ff",
      mutableBorrowColor: "#ffff00",
      outliveColor: "#ff00ff",
    });

    await extension.activate(context);
    assert.equal(
      (vscode.window.createTextEditorDecorationType as sinon.SinonStub)
        .callCount,
      6,
    );
  });

  /* ---------- hover handler ---------- */
  it("handles invalid LSP response", async () => {
    sendRequestStub.resolves({ garbage: true });
    await extension.activate(context);
    const command = commandStub.getCall(0).args[1];
    await command();
  });

  it("handles status = error", async () => {
    sendRequestStub.resolves({
      is_analyzed: true,
      status: "error",
      decorations: [],
    });
    await extension.activate(context);
    const command = commandStub.getCall(0).args[1];
    await command();
  });

  it("skips decoration when not active editor", () => {
    const otherEditor = {
      document: { uri: vscode.Uri.parse("file:///other.rs") },
    };
    onDidChangeTextEditorSelection.emit("change", {
      textEditor: otherEditor,
      selections: [{ active: new vscode.Position(0, 0) }],
    });
    assert.equal(sendRequestStub.callCount, 0);
  });

  it("clears timeout on reset", async () => {
    await extension.activate(context);
    onDidChangeTextEditorSelection.emit("change", {
      textEditor: activeEditor,
      selections: [{ active: new vscode.Position(0, 0) }],
    });
    extension.deactivate();
  });
});
