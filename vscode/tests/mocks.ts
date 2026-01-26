import * as fs from "fs";
import * as os from "os";
import * as path from "path";

import type { ExtensionContext, Memento } from "vscode";

const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "rustowl-vscode-test-"));

class MockMemento implements Memento {
  private _storage: Record<string, unknown> = {};
  get<T>(key: string): T | undefined;
  get<T>(key: string, defaultValue: T): T;
  get(key: string, defaultValue?: unknown): unknown {
    // eslint-disable-next-line security/detect-object-injection
    return key in this._storage ? this._storage[key] : defaultValue;
  }
  update(key: string, value: unknown): Promise<void> {
    // eslint-disable-next-line security/detect-object-injection
    this._storage[key] = value;
    return Promise.resolve();
  }
  keys(): readonly string[] {
    return Object.keys(this._storage);
  }
  setKeysForSync(_keys: string[]): void {
    //
  }
}

export const context: ExtensionContext = {
  extensionPath: tmpDir,
  storagePath: tmpDir,
  globalStoragePath: tmpDir,
  logPath: tmpDir,
  asAbsolutePath: (relativePath: string) => path.join(tmpDir, relativePath),
  storageUri: {
    fsPath: tmpDir,
    scheme: "file",
    authority: "",
    path: tmpDir,
    query: "",
    fragment: "",
    with: () => {
      throw new Error("Not implemented");
    },
    toJSON: () => {
      throw new Error("Not implemented");
    },
  },
  globalStorageUri: {
    fsPath: tmpDir,
    scheme: "file",
    authority: "",
    path: tmpDir,
    query: "",
    fragment: "",
    with: () => {
      throw new Error("Not implemented");
    },
    toJSON: () => {
      throw new Error("Not implemented");
    },
  },
  logUri: {
    fsPath: tmpDir,
    scheme: "file",
    authority: "",
    path: tmpDir,
    query: "",
    fragment: "",
    with: () => {
      throw new Error("Not implemented");
    },
    toJSON: () => {
      throw new Error("Not implemented");
    },
  },
  extensionUri: {
    fsPath: tmpDir,
    scheme: "file",
    authority: "",
    path: tmpDir,
    query: "",
    fragment: "",
    with: () => {
      throw new Error("Not implemented");
    },
    toJSON: () => {
      throw new Error("Not implemented");
    },
  },
  environmentVariableCollection: {
    persistent: false,
    replace: () => {
      throw new Error("Not implemented");
    },
    append: () => {
      throw new Error("Not implemented");
    },
    prepend: () => {
      throw new Error("Not implemented");
    },
    get: () => {
      throw new Error("Not implemented");
    },
    forEach: () => {
      throw new Error("Not implemented");
    },
    delete: () => {
      throw new Error("Not implemented");
    },
    clear: () => {
      throw new Error("Not implemented");
    },
    [Symbol.iterator]: () => {
      throw new Error("Not implemented");
    },
    getScoped: () => {
      throw new Error("Not implemented");
    },
    description: "",
  },
  extensionMode: 3,
  globalState: new MockMemento(),
  workspaceState: new MockMemento(),
  secrets: {
    get: () => Promise.resolve(undefined),
    store: () => Promise.resolve(),
    delete: () => Promise.resolve(),
    keys: () => Promise.resolve([]),
    onDidChange: () => {
      throw new Error("Not implemented");
    },
  },
  subscriptions: [],
  extension: {
    id: "rustowl.rustowl-vscode",
    extensionUri: {
      fsPath: tmpDir,
      scheme: "file",
      authority: "",
      path: tmpDir,
      query: "",
      fragment: "",
      with: () => {
        throw new Error("Not implemented");
      },
      toJSON: () => {
        throw new Error("Not implemented");
      },
    },
    extensionPath: tmpDir,
    isActive: false,
    packageJSON: {},
    extensionKind: 1,
    exports: {},
    activate: () => {
      throw new Error("Not implemented");
    },
  },
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  languageModelAccessInformation: undefined as any,
};
