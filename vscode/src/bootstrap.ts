import { spawn, spawnSync } from "node:child_process";
import fs from "node:fs/promises";
import path from "node:path";

import * as vscode from "vscode";

import packageJson from "../package.json";

const version: string = packageJson.version;

export class UserCancelledError extends Error {
  constructor() {
    super("RustOwl installation was cancelled by the user");
    this.name = "UserCancelledError";
  }
}

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
    const isUpdate = rustowlCommand !== null;
    const message = isUpdate
      ? `RustOwl v${version} is available. Do you want to update?`
      : `RustOwl is not installed. Do you want to install RustOwl v${version}?`;
    const actionLabel = isUpdate ? "Update" : "Install";
    const selection = await vscode.window.showInformationMessage(
      message,
      actionLabel,
      "Cancel",
    );
    if (selection !== actionLabel) {
      throw new UserCancelledError();
    }

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

          const installedCommand = rustowlCommand;
          const installer = spawn(installedCommand, ["toolchain", "install"], {
            stdio: ["ignore", "ignore", "pipe"],
            env: { ...process.env, RUST_LOG: "debug" },
          });

          let stderrOutput = "";
          const urlProgress = new Map<string, number>();
          let lastReportedPercent = 0;

          const progressPattern = /received from (.+?):\s+(\d+)%/;

          installer.stderr.addListener("data", (data) => {
            const dataStr = `${data}`;
            stderrOutput += dataStr;

            for (const line of dataStr.split("\n")) {
              const match = progressPattern.exec(line);
              if (match !== null) {
                const url = match[1];
                const percent = parseInt(match[2], 10);
                urlProgress.set(url, percent);

                const totalPercent =
                  [...urlProgress.values()].reduce((a, b) => a + b, 0) /
                  urlProgress.size;
                const increment = totalPercent - lastReportedPercent;
                if (increment > 0) {
                  progress.report({
                    message: "toolchain downloading",
                    increment,
                  });
                  lastReportedPercent = totalPercent;
                }
              }
            }
          });

          return await new Promise<string>((resolve, reject) => {
            installer.addListener("error", (error) => {
              reject(error);
            });
            installer.addListener("exit", (code) => {
              (async () => {
                if (code === 0) {
                  resolve(installedCommand);
                  return;
                }

                const errorMessage =
                  stderrOutput.trim() || `Process exited with code ${code}`;
                const logPath = await writeErrorLog(dirPath, errorMessage);
                await showDetailedError(errorMessage, logPath);
                reject(new Error(`toolchain setup failed (exit code ${code})`));
              })().catch((error) => {
                reject(error);
              });
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

const writeErrorLog = async (
  dirPath: string,
  error: string,
): Promise<string> => {
  const logPath = path.join(dirPath, "rustowl-error.log");
  const timestamp = new Date().toISOString();
  const logContent = `[${timestamp}] RustOwl Toolchain Setup Error:\n${error}\n\n`;

  try {
    // eslint-disable-next-line security/detect-non-literal-fs-filename
    await fs.appendFile(logPath, logContent);
    return logPath;
  } catch {
    return "";
  }
};

const showDetailedError = async (errorOutput: string, logPath: string) => {
  const errorLines = errorOutput.trim().split("\n");

  // Extract meaningful error summary from stderr (not just first line which may be logging)
  let summary = "Toolchain setup failed";

  // Look for actual error messages (lines starting with "error:")
  const errorLine = errorLines.find((line) => line.trim().startsWith("error:"));
  if (errorLine !== undefined) {
    summary = errorLine.trim();
  } else {
    // Look for other failure indicators
    const failureLine = errorLines.find(
      (line) =>
        line.toLowerCase().includes("failed") ||
        line.toLowerCase().includes("cannot") ||
        line.toLowerCase().includes("unable to") ||
        line.toLowerCase().includes("permission denied"),
    );
    if (failureLine !== undefined) {
      summary = failureLine.trim();
    }
  }

  // Create a more detailed error message
  let detailedMessage = `RustOwl toolchain setup failed:\n\n${summary}`;

  if (errorLines.length > 1) {
    detailedMessage += `\n\nAdditional details:\n${errorLines.slice(1, 3).join("\n")}`;
    if (errorLines.length > 3) {
      detailedMessage += "\n...";
    }
  }

  if (logPath !== "") {
    detailedMessage += `\n\nFull error details have been saved to:\n${logPath}`;
  }

  const selection = await vscode.window.showErrorMessage(
    detailedMessage,
    { modal: true },
    ...(logPath !== "" ? ["Open Log File", "Copy Log Path"] : []),
  );

  if (selection === "Open Log File" && logPath !== "") {
    try {
      const logUri = vscode.Uri.file(logPath);
      await vscode.window.showTextDocument(logUri);
    } catch (error) {
      const errorMessage =
        error instanceof Error ? error.message : String(error);
      vscode.window.showErrorMessage(
        `Failed to open log file: ${errorMessage}`,
      );
    }
  } else if (selection === "Copy Log Path" && logPath !== "") {
    await vscode.env.clipboard.writeText(logPath);
    vscode.window.showInformationMessage("Log path copied to clipboard");
  }
};
