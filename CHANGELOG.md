<a name="unreleased"></a>
## [Unreleased]

### 🐞 Bug Fixes
- pr permission for changelog


<a name="v0.3.1-alpha.1"></a>
## [v0.3.1-alpha.1] - 2025-05-04
### 🎨 Chores
- add comments to cargo.toml on next release changes
- added build time env var description
- update changelog
- update changelog
- update changelog

### 🐞 Bug Fixes
- **binstall:** use archives instead of binaries
- **reqwest:** dont depend on openssl-sys, use rustls for lower system deps

### 🚀 Features
- use zip instead of tar in windows
- auto release changelogs, changelog generation
- **archive:** implement zipping for windows

### Pull Requests
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

<a name="v0.3.0-alpha.2"></a>
## [v0.3.0-alpha.2] - 2025-04-30
### Pull Requests
- Merge pull request [#88](https://github.com/cordx56/rustowl/issues/88) from yasuo-ozu/fix_build_canonical


<a name="v0.3.0-alpha.1"></a>
## [v0.3.0-alpha.1] - 2025-04-27

<a name="v0.2.3-alpha.1"></a>
## [v0.2.3-alpha.1] - 2025-04-25

<a name="v0.2.3pre"></a>
## [v0.2.3pre] - 2025-04-25
### 🚀 Features
- shell completions and man pages

### Reverts
- test workflow

### Pull Requests
- Merge pull request [#85](https://github.com/cordx56/rustowl/issues/85) from MuntasirSZN/main
- Merge pull request [#80](https://github.com/cordx56/rustowl/issues/80) from siketyan/ci/more-platform


<a name="v0.2.2"></a>
## [v0.2.2] - 2025-04-18

<a name="v0.2.2pre2"></a>
## [v0.2.2pre2] - 2025-04-18

<a name="v0.2.2pre"></a>
## [v0.2.2pre] - 2025-04-18
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


<a name="v0.0.3pre"></a>
## [v0.0.3pre] - 2025-01-26
### Pull Requests
- Merge pull request [#5](https://github.com/cordx56/rustowl/issues/5) from mu001999-contrib/main


<a name="v0.0.2"></a>
## [v0.0.2] - 2025-01-23

<a name="v0.0.2pre"></a>
## [v0.0.2pre] - 2025-01-23

<a name="v0.0.1"></a>
## [v0.0.1] - 2024-11-13

<a name="vpre"></a>
## vpre - 2024-11-11

[Unreleased]: https://github.com/cordx56/rustowl/compare/v0.3.1-alpha.1...HEAD
[v0.3.1-alpha.1]: https://github.com/cordx56/rustowl/compare/v0.3.0...v0.3.1-alpha.1
[v0.3.0]: https://github.com/cordx56/rustowl/compare/v0.3.0-alpha.2...v0.3.0
[v0.3.0-alpha.2]: https://github.com/cordx56/rustowl/compare/v0.3.0-alpha.1...v0.3.0-alpha.2
[v0.3.0-alpha.1]: https://github.com/cordx56/rustowl/compare/v0.2.3-alpha.1...v0.3.0-alpha.1
[v0.2.3-alpha.1]: https://github.com/cordx56/rustowl/compare/v0.2.3pre...v0.2.3-alpha.1
[v0.2.3pre]: https://github.com/cordx56/rustowl/compare/v0.2.2...v0.2.3pre
[v0.2.2]: https://github.com/cordx56/rustowl/compare/v0.2.2pre2...v0.2.2
[v0.2.2pre2]: https://github.com/cordx56/rustowl/compare/v0.2.2pre...v0.2.2pre2
[v0.2.2pre]: https://github.com/cordx56/rustowl/compare/v0.2.1...v0.2.2pre
[v0.2.1]: https://github.com/cordx56/rustowl/compare/v0.2.0...v0.2.1
[v0.2.0]: https://github.com/cordx56/rustowl/compare/v0.1.4...v0.2.0
[v0.1.4]: https://github.com/cordx56/rustowl/compare/v0.1.3...v0.1.4
[v0.1.3]: https://github.com/cordx56/rustowl/compare/v0.1.2...v0.1.3
[v0.1.2]: https://github.com/cordx56/rustowl/compare/v0.1.1...v0.1.2
[v0.1.1]: https://github.com/cordx56/rustowl/compare/v0.1.0...v0.1.1
[v0.1.0]: https://github.com/cordx56/rustowl/compare/v0.0.5...v0.1.0
[v0.0.5]: https://github.com/cordx56/rustowl/compare/v0.0.4...v0.0.5
[v0.0.4]: https://github.com/cordx56/rustowl/compare/v0.0.3...v0.0.4
[v0.0.3]: https://github.com/cordx56/rustowl/compare/v0.0.3pre...v0.0.3
[v0.0.3pre]: https://github.com/cordx56/rustowl/compare/v0.0.2...v0.0.3pre
[v0.0.2]: https://github.com/cordx56/rustowl/compare/v0.0.2pre...v0.0.2
[v0.0.2pre]: https://github.com/cordx56/rustowl/compare/v0.0.1...v0.0.2pre
[v0.0.1]: https://github.com/cordx56/rustowl/compare/vpre...v0.0.1
