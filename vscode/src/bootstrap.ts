import fs from "node:fs/promises";
import path from "node:path";
import { spawn, spawnSync } from "node:child_process";
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
  if (arch && platform) {
    return `${arch}-${platform}`;
  } else {
    return null;
  }
};

const exeExt = hostTuple()?.includes("windows") ? ".exe" : "";

export const downloadRustowl = async (basePath: string) => {
  const baseUrl = `https://github.com/cordx56/rustowl/releases/download/v${version}`;
  const host = hostTuple();
  if (host) {
    const owl = await fetch(`${baseUrl}/rustowl-${host}${exeExt}`);
    if (owl.status !== 200) {
      throw Error("RustOwl download error");
    }
    await fs.writeFile(
      `${basePath}/rustowl${exeExt}`,
      Buffer.from(await owl.arrayBuffer()),
      { flag: "w" },
    );
    await fs.chmod(`${basePath}/rustowl${exeExt}`, "755");
  } else {
    throw Error("unsupported architecture or platform");
  }
};

const exists = async (path: string) => {
  return fs
    .access(path)
    .then(() => true)
    .catch(() => false);
};
export const needUpdated = async (currentVersion: string) => {
  if (!currentVersion) {
    return true;
  }
  console.log(`current RustOwl version: ${currentVersion.trim()}`);
  console.log(`extension version: v${version}`);
  try {
    const semverParser = await import("semver-parser");
    const current = semverParser.parseSemVer(currentVersion.trim(), false);
    const self = semverParser.parseSemVer(version, false);
    if (
      current.major === self.major &&
      current.minor === self.minor &&
      current.patch === self.patch &&
      JSON.stringify(current.pre) === JSON.stringify(self.pre)
    ) {
      return false;
    } else {
      return true;
    }
  } catch (_e) {
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
    !rustowlCommand ||
    (await needUpdated(
      spawnSync(rustowlCommand, ["--version", "--quiet"]).stdout?.toString(),
    ))
  ) {
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
        } catch (e) {
          vscode.window.showErrorMessage(`${e}`);
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

          if (!rustowlCommand) {
            throw Error("failed to install RustOwl");
          }

          const installer = spawn(rustowlCommand, ["toolchain", "install"], {
            stdio: ["ignore", "ignore", "pipe"],
          });
          
          let stderrOutput = "";
          
          installer.stderr.addListener("data", (data) => {
            const dataStr = `${data}`;
            stderrOutput += dataStr;
            
            if (dataStr.includes("%")) {
              progress.report({
                message: "toolchain downloading",
                increment: 0.25, // downloads 4 toolchain components
              });
            }
          });
          
          return new Promise(async (resolve, reject) => {
            installer.addListener("exit", async (code) => {
              if (code === 0) {
                resolve(rustowlCommand);
              } else {
                const errorMessage = stderrOutput.trim() || `Process exited with code ${code}`;
                const logPath = await writeErrorLog(dirPath, errorMessage);
                await showDetailedError(errorMessage, logPath);
                reject(`toolchain setup failed (exit code ${code})`);
              }
            });
          });
        } catch (e) {
          vscode.window.showErrorMessage(`${e}`);
        }
        return null;
      },
    );
  }

  if (!rustowlCommand) {
    throw Error("failed to install RustOwl");
  }

  return rustowlCommand;
};

const writeErrorLog = async (dirPath: string, error: string): Promise<string> => {
  const logPath = path.join(dirPath, "rustowl-error.log");
  const timestamp = new Date().toISOString();
  const logContent = `[${timestamp}] RustOwl Toolchain Setup Error:\n${error}\n\n`;
  
  try {
    await fs.appendFile(logPath, logContent);
    return logPath;
  } catch (e) {
    console.error("Failed to write error log:", e);
    return "";
  }
};

const showDetailedError = async (errorOutput: string, logPath: string) => {
  const errorLines = errorOutput.trim().split('\n');
  const summary = errorLines.length > 0 ? errorLines[0] : "Unknown error occurred";
  
  // Create a more detailed error message
  let detailedMessage = `RustOwl toolchain setup failed:\n\n${summary}`;
  
  if (errorLines.length > 1) {
    detailedMessage += `\n\nAdditional details:\n${errorLines.slice(1, 3).join('\n')}`;
    if (errorLines.length > 3) {
      detailedMessage += "\n...";
    }
  }
  
  if (logPath) {
    detailedMessage += `\n\nFull error details have been saved to:\n${logPath}`;
  }
  
  const selection = await vscode.window.showErrorMessage(
    detailedMessage,
    { modal: true },
    ...(logPath ? ["Open Log File", "Copy Log Path"] : [])
  );
  
  if (selection === "Open Log File" && logPath) {
    try {
      const logUri = vscode.Uri.file(logPath);
      await vscode.window.showTextDocument(logUri);
    } catch (e) {
      vscode.window.showErrorMessage(`Failed to open log file: ${e}`);
    }
  } else if (selection === "Copy Log Path" && logPath) {
    await vscode.env.clipboard.writeText(logPath);
    vscode.window.showInformationMessage("Log path copied to clipboard");
  }
};
