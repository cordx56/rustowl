<a name="v0.4.0"></a>

## [v0.4.0](https://github.com/MuntasirSZN/getquotes/compare/v1.0.0-rc.1...v0.4.0) (2026-05-28)

### ✨ Features

- **performance:** add rustc-threads option [#491]
- implement CLI underline show command [#509]
- support Rust 1.94.0 [#548]
- add installer script [#567]
- toolchain installation progress bar [#569]
- new lifetime visualization algorithm based on CFG analysis [#595]
- implement new precise lifetime visualization plugin for editors [#600]
- support Rust 1.95.0 [#613]


### 🐞 Bug Fixes

- terminate range bug [#508]
- use macos-15-intel in build actions [#511]
- select boxes are is considered as task list [#520]
- **windows:** add turbofish arguments in visualize to make trait solver happy [#518]
- lto in windows, rustc allocator [#519]
- **ci:** eval echo in new toolchain script [#579]
- support more MIR statement (e.g. aggregate rval) [#612]
- **ci:** release workflow ref [#620]
- **ci:** release CI [#621]


### 📝 Code Refactoring

- **core-analyzer:** refactor and change must-lives calculation algorithm [#407]
- vscode [#480]


<a name="v1.0.0-rc.1"></a>

## [v1.0.0-rc.1](https://github.com/MuntasirSZN/getquotes/compare/v0.3.4...v1.0.0-rc.1) (2025-08-19)

### ✨ Features

- winget package [#178]
- **perf-tests:** Memory fixes [#226]
- consolidate and enhance CI workflows by replacing check.yaml with checks.yml and adding a development checks script [#233]
- Add security and memory safety testing workflow [#234]
- enhance CLI command handling with options for all targets and features [#225]
- update to rustc 1.88.0
- **toolchain:** new toolchain installer [#320]
- **toolchain:** install cargo on toolchain installation [#334]
- **build:** add consistent build script [#336]
- **build:** update build process and use rustowl archive [#340]
- add vscode tests, migrate to zod v4, format code [#330]
- **cache:** Implement cache mechanism [#361]
- add docker image
- bump rustc to 1.89.0
- **lsp:** Improve VS Code experience
- use rustowl/analyze in nvim
- use rustowl/analyze in emacs
- **neovim:** prevent analyzing when disabled
- commitlint in script
- **config:** add configurable highlight colors for neovim plugin
- **neovim:** ensure color config falls back to defaults using deep merge


### 🐞 Bug Fixes

- **deps:** update tar dependency to version 0.4
- **benchmarks:** increase measurement and warm-up time for benchmark tests
- **bencmarks:** fix benchmarks script to calculate result correctly, and increased the amount of iteration for more precise results
- **alloc:** properly setup mimalloc
- call vscode bootstrap only when RustOwl is downloaded [#309]
- **rustc:** new 1.88.0, bump version in ci [#300]
- visualize inside async function [#327]
- visualize wrong range, caused by byte check [#325]
- **alloc:** move to jemalloc as rustc does [#335]
- **windows:** zip now contains a top level dir [#341]
- add skip installing RustOwl toolchain option on toolchain installation [#342]
- add mkdir command
- run toolchain install BEFORE cache see https://github.com/Swatinem/rust-cache
- cache only on main (we are reaching 10gb)
- emacs rustowl/analyze
- emacs fixed (by GPT-5)
- **docs:** fix Emacs document
- eslint:
- yarn
- yarn
- yarn
- revert
- commitlint
- analysis on check failure [#379]
- **lsp-backend:** set target dir to [target]/owl
- **lsp-backend:** kill unnecessary processes
- no need to pass frozen-lockfile in ci
- block scalar in action-setup
- lockfile:
- **core-analyzer:** fix analyze locations and refactor
- **core-analyzer:** fix locked block_on
- **analyzer:** fix async blocking and panic
- **neovim:** set inclusive false


### 📝 Code Refactoring

- **alloc:** move to mimalloc because of jemalloc archive [#230]
- **runtime:** refactor the runtime to use more suitable stack size for generic machine, and to use amount of cores counting the existing cores on the machine and using half to not make or get stuck
- **reqwest:** move to aws_lc instead of ring [#338]
- **analyzer:** analyzer refactoring and performance improvement [#363]
- **lsp:** separate analyzers [#367]
- **lsp-backend:** fix and unify cargo command exec code
- **docker:** simplify Docker file
- **ci:** check build on main push [#386]
- **lsp-backend:** fix path problem


<a name="v0.3.4"></a>

## [v0.3.4](https://github.com/MuntasirSZN/getquotes/compare/v0.3.3...v0.3.4) (2025-05-20)

### 🐞 Bug Fixes

- **lsp-core:** fix actual lifetime range visualization for `Drop` variable.


<a name="v0.3.3"></a>

## [v0.3.3](https://github.com/MuntasirSZN/getquotes/compare/v0.3.3-rc.2...v0.3.3) (2025-05-17)

### 🐞 Bug Fixes

- support CRLF


### 📝 Code Refactoring

- split build action from release action


<a name="v0.3.3-rc.2"></a>

## [v0.3.3-rc.2](https://github.com/MuntasirSZN/getquotes/compare/v0.3.3-rc.1...v0.3.3-rc.2) (2025-05-16)

### 🐞 Bug Fixes

- GitHub Actions typo


<a name="v0.3.3-rc.1"></a>

## [v0.3.3-rc.1](https://github.com/MuntasirSZN/getquotes/compare/v0.3.2...v0.3.3-rc.1) (2025-05-16)

### ✨ Features

- update rustc to 1.87.0


### 🐞 Bug Fixes

- use native ca certs by enabling native roots feature of reqwest [#162]
- **pkgbuild:** use rustup instead of cargo [#156]
- libLLVM
- vscode version and release script


<a name="v0.3.2"></a>

## [v0.3.2](https://github.com/MuntasirSZN/getquotes/compare/v0.3.1...v0.3.2) (2025-05-09)

### ✨ Features

- support single .rs file analyze and VS Code download progress
- documented binstall method
- add a bump.sh for bumping [#148]
- support RUSTOWL_SYSROOT_DIRS
- v0.3.2 release


### 🐞 Bug Fixes

- restore current newest version
- specify pkg-fmt for binstall
- version.sh removed and use ./scripts/bump.sh
- support gsed (macOS)


<a name="v0.3.1"></a>

## [v0.3.1](https://github.com/MuntasirSZN/getquotes/compare/v0.3.1-rc.1...v0.3.1) (2025-05-06)

### 🐞 Bug Fixes

- VS Code version check returns null


<a name="v0.3.1-rc.1"></a>

## [v0.3.1-rc.1](https://github.com/MuntasirSZN/getquotes/compare/v0.3.1-alpha.3...v0.3.1-rc.1) (2025-05-06)

### ✨ Features

- RustOwl version check for VS Code extension
- remove redundant rustc_driver
- support multiple fallbacks
- better-release-notes


### 🐞 Bug Fixes

- email
- **aur:** add cd lines as it errors
- **windows:** unzip
- avoid failure to find sysroot
- **changelogen:** only add normal releases, not alpha and others
- arm Windows build
- check before release and profile dir


<a name="v0.3.1-alpha.3"></a>

## [v0.3.1-alpha.3](https://github.com/MuntasirSZN/getquotes/compare/v0.3.1-alpha.2...v0.3.1-alpha.3) (2025-05-06)

### 🐞 Bug Fixes

- rustowlc ext for Windows


<a name="v0.3.1-alpha.2"></a>

## [v0.3.1-alpha.2](https://github.com/MuntasirSZN/getquotes/compare/v0.3.1-alpha.1...v0.3.1-alpha.2) (2025-05-06)

### ✨ Features

- add a code of conduct and security file
- add a pr template


### 🐞 Bug Fixes

- dont use tar, use Compress-Archive instead


<a name="v0.3.1-alpha.1"></a>

## [v0.3.1-alpha.1](https://github.com/MuntasirSZN/getquotes/compare/v0.3.0...v0.3.1-alpha.1) (2025-05-05)

### ✨ Features

- auto release changelogs, changelog generation
- use zip instead of tar in windows
- **archive:** implement zipping for windows
- automatic updates with dependabot
- aur packages
- aur packages [#105]


### 🐞 Bug Fixes

- **reqwest:** dont depend on openssl-sys, use rustls for lower system deps
- **binstall:** use archives instead of binaries
- pr permission for changelog
- use target name in cp command
- add release on top of cp
- **ci:** use powershell in windoes ci
- change compress script to use sysroot dir [#125]


<a name="v0.2.3pre"></a>

## [v0.2.3pre](https://github.com/MuntasirSZN/getquotes/compare/v0.2.2...v0.2.3pre) (2025-04-25)

### ✨ Features

- shell completions and man pages


<a name="v0.2.2pre"></a>

## [v0.2.2pre](https://github.com/MuntasirSZN/getquotes/compare/v0.2.1...v0.2.2pre) (2025-04-18)

### ✨ Features

- **toolchain:** add support for RUSTOWL_TOOLCHAIN_DIR to bypass rustup


### 📝 Code Refactoring

- streamline toolchain detection and correct cargo path


<a name="v0.2.0"></a>

## [v0.2.0](https://github.com/MuntasirSZN/getquotes/compare/v0.1.4...v0.2.0) (2025-04-08)

### 🐞 Bug Fixes

- package-requires


### 📝 Code Refactoring

- add prefix to functions with commonly used names


<a name="v0.1.4"></a>

## [v0.1.4](https://github.com/MuntasirSZN/getquotes/compare/v0.1.3...v0.1.4) (2025-02-22)

### 📝 Code Refactoring

- simplify HashMap insertion by using entry API


<a name="v0.1.3"></a>

## [v0.1.3](https://github.com/MuntasirSZN/getquotes/compare/v0.1.2...v0.1.3) (2025-02-20)

### 🐞 Bug Fixes

- install the newest version


<a name="v0.1.2"></a>

## [v0.1.2](https://github.com/MuntasirSZN/getquotes/compare/v0.1.1...v0.1.2) (2025-02-19)

### 🐞 Bug Fixes

- kill process when the client/server is dead
- s/rustowl/RustOwl/
- update the file extension
- update the information
- remove redundant textarea
- correct label
- update the introduction
- s/enhancement/bug/


<a name="vpre"></a>

## vpre (2024-11-11)

