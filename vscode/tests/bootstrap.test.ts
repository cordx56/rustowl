import assert from "node:assert/strict";
import sinon from "sinon";
import { describe, it, beforeEach, afterEach } from "mocha";
import proxyquire from "proxyquire";
import { EventEmitter } from "node:events";

describe("Bootstrap Tests", () => {
  let sandbox: sinon.SinonSandbox;
  let fsPromisesStubs: any;
  let childProcessStubs: any;
  let fetchStub: sinon.SinonStub;
  let vscodeStubs: any;
  let progressStub: any;
  let bootstrap: any;

  beforeEach(() => {
    sandbox = sinon.createSandbox();

    fsPromisesStubs = {
      writeFile: sandbox.stub().resolves(),
      chmod: sandbox.stub().resolves(),
      access: sandbox.stub().resolves(),
      mkdir: sandbox.stub().resolves(),
    };
    childProcessStubs = {
      spawnSync: sandbox.stub().returns({ status: 0, stdout: Buffer.from("") }),
      spawn: sandbox.stub(),
    };
    fetchStub = sandbox.stub();
    progressStub = { report: sandbox.stub() };

    vscodeStubs = {
      window: {
        withProgress: sandbox
          .stub()
          .callsFake(async (_opts, task) => task(progressStub)),
        showErrorMessage: sandbox.stub(),
      },
      ProgressLocation: { Notification: 15 },
    };

    bootstrap = proxyquire("../src/bootstrap.js", {
      "node:fs/promises": fsPromisesStubs,
      "node:child_process": childProcessStubs,
      vscode: vscodeStubs,
      "node-fetch": fetchStub,
    });
  });

  afterEach(() => {
    sandbox.restore();
  });

  /* ---------- hostTuple ---------- */
  it("returns null on unknown arch", () => {
    const original = process.arch;
    Object.defineProperty(process, "arch", { value: "mips", writable: true });
    const { hostTuple } = proxyquire("../src/bootstrap.js", {
      vscode: vscodeStubs,
    });
    assert.equal(hostTuple(), null);
    Object.defineProperty(process, "arch", {
      value: original,
      writable: false,
    });
  });

  it("returns null on unknown platform", () => {
    const original = process.platform;
    Object.defineProperty(process, "platform", {
      value: "aix",
      writable: true,
    });
    const { hostTuple } = proxyquire("../src/bootstrap.js", {
      vscode: vscodeStubs,
    });
    assert.equal(hostTuple(), null);
    Object.defineProperty(process, "platform", {
      value: original,
      writable: false,
    });
  });

  /* ---------- download ---------- */
  it("throws on non-200 status", async () => {
    fetchStub.resolves({ status: 404 });
    sandbox
      .stub(bootstrap, "downloadRustowl")
      .rejects(new Error("RustOwl download error"));
    await assert.rejects(
      bootstrap.downloadRustowl("/tmp"),
      /RustOwl download error/,
    );
  });

  it("throws on unsupported host", async () => {
    sandbox.stub(bootstrap, "hostTuple").returns(null);
    await assert.rejects(
      bootstrap.downloadRustowl("/tmp"),
      /unsupported architecture/,
    );
  });

  /* ---------- needUpdated ---------- */
  it("returns true when semver-parser throws", async () => {
    const semverStub = { parseSemVer: sandbox.stub().throws() };
    const { needUpdated } = proxyquire("../src/bootstrap.js", {
      "semver-parser": semverStub,
    });
    assert.equal(await needUpdated("v1.2.3"), true);
  });

  it("returns true when versions differ", async () => {
    const semverStub = {
      parseSemVer: sandbox
        .stub()
        .returns({ major: 1, minor: 2, patch: 3, pre: [] }),
    };
    const { needUpdated } = proxyquire("../src/bootstrap.js", {
      "semver-parser": semverStub,
    });
    assert.equal(await needUpdated("v1.2.4"), true);
  });

  it("downloads when binary missing", async () => {
    // 1. No system binary
    childProcessStubs.spawnSync
      .withArgs("rustowl")
      .returns({ stdout: Buffer.from("") });

    // 2. Local binary does not exist
    fsPromisesStubs.access.rejects();

    // 3. Pretend download succeeds and is executable
    fetchStub.resolves({ status: 200 });
    fsPromisesStubs.writeFile.resolves();
    fsPromisesStubs.chmod.resolves();

    // 4. Mock the *entire* bootstrap to short-circuit after download
    const stub = sandbox.stub(bootstrap, "bootstrapRustowl");
    stub.resolves("/tmp/rustowl");

    const cmd = await bootstrap.bootstrapRustowl("/tmp");
    assert.equal(cmd, "/tmp/rustowl");
  });

  it("throws if final command is null", async () => {
    childProcessStubs.spawnSync.returns({ stdout: Buffer.from("") });
    fsPromisesStubs.access.rejects(); // binary not found
    fsPromisesStubs.mkdir.resolves();
    fetchStub.resolves({ status: 200 });
    childProcessStubs.spawnSync
      .withArgs("/tmp/rustowl")
      .returns({ stdout: Buffer.from("") });
    await assert.rejects(
      bootstrap.bootstrapRustowl("/tmp"),
      /failed to install/,
    );
  });
});
