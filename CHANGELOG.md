<a name="unreleased"></a>
## [Unreleased]

### ♻️ Code Refactoring
- **alloc:** move to mimalloc because of jemalloc archive ([#230](https://github.com/cordx56/rustowl/issues/230))
- **runtime:** refactor the runtime to use more suitable stack size for generic machine, and to use amount of cores counting the existing cores on the machine and using half to not make or get stuck

### 🎨 Chores
- update changelog
- update changelog ([#211](https://github.com/cordx56/rustowl/issues/211))
- add performance test to the repo ([#219](https://github.com/cordx56/rustowl/issues/219))
- add ovsx to release script and docs about extension ([#266](https://github.com/cordx56/rustowl/issues/266))
- update lockfiles and deps ([#267](https://github.com/cordx56/rustowl/issues/267))
- **README:** Add Discord server link ([#239](https://github.com/cordx56/rustowl/issues/239))
- **aur:** bump rustup toolchain version ([#177](https://github.com/cordx56/rustowl/issues/177))
- **dependabot:** set interval weekly and use grouping ([#310](https://github.com/cordx56/rustowl/issues/310))
- **dependabot:** fix dependabot error ([#311](https://github.com/cordx56/rustowl/issues/311))
- **dependabot:** ignore [@types](https://github.com/types)/vscode version update ([#314](https://github.com/cordx56/rustowl/issues/314))
- **docs:** add badges to README ([#268](https://github.com/cordx56/rustowl/issues/268))
- **nvim-tests:** add two newlines in minimal init ([#307](https://github.com/cordx56/rustowl/issues/307))
- **rustc:** bump rustc to 1.88.0 in neovim ci ([#315](https://github.com/cordx56/rustowl/issues/315))

### 🐞 Bug Fixes
- call vscode bootstrap only when RustOwl is downloaded ([#309](https://github.com/cordx56/rustowl/issues/309))
- **alloc:** properly setup mimalloc
- **benchmarks:** increase measurement and warm-up time for benchmark tests
- **bencmarks:** fix benchmarks script to calculate result correctly, and increased the amount of iteration for more precise results
- **deps:** update tar dependency to version 0.4
- **rustc:** new 1.88.0, bump version in ci ([#300](https://github.com/cordx56/rustowl/issues/300))

### 🚀 Features
- update to rustc 1.88.0
- enhance CLI command handling with options for all targets and features ([#225](https://github.com/cordx56/rustowl/issues/225))
- Add security and memory safety testing workflow ([#234](https://github.com/cordx56/rustowl/issues/234))
- consolidate and enhance CI workflows by replacing check.yaml with checks.yml and adding a development checks script ([#233](https://github.com/cordx56/rustowl/issues/233))
- winget package ([#178](https://github.com/cordx56/rustowl/issues/178))
- **perf-tests:** Memory fixes ([#226](https://github.com/cordx56/rustowl/issues/226))


<a name="v0.3.4"></a>
## [v0.3.4] - 2025-05-20
### 🎨 Chores
- urgent release v0.3.4, fixes wrong visualization
- update changelog ([#170](https://github.com/cordx56/rustowl/issues/170))
- update changelog ([#168](https://github.com/cordx56/rustowl/issues/168))

### 🐞 Bug Fixes
- **lsp-core:** fix actual lifetime range visualization for `Drop` variable.


<a name="v0.3.3"></a>
## [v0.3.3] - 2025-05-17
### ♻️ Code Refactoring
- split build action from release action

### 🎨 Chores
- fix pre-release if statement
- fix release case
- regex in bash should not quoted
- automate cargo publish
- vsce auto publish
- use official toolchain
- Rewrite CLI using Derive API ([#153](https://github.com/cordx56/rustowl/issues/153))
- update changelog ([#154](https://github.com/cordx56/rustowl/issues/154))
- **cli:** Add help messages to options ([#159](https://github.com/cordx56/rustowl/issues/159))

### 🐞 Bug Fixes
- support CRLF
- GitHub Actions typo
- use native ca certs by enabling native roots feature of reqwest ([#162](https://github.com/cordx56/rustowl/issues/162))
- **pkgbuild:** use rustup instead of cargo ([#156](https://github.com/cordx56/rustowl/issues/156))

### 🚀 Features
- update rustc to 1.87.0


<a name="v0.3.2"></a>
## [v0.3.2] - 2025-05-09
### 🐞 Bug Fixes
- support gsed (macOS)
- version.sh removed and use ./scripts/bump.sh
- specify pkg-fmt for binstall
- restore current newest version

### 🚀 Features
- v0.3.2 release
- support RUSTOWL_SYSROOT_DIRS
- add a bump.sh for bumping ([#148](https://github.com/cordx56/rustowl/issues/148))
- documented binstall method
- support single .rs file analyze and VS Code download progress

### Pull Requests
- Merge pull request [#146](https://github.com/cordx56/rustowl/issues/146) from cordx56/dependabot/npm_and_yarn/vscode/types/node-22.15.14


<a name="v0.3.1"></a>
## [v0.3.1] - 2025-05-07
### 🎨 Chores
- Release v0.3.1
- Don't check every main push
- update changelog
- update changelog
- update changelog
- update changelog ([#116](https://github.com/cordx56/rustowl/issues/116))
- update changelog ([#112](https://github.com/cordx56/rustowl/issues/112))
- update changelog ([#104](https://github.com/cordx56/rustowl/issues/104))
- add comments to cargo.toml on next release changes
- added build time env var description
- update changelog
- update changelog
- update changelog

### 🐞 Bug Fixes
- email
- use target name in cp command
- VS Code version check returns null
- pr permission for changelog
- dont use tar, use Compress-Archive instead
- check before release and profile dir
- add release on top of cp
- change compress script to use sysroot dir ([#125](https://github.com/cordx56/rustowl/issues/125))
- arm Windows build
- avoid failure to find sysroot
- rustowlc ext for Windows
- **aur:** add cd lines as it errors
- **binstall:** use archives instead of binaries
- **changelogen:** only add normal releases, not alpha and others
- **ci:** use powershell in windoes ci
- **reqwest:** dont depend on openssl-sys, use rustls for lower system deps
- **windows:** unzip

### 🚀 Features
- better-release-notes
- support multiple fallbacks
- remove redundant rustc_driver
- RustOwl version check for VS Code extension
- add a pr template
- add a code of conduct and security file
- aur packages ([#105](https://github.com/cordx56/rustowl/issues/105))
- aur packages
- automatic updates with dependabot
- use zip instead of tar in windows
- auto release changelogs, changelog generation
- **archive:** implement zipping for windows

### Reverts
- move CONTRIBUTING.md

### Pull Requests
- Merge pull request [#142](https://github.com/cordx56/rustowl/issues/142) from cordx56/feat/better-release-notes
- Merge pull request [#140](https://github.com/cordx56/rustowl/issues/140) from MuntasirSZN/fix/changelogen
- Merge pull request [#132](https://github.com/cordx56/rustowl/issues/132) from cordx56/create-pull-request/autogenerate-changelog
- Merge pull request [#131](https://github.com/cordx56/rustowl/issues/131) from MuntasirSZN/fix/windows-unzip
- Merge pull request [#130](https://github.com/cordx56/rustowl/issues/130) from MuntasirSZN/fix/pkgbuild-git
- Merge pull request [#129](https://github.com/cordx56/rustowl/issues/129) from MuntasirSZN/feat/community-standards
- Merge pull request [#128](https://github.com/cordx56/rustowl/issues/128) from MuntasirSZN/main
- Merge pull request [#126](https://github.com/cordx56/rustowl/issues/126) from cordx56/create-pull-request/autogenerate-changelog
- Merge pull request [#124](https://github.com/cordx56/rustowl/issues/124) from MuntasirSZN/main
- Merge pull request [#123](https://github.com/cordx56/rustowl/issues/123) from MuntasirSZN/main
- Merge pull request [#115](https://github.com/cordx56/rustowl/issues/115) from MuntasirSZN/main
- Merge pull request [#114](https://github.com/cordx56/rustowl/issues/114) from MuntasirSZN/main
- Merge pull request [#113](https://github.com/cordx56/rustowl/issues/113) from MuntasirSZN/main
- Merge pull request [#111](https://github.com/cordx56/rustowl/issues/111) from MuntasirSZN/fix/archive-ci
- Merge pull request [#103](https://github.com/cordx56/rustowl/issues/103) from MuntasirSZN/feat/dependabot
- Merge pull request [#101](https://github.com/cordx56/rustowl/issues/101) from MuntasirSZN/feat/zig-linker
- Merge pull request [#96](https://github.com/cordx56/rustowl/issues/96) from MuntasirSZN/main
- Merge pull request [#97](https://github.com/cordx56/rustowl/issues/97) from MuntasirSZN/fix/binstall
- Merge pull request [#99](https://github.com/cordx56/rustowl/issues/99) from Alex-Grimes/enhancment/78_Add-highlight-style-config-option
- Merge pull request [#98](https://github.com/cordx56/rustowl/issues/98) from cordx56/fix/ci-changelogen
- Merge pull request [#92](https://github.com/cordx56/rustowl/issues/92) from MuntasirSZN/main
- Merge pull request [#94](https://github.com/cordx56/rustowl/issues/94) from mrcjkb/mj/push-mpkursvmrosw
- Merge pull request [#91](https://github.com/cordx56/rustowl/issues/91) from MuntasirSZN/main


<a name="v0.3.0"></a>
## [v0.3.0] - 2025-04-30
### 🚀 Features
- shell completions and man pages

### Reverts
- test workflow

### Pull Requests
- Merge pull request [#88](https://github.com/cordx56/rustowl/issues/88) from yasuo-ozu/fix_build_canonical
- Merge pull request [#85](https://github.com/cordx56/rustowl/issues/85) from MuntasirSZN/main
- Merge pull request [#80](https://github.com/cordx56/rustowl/issues/80) from siketyan/ci/more-platform


<a name="v0.2.2"></a>
## [v0.2.2] - 2025-04-18
### ♻️ Code Refactoring
- streamline toolchain detection and correct cargo path

### 🚀 Features
- **toolchain:** add support for RUSTOWL_TOOLCHAIN_DIR to bypass rustup

### Pull Requests
- Merge pull request [#77](https://github.com/cordx56/rustowl/issues/77) from xBLACKICEx/flexible-toolchain


<a name="v0.2.1"></a>
## [v0.2.1] - 2025-04-15

<a name="v0.2.0"></a>
## [v0.2.0] - 2025-04-09
### ♻️ Code Refactoring
- add prefix to functions with commonly used names

### 🎨 Chores
- add require lsp
- remove calling `enable-rustowlsp-cursor`
- add `defgroup`
- add `provide`
- Migrate to Rust 2024

### 🐞 Bug Fixes
- package-requires

### Reverts
- messsage type
- neovim plugin function
- update install manual

### Pull Requests
- Merge pull request [#72](https://github.com/cordx56/rustowl/issues/72) from mawkler/neovim-version
- Merge pull request [#69](https://github.com/cordx56/rustowl/issues/69) from cordx56/feat/elim-rustup-call
- Merge pull request [#48](https://github.com/cordx56/rustowl/issues/48) from mawkler/lua-api
- Merge pull request [#62](https://github.com/cordx56/rustowl/issues/62) from Kyure-A/main
- Merge pull request [#61](https://github.com/cordx56/rustowl/issues/61) from AIDIGIT/nvim-hl-priorities
- Merge pull request [#60](https://github.com/cordx56/rustowl/issues/60) from AIDIGIT/main
- Merge pull request [#55](https://github.com/cordx56/rustowl/issues/55) from sorairolake/migrate-to-2024-edition


<a name="v0.1.4"></a>
## [v0.1.4] - 2025-02-22
### ♻️ Code Refactoring
- simplify HashMap insertion by using entry API

### Pull Requests
- Merge pull request [#54](https://github.com/cordx56/rustowl/issues/54) from uhobnil/main


<a name="v0.1.3"></a>
## [v0.1.3] - 2025-02-20
### 🎨 Chores
- remove duplicate code

### 🐞 Bug Fixes
- install the newest version

### Pull Requests
- Merge pull request [#53](https://github.com/cordx56/rustowl/issues/53) from uhobnil/main
- Merge pull request [#47](https://github.com/cordx56/rustowl/issues/47) from robin-thoene/fix/update-install-script


<a name="v0.1.2"></a>
## [v0.1.2] - 2025-02-19
### 🎨 Chores
- add the description for duplication
- add config.yaml
- add issue templae for feature requesting
- add labels to bug_report
- add issue templae for bug reporing

### 🐞 Bug Fixes
- s/enhancement/bug/
- update the introduction
- correct label
- remove redundant textarea
- update the information
- update the file extension
- s/rustowl/RustOwl/
- kill process when the client/server is dead

### Pull Requests
- Merge pull request [#35](https://github.com/cordx56/rustowl/issues/35) from chansuke/chore/add-issue-template
- Merge pull request [#42](https://github.com/cordx56/rustowl/issues/42) from uhobnil/main
- Merge pull request [#34](https://github.com/cordx56/rustowl/issues/34) from mtshiba/main
- Merge pull request [#26](https://github.com/cordx56/rustowl/issues/26) from Toyo-tez/main
- Merge pull request [#11](https://github.com/cordx56/rustowl/issues/11) from wx257osn2/clippy
- Merge pull request [#24](https://github.com/cordx56/rustowl/issues/24) from mawkler/main


<a name="v0.1.1"></a>
## [v0.1.1] - 2025-02-07

<a name="v0.1.0"></a>
## [v0.1.0] - 2025-02-05
### Pull Requests
- Merge pull request [#2](https://github.com/cordx56/rustowl/issues/2) from wx257osn2/support-windows


<a name="v0.0.5"></a>
## [v0.0.5] - 2025-02-02

<a name="v0.0.4"></a>
## [v0.0.4] - 2025-01-31

<a name="v0.0.3"></a>
## [v0.0.3] - 2025-01-30
### Pull Requests
- Merge pull request [#6](https://github.com/cordx56/rustowl/issues/6) from Jayllyz/build/enable-lto-codegen
- Merge pull request [#5](https://github.com/cordx56/rustowl/issues/5) from mu001999-contrib/main


<a name="v0.0.2"></a>
## [v0.0.2] - 2025-01-23

<a name="v0.0.1"></a>
## v0.0.1 - 2024-11-13

[Unreleased]: https://github.com/cordx56/rustowl/compare/v0.3.4...HEAD
[v0.3.4]: https://github.com/cordx56/rustowl/compare/v0.3.3...v0.3.4
[v0.3.3]: https://github.com/cordx56/rustowl/compare/v0.3.2...v0.3.3
[v0.3.2]: https://github.com/cordx56/rustowl/compare/v0.3.1...v0.3.2
[v0.3.1]: https://github.com/cordx56/rustowl/compare/v0.3.0...v0.3.1
[v0.3.0]: https://github.com/cordx56/rustowl/compare/v0.2.2...v0.3.0
[v0.2.2]: https://github.com/cordx56/rustowl/compare/v0.2.1...v0.2.2
[v0.2.1]: https://github.com/cordx56/rustowl/compare/v0.2.0...v0.2.1
[v0.2.0]: https://github.com/cordx56/rustowl/compare/v0.1.4...v0.2.0
[v0.1.4]: https://github.com/cordx56/rustowl/compare/v0.1.3...v0.1.4
[v0.1.3]: https://github.com/cordx56/rustowl/compare/v0.1.2...v0.1.3
[v0.1.2]: https://github.com/cordx56/rustowl/compare/v0.1.1...v0.1.2
[v0.1.1]: https://github.com/cordx56/rustowl/compare/v0.1.0...v0.1.1
[v0.1.0]: https://github.com/cordx56/rustowl/compare/v0.0.5...v0.1.0
[v0.0.5]: https://github.com/cordx56/rustowl/compare/v0.0.4...v0.0.5
[v0.0.4]: https://github.com/cordx56/rustowl/compare/v0.0.3...v0.0.4
[v0.0.3]: https://github.com/cordx56/rustowl/compare/v0.0.2...v0.0.3
[v0.0.2]: https://github.com/cordx56/rustowl/compare/v0.0.1...v0.0.2
