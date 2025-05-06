<a name="unreleased"></a>
## [Unreleased]

### 🎨 Chores
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
- dont use tar, use Compress-Archive instead
- change compress script to use sysroot dir ([#125](https://github.com/cordx56/rustowl/issues/125))
- add release on top of cp
- use target name in cp command
- pr permission for changelog
- **aur:** add cd lines as it errors
- **binstall:** use archives instead of binaries
- **changelogen:** only add normal releases, not alpha and others
- **ci:** use powershell in windoes ci
- **reqwest:** dont depend on openssl-sys, use rustls for lower system deps
- **windows:** unzip

### 🚀 Features
- add a pr template
- add a code of conduct and security file
- aur packages ([#105](https://github.com/cordx56/rustowl/issues/105))
- aur packages
- automatic updates with dependabot
- use zip instead of tar in windows
- auto release changelogs, changelog generation
- **archive:** implement zipping for windows


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

[Unreleased]: https://github.com/cordx56/rustowl/compare/v0.3.0...HEAD
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
