import { spawn, spawnSync } from "node:child_process";
import fs from "node:fs/promises";

import * as vscode from "vscode";

import packageJson from "../package.json";

const version: string = packageJson.version;

export const hostTuple = (): string | null => {
  let arch = null;
  if (process.arch === "arm64") {
    arch = "aarch64";
  } else if (process.arch === "x64") {
    arch = "x86_64";
  }
  let platform = null;
  if (process.platform === "linux") {
    platform = "unknown-linux-gnu";
  } else if (process.platform === "darwin") {
    platform = "apple-darwin";
  } else if (process.platform === "win32") {
    platform = "pc-windows-msvc";
  }
  return arch !== null && platform !== null ? `${arch}-${platform}` : null;
};

const exeExt = hostTuple()?.includes("windows") === true ? ".exe" : "";

export const downloadRustowl = async (basePath: string) => {
  const baseUrl = `https://github.com/cordx56/rustowl/releases/download/v${version}`;
  const host = hostTuple();
  if (host !== null) {
    const owl = await fetch(`${baseUrl}/rustowl-${host}${exeExt}`);
    if (owl.status !== 200) {
      throw new Error("RustOwl download error");
    }
    const filePath = `${basePath}/rustowl${exeExt}`;
    // eslint-disable-next-line security/detect-non-literal-fs-filename
    await fs.writeFile(filePath, Buffer.from(await owl.arrayBuffer()), {
      flag: "w",
    });
    // eslint-disable-next-line security/detect-non-literal-fs-filename
    await fs.chmod(filePath, "755");
  } else {
    throw new Error("unsupported architecture or platform");
  }
};

const exists = async (path: string) => {
  try {
    await fs.access(path);
    return true;
  } catch {
    return false;
  }
};
export const needUpdated = async (currentVersion: string) => {
  if (!currentVersion) {
    return true;
  }
  // eslint-disable-next-line no-console
  console.log(`current RustOwl version: ${currentVersion.trim()}`);
  // eslint-disable-next-line no-console
  console.log(`extension version: v${version}`);
  try {
    const semverParser = await import("semver-parser");
    const current = semverParser.parseSemVer(currentVersion.trim(), false);
    const self = semverParser.parseSemVer(version, false);
    return current.major === self.major &&
      current.minor === self.minor &&
      current.patch === self.patch &&
      JSON.stringify(current.pre) === JSON.stringify(self.pre)
      ? false
      : true;
  } catch {
    return true;
  }
};
const getRustowlCommand = async (dirPath: string) => {
  const rustowlPath = `${dirPath}/rustowl${exeExt}`;
  if (spawnSync("rustowl", ["--version", "--quiet"]).stdout?.toString()) {
    return "rustowl";
  } else if (
    (await exists(rustowlPath)) &&
    spawnSync(rustowlPath, ["--version", "--quiet"]).stdout?.toString()
  ) {
    return rustowlPath;
  } else {
    return null;
  }
};

export const bootstrapRustowl = async (dirPath: string): Promise<string> => {
  let rustowlCommand = await getRustowlCommand(dirPath);
  if (
    rustowlCommand === null ||
    (await needUpdated(
      spawnSync(rustowlCommand, ["--version", "--quiet"]).stdout?.toString(),
    ))
  ) {
    // eslint-disable-next-line security/detect-non-literal-fs-filename
    await fs.mkdir(dirPath, { recursive: true });
    // download rustowl binary
    await vscode.window.withProgress(
      {
        location: vscode.ProgressLocation.Notification,
        title: "RustOwl installing",
        cancellable: false,
      },
      async (progress) => {
        try {
          progress.report({ message: "binary downloading" });
          await downloadRustowl(dirPath);
          progress.report({ message: "binary downloaded" });
        } catch (error) {
          vscode.window.showErrorMessage(
            `${error instanceof Error ? error.message : String(error)}`,
          );
        }
      },
    );
    rustowlCommand = await vscode.window.withProgress(
      {
        location: vscode.ProgressLocation.Notification,
        title: "Setup RustOwl toolchain",
        cancellable: false,
      },
      async (progress) => {
        try {
          rustowlCommand = await getRustowlCommand(dirPath);

          if (rustowlCommand === null) {
            throw new Error("failed to install RustOwl");
          }

          const installer = spawn(rustowlCommand, ["toolchain", "install"], {
            stdio: ["ignore", "ignore", "pipe"],
          });
          installer.stderr.addListener("data", (data) => {
            if (`${data}`.includes("%")) {
              progress.report({
                message: "toolchain downloading",
                increment: 0.25, // downloads 4 toolchain components
              });
            }
          });
          return new Promise<string | null>((resolve, reject) => {
            installer.addListener("exit", (code) => {
              if (code === 0) {
                resolve(rustowlCommand);
              } else {
                reject(`toolchain setup failed (exit code ${code})`);
              }
            });
          });
        } catch (error) {
          vscode.window.showErrorMessage(
            `${error instanceof Error ? error.message : String(error)}`,
          );
        }
        return null;
      },
    );
  }

  if (rustowlCommand === null) {
    throw new Error("failed to install RustOwl");
  }

  return rustowlCommand;
};
