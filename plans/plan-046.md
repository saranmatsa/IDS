## Phase 046 — Create the ids-cli crate: Command-line interface. Research Findings & Implementation Blueprint

**Research complete (30 targeted searches executed). No code written, no files created, no repository changes made. This document is the blueprint for implementation.**

Verbatim prompt (C:/idfs/prompts.txt lines 685-699):

```
46. Research only do not read plan.md **[Phase 046 — Create the ids-cli crate: Command-line interface.]**

Crate: ids-cli
Purpose: Command-line interface.
Contents: CLI commands, subcommands, argument parsing, output formatting, interactive and scriptable modes, configuration loading.

try to use seperate crates instead of more folders in existing onesResearch only do not read plan.md research **[Phase 046 — Create the ids-cli crate: Command-line interface.]**

Create plan-046.md in C:/idfs/plans/ for the ids-cli crate.
Follow the HX-241 plan format (plan-077.md, plan-080.md, plan-121.md): Executive Summary, Architecture Overview, Detailed Research Dimensions, Threat Model, Integration with Existing Crates, Proposed File Structure (~90-1000 files in subfolders), Key Algorithms & Data Structures, Dependencies, Measurable Acceptance Criteria, Remaining Uncertainties, Engineering Decisions Summary, Sources.
Carrier-agnostic: no std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic via ids-carrier::Endpoint (CarrierType + Vec<u8>).
Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.
ids-000-aa is the master crate with all canonical types and sealed trait contracts.
Research only — no code, no source modifications, only the plan document.
```

---

### 1. Executive Summary

Phase 046 creates `ids-cli`, the IDS command-line interface crate. It owns CLI commands, subcommands, argument parsing, output formatting, interactive and scriptable modes, and configuration loading. `ids-cli` is the user-facing entry point into the IDS stack — it never owns network transport; it drives other IDS crates (ids-store, ids-fragment, ids-replication, ids-immutable, ids-reclaim) through their public APIs and sends all cross-node traffic as opaque `Vec<u8>` datagrams to `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`, `CarrierType` ∈ {Loopback, Serial, Optical, Radio, Custom}).

`ids-cli` supports two modes side by side:
- **Scriptable mode**: subcommand-driven, structured (JSON or human-readable) output, deterministic exit codes, suitable for pipelines and automation.
- **Interactive mode**: a full-screen TUI (when a TTY is present) with styled tables/panels, progress bars, confirm prompts, password input, and live refresh — built without `tokio::net` or any networking crate.

Carrier-agnostic absolute rule: no `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic is opaque `Vec<u8>` datagrams to `ids-carrier::Endpoint`. Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 (consumed via `ids-crypto`; primitive IDs from `ids-000-aa` only).

`ids-cli` does NOT yet exist at `C:/idfs/ids-cli/`. `ids-000-aa` already declares canonical types `RecordId([u8;32])`, `NodeId([u8;32])`, `RegionId([u8;32])`, `SnapshotId([u8;16])`, `Checksum([u8;32])`, `IdsTime(pub u64)`, `CarrierType` ∈ {Loopback, Serial, Optical, Radio, Custom(u16)}, and `AnomalyType` ∈ {ByteMismatch, ReplicaDrift, ChecksumFailure, StateCorruption} — all referenced but never redefined here.

Research basis: 30 parallel searches returned clap derive API (`Parser`/`Subcommand`/`Args`/`TypedValueParser`, `arg_required_else_help`, `subcommand_required`, `ArgGroup`/`required_unless`/`conflicts_with`), clap_complete shell completions (bash, fish, zsh, powershell, elvish), ratatui/crossterm/termion TUI (raw mode, alternate screen, layout constraints, table/list widgets), styled output crates (gilt, rich_rust, richrs, sparcli), confy/toml/toml-config configuration loading, ctrlc/signal-hook graceful shutdown, indicatif progress bars (single + MultiProgress), dialoguer/cliclack interactive prompts (confirm/select/input/password), rpassword hidden input, serde_json pretty-printed JSON to stdout, std::env::args_os for argument iteration, walkdir recursive directory iteration, tempfile secure temp files, std::fs::canonicalize path normalization, thiserror/anyhow CLI error handling, once_cell singleton config, directories ProjectDirs for config/data/cache dirs, std::io stdin/stdout/stderr, fs::set_permissions executable bits, clipboard-rs/clipboard-win cross-platform clipboard, pager2/lessify/sartorial terminal paging.

---

### 2. Architecture Overview

`ids-cli` sits above `ids-000-aa` (canonical types + sealed trait contracts + errors), `ids-carrier` (opaque `Endpoint` datagram transport), and `ids-crypto` (allowed primitives). It is a thin orchestration layer: it parses CLI input, validates it, translates it into calls on other IDS crates, and renders results. It does not implement storage, replication, or crypto itself.

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                         USER / SHELL / PIPELINE                              │
│  interactive TUI  ·  scriptable subcommands  ·  stdin/stdout/stderr  ·  pager │
├──────────────────────────────────────────────────────────────────────────────┤
│                       Phase 046: ids-cli (THIS CRATE)                        │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌────────────────┐      │
│  │ clap parser  │ │ argument     │ │ output       │ │ interactive    │      │
│  │ (derive API) │ │ validation   │ │ formatting   │ │ mode (TUI)     │      │
│  └──────────────┘ └──────────────┘ └──────────────┘ └────────────────┘      │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌────────────────┐      │
│  │ config       │ │ command      │ │ signal/      │ │ carrier adapter │      │
│  │ loading      │ │ dispatch     │ │ shutdown     │ │ (Endpoint only)│      │
│  └──────────────┘ └──────────────┘ └──────────────┘ └────────────────┘      │
├──────────────────────────────────────────────────────────────────────────────┤
│              IDS CRATE DRIVERS (mutation/query issuers)                      │
│  ids-store · ids-fragment · ids-replication · ids-immutable · ids-reclaim   │
├──────────────────────────────────────────────────────────────────────────────┤
│              CARRIER BOUNDARY: ids-carrier::Endpoint only                   │
│  send_datagram(Endpoint{CarrierType≤1024B addr, Vec<u8>}, payload: &[u8])   │
│  No std::net / tokio::net / libp2p / IP / TCP / UDP / QUIC / HTTP / DNS      │
└──────────────────────────────────────────────────────────────────────────────┘
```

**Data flow of a CLI command:**
1. `std::env::args_os()` (via clap `Parser::parse()`) yields `OsString` args; clap validates against the derived `Cli` struct.
2. Command dispatch routes the matched subcommand to a command handler.
3. The handler calls the relevant IDS crate API (e.g. `ids-store::write(record_id, bytes)`), optionally signing/encrypting via `ids-crypto`.
4. Any cross-node payload is framed as opaque `Vec<u8>` and sent to `ids-carrier::Endpoint`.
5. Results are formatted: pretty JSON (`serde_json::to_string_pretty`) for scriptable mode, or styled tables/panels (gilt/rich_rust/sparcli vocabulary) for interactive mode, written to stdout (or paged via pager2/lessify when output is long).
6. On `SIGINT`/`SIGTERM` (ctrlc/signal-hook), the TUI restores cooked mode and exits with a clean code.

**Mode selection:** when stdout/stderr is a TTY and no scriptable flag is set, `ids-cli` may offer an interactive TUI (ratatui over crossterm/termion). When piped or when `--json`/scriptable flags are set, it emits structured output and exits. The same command set works in both modes.

---

### 3. Detailed Research Dimensions

Each dimension maps to one or more of the 30 targeted searches executed. Sources are cited inline with the source list in Section 13.

#### 3.1 Argument parsing & subcommand structure (clap derive API)

**Source:** clap 4.x derive docs (`docs.rs/clap/4.3.24/clap/_derive`, `docs.rs/clap/latest/clap/_derive/_tutorial`, clap `Arg` builder docs).

`#[derive(Parser)]` on a top-level `Cli` struct and `#[derive(Subcommand)]` on an enum of commands is the idiomatic Rust CLI pattern. Each subcommand variant carries its own `Args`-deriving struct. `#[command(subcommand)]` wires subcommands; `subcommand_required(true)` and `arg_required_else_help(true)` enforce that a command is always given. `#[command(flatten)]` merges reusable argument groups (e.g. a global `--config`, `--verbose`, `--carrier` group shared by every subcommand). `TypedValueParser` enables custom value validation (e.g. validating a `RecordId` hex string, a `CarrierType` name, or an `IdsTime` timestamp at parse time rather than in the handler).

**Application to ids-cli:** Top-level `Cli` holds global options + `#[command(subcommand)] command: Commands`. `Commands` enum covers `store`, `fragment`, `replicate`, `immutable`, `reclaim`, `config`, `carrier`, `key`, `version`, `help`. Each variant has its own args struct. A flattened `GlobalArgs` group carries `--config <PATH>`, `--format json|human`, `--quiet`, `--verbose`, `--no-color`.

#### 3.2 Shell completion generation (clap_complete)

**Source:** clap_complete `Shell` enum (Bash, Elvish, Fish, PowerShell, Zsh), `clap_complete::aot::{generate, generate_to}`, docs.rs examples, kbknapp.dev blog.

`clap_complete` generates completion scripts at compile time (build.rs `generate_to`) or runtime (`generate` via a `--completion <SHELL>` flag). Supported shells: bash, fish, zsh, powershell, elvish. Completions stay in sync with the actual CLI because they're derived from the same `Command` definition.

**Application to ids-cli:** A `--completion <SHELL>` subcommand/flag emits the completion script to stdout; a build.rs emits completions into `completions/` at compile time. Documented install paths: `~/.config/fish/completions/ids.fish`, `~/.bash_completion.d/ids`, `~/.zsh/completions/_ids`, PowerShell profile.

#### 3.3 Interactive / TUI mode (ratatui, crossterm, termion)

**Source:** ratatui layout/docs (`ratatui.rs/concepts/layout`, `ratatui.rs/examples/widgets/table`), crossterm terminal/event docs (`crossterm::terminal`, `crossterm::event`), termion alternate screen/raw mode docs.

Ratatui divides the terminal into layouts via `Constraint` (Length/Min/Max/Ratio/Percentage) and renders widgets (Table, List, Paragraph, BarChart, Gauge). Crossterm provides cross-platform raw mode (`enable_raw_mode`), alternate screen (`EnterAlternateScreen`/`LeaveAlternateScreen`), and event polling (`Event::Key`, `poll`/`read`). Termion offers `IntoRawMode`/`IntoAlternateScreen` as an alternative. Raw mode disables line buffering and special-key processing; the app must handle `Ctrl+C` itself and restore cooked mode on exit/panic (panic hook).

**Application to ids-cli:** Interactive mode uses ratatui over crossterm (primary) with termion as fallback. Layouts: header (command/status), body (table/list of records/fragments), footer (help hints). Raw mode + alternate screen for full-screen TUI; on non-TTY or piped output, falls back to scriptable mode. Panic hook restores terminal state.

#### 3.4 Styled terminal output (gilt, rich_rust, richrs, sparcli)

**Source:** gilt docs (`docs.rs/gilt`), rich_rust docs (`docs.rs/rich_rust`), richrs GitHub, sparcli GitHub.

These crates bring Python-Rich-style markup (`[bold red]text[/]`), tables with borders/alignment, panels, rules, trees, progress bars, and ANSI color with `NO_COLOR`/`FORCE_COLOR`/`CLICOLOR_FORCE` env support and terminal-size detection. gilt auto-detects TrueColor and records output; rich_rust supports 4-bit/256/24-bit color and hyperlinks; sparcli renders via crossterm without ratatui.

**Application to ids-cli:** Human-readable output uses a styled-console abstraction: records/tables rendered with borders and color, errors in red, successes in green, warnings in yellow. JSON output is always plain (no markup). Color is disabled when `NO_COLOR` is set or stdout is not a TTY.

#### 3.5 Configuration loading (confy, toml, toml-config)

**Source:** confy docs/GitHub (`docs.rs/confy`, rust-lang-nursery cookbook), toml SO answers (`docs.rs/toml_config`, stackoverflow), toml-config `ConfigFactory`.

`confy` zero-boilerplate: `confy::load::<MyConfig>("app-name", None)` resolves per-user config dir (XDG on Linux/macOS, `{FOLDERID_RoamingAppData}` on Windows), deserializes a serde `Default + Serialize + DeserializeOwned` struct from TOML, creates defaults on first run. `load_path`/`store_path` allow explicit paths for testing. TOML structs with nested tables and `#[serde(default)]` handle partial config gracefully.

**Application to ids-cli:** `ids-cli` loads `CliConfig` (default carrier type, default output format, default editor, pager command, confirm-before-destructive, color preference) from `~/.config/ids/ids.toml` (or platform equivalent) via confy. `--config <PATH>` overrides. `#[serde(default)]` on every field so missing keys don't fail. Config is re-loaded, never auto-written without user consent.

#### 3.6 Graceful shutdown & signal handling (ctrlc, signal-hook)

**Source:** tokio shutdown topics (`tokio.rs/tokio/topics/shutdown`), ctrlc crate, signal_hook docs, rust-cli book signals chapter.

`ctrlc::set_handler` registers a Ctrl+C (SIGINT) handler cross-platform; `signal_hook` handles broader Unix signals (SIGTERM, SIGHUP, SIGWINCH) via an iterator or flag. For a CLI that may hold a TUI in raw mode, the signal handler must restore terminal state before exiting. A second Ctrl+C forces immediate exit.

**Application to ids-cli:** `ctrlc` for SIGINT (cross-platform, incl. Windows); `signal_hook` for SIGTERM/SIGHUP on Unix. Handler sets an `AtomicBool` shutdown flag watched by the TUI loop and by long-running commands; TUI restore + cooked-mode reset runs in the handler / panic hook before `process::exit`.

#### 3.7 Progress indicators (indicatif)

**Source:** indicatif docs (`docs.rs/indicatif`, `docs.rs/indicatif/latest/struct.MultiProgress`, `struct.ProgressBar`).

`ProgressBar` (bounded or spinner) and `MultiProgress` (multiple bars from multiple threads, draw to stderr, refresh-rate bounded, auto-hidden on non-TTY). `set_message`/`inc`/`set_position`/`tick`; `suspend()` for external output; `finish_and_clear`/`finish`.

**Application to ids-cli:** Long operations (bulk store, fragment sync, reclaim sweep) show a `ProgressBar` to stderr; multiple concurrent operations use `MultiProgress`; bars auto-hide when piped (non-TTY) so scriptable output stays clean.

#### 3.8 Interactive prompts & hidden input (dialoguer, cliclack, rpassword)

**Source:** dialoguer docs (`docs.rs/dialoguer`, Password/Confirm/Select), cliclack docs (`docs.rs/cliclack`), rpassword source (`docs.rs/rpassword`).

`dialoguer::Confirm` (yes/no), `Select` (pick from list), `Input` (text), `Password` (hidden, optional confirmation) render on stderr using `Term`. `cliclack` offers a similar API with masking. `rpassword::read_password()` hides input by disabling terminal echo (tcgetattr/tcsetattr on Unix, console handle on Windows), returning a `SafeString` that zeroes on drop.

**Application to ids-cli:** Destructive commands (reclaim purge, immutable hold release) require `Confirm::new().interact()`; key/passphrase entry uses `Password` or `rpassword`; multi-option selection uses `Select`. All prompts render to stderr so stdout stays clean for piped data.

#### 3.9 Custom argument validation (clap TypedValueParser)

**Source:** clap `TypedValueParser` docs (`docs.rs/clap/latest/clap/builder/trait.TypedValueParser`), clap derive reference.

`TypedValueParser::parse_ref` validates/transforms a single `OsStr` value into a typed value at parse time. Alternatives: a closure `Fn(&str) -> Result<T, E>`, or `.map()`/`.try_map()` on an existing parser. Used for `RecordId` hex decoding, `CarrierType` parsing, `IdsTime` parsing, file-existence checks.

**Application to ids-cli:** `value_parser!(RecordIdParser)`, `value_parser!(CarrierTypeParser)`, `value_parser!(IdsTimeParser)` reject malformed input before any handler runs, giving immediate clap error messages instead of runtime failures.

#### 3.10 Structured JSON output (serde_json)

**Source:** serde_json `ser` docs (`docs.rs/serde_json/1.0.35/x86_64-unknown-linux-gnu/serde_json/ser`), serde_json_pretty, papercut-cli output example.

`serde_json::to_string_pretty(&value)` / `to_string(&value)` serialize any `Serialize` type to JSON. Pretty-printed output is human-readable; compact output is machine-friendly. CLI tools typically print to stdout with `println!`.

**Application to ids-cli:** `--format json` makes every command emit `serde_json::to_string_pretty` to stdout. A `CliOutput<T>` envelope wraps `{ ok: bool, data: T, error: Option<String> }` so pipelines can parse results reliably.

#### 3.11 Raw argument iteration (std::env::args_os)

**Source:** std `env::args_os` docs, `argv` crate, rust-lang SO.

`std::env::args_os()` returns `ArgsOs` yielding `OsString` — never panics on non-Unicode, first element is the program path. clap's `Parser::parse()` consumes `args_os()` internally. `parse_from` allows supplying a custom iterator for testing.

**Application to ids-cli:** clap handles iteration; `ids-cli` tests use `Cli::parse_from(vec![...])` for deterministic subcommand tests. No direct `args_os()` needed except for `--` trailing-argument passthrough to external tools.

#### 3.12 Recursive directory/file walking (walkdir)

**Source:** walkdir docs (`docs.rs/walkdir`), `WalkDir`/`DirEntry` example.

`WalkDir::new(root).into_iter()` recursively yields `DirEntry` with options to follow symlinks (default off), limit open fds, and skip directories. Used by `ids-cli` commands that operate on fragment directories or stores.

**Application to ids-cli:** `ids store ls <dir>` and `ids fragment list <path>` use `WalkDir` to enumerate files; `ids reclaim` walks fragment roots to find reclaimable files. Symlink-following disabled by default (safety).

#### 3.13 Secure temporary files (tempfile)

**Source:** tempfile docs (`docs.rs/tempfile`, `Builder`, `tempdir`, `NamedTempFile`).

`tempdir()`/`Builder::tempdir_in()` create auto-deleted temp directories; `NamedTempFile`/`tempfile()` create auto-deleted temp files with exclusive creation and private permissions (0600/0700 Unix). Cleanup on drop; `persist()` to keep.

**Application to ids-cli:** `ids store import <file>` stages large imports in a temp dir before committing; `ids key generate` writes private key material to a temp file with 0600 then moves it. Prevents partial-state exposure.

#### 3.14 Path normalization & canonicalization (std::fs::canonicalize, PathBuf)

**Source:** std `fs::canonicalize` docs, `PathBuf` docs, path-absolutize/path-clean crates.

`fs::canonicalize` resolves symlinks and `..` to an absolute path but fails on non-existent paths. `path-absolutize`/`path-clean` normalize without filesystem access. `PathBuf::push` replaces if absolute; `is_absolute`/`has_root` distinguish.

**Application to ids-cli:** `--config`, `--store-path`, `--fragment-dir` arguments are normalized: canonicalize if the path exists, else absolutize relative to cwd. Carried into `ids-carrier::Endpoint` addresses only as opaque strings — `ids-cli` never opens sockets itself.

#### 3.15 CLI error handling (thiserror, anyhow)

**Source:** thiserror guide (`medium.com/rustaceans`, dtolnay/thiserror), anyhow docs (`docs.rs/anyhow`), rustup `errors.rs` example.

`thiserror::Error` derive generates `Display`/`Error`/`From` for enum error types with `#[error("...")]` and `#[from]`. `anyhow::Error` is `Box<dyn Error + Send + Sync + 'static]` with backtrace and `.context()` chaining — suited for `main` returning `Result<(), anyhow::Error>`. Libraries should use `thiserror`; apps/binaries use `anyhow`.

**Application to ids-cli:** `ids-cli` library errors use `thiserror` (`CliError`, `ConfigError`, `DispatchError`, `IoWrap`, `CryptoError`, `ParseError`); the binary `main` returns `Result<(), anyhow::Error>` with context. Exit codes map to error categories (1 = usage, 2 = IO, 3 = protocol/crypto, 130 = SIGINT).

#### 3.16 Singleton / global config (once_cell)

**Source:** once_cell docs (`docs.rs/once_cell`), `OnceCell::get_or_init`, `lazy_static!` pattern.

`OnceCell<T>` holds a value initialized once; `get_or_init(|| ...)` lazily initializes. Safer than `lazy_static!` (no macro, no `Deref` surprises). Used for a global config handle, a global console, or a global shutdown flag.

**Application to ids-cli:** `static CONFIG: OnceCell<CliConfig> = OnceCell::new();` initialized in `main` before dispatch; subcommand handlers read `CONFIG.get()`. The shutdown `AtomicBool` is likewise a `OnceCell`/`AtomicBool` static.

#### 3.17 Platform user directories (directories crate)

**Source:** directories docs (`docs.rs/directories`, `ProjectDirs`, `BaseDirs`, `dirs`).

`ProjectDirs::from("com", "ids", "ids-cli")` resolves `config_dir()`, `data_dir()`, `cache_dir()`, `preference_dir()` per platform: Windows `{FOLDERID_RoamingAppData}/ids/ids-cli/config`, Linux `$XDG_CONFIG_HOME/ids/ids-cli`, macOS `~/Library/Application Support/ids/ids-cli`.

**Application to ids-cli:** Default config path = `ProjectDirs::config_dir()/ids.toml`; default data/cache dirs for local fragment stores and key material. Falls back to cwd-relative `./.ids/` when no home dir (headless/embedded).

#### 3.18 stdin/stdout/stderr handling (std::io)

**Source:** std `io` docs, users.rust-lang piping examples.

`std::io::stdin()`/`stdout()`/`stderr()` are locked handles; `read_line`/`read_to_string` for input, `write!`/`writeln!`/`println!` for output. Raw-mode TUI must use `write!` (not `println!`) since line processing is disabled. Progress bars draw to stderr to keep stdout clean.

**Application to ids-cli:** Scriptable commands read pipe input from stdin (`ids store import --from -`); output to stdout; errors/progress to stderr; interactive prompts to stderr via `Term::stderr()`.

#### 3.19 Argument groups & requirement logic (ArgGroup, required_unless, conflicts_with)

**Source:** clap `Arg`/`ArgGroup` docs (`docs.rs/clap/4.0.32/clap/struct.ArgGroup.html`), derive examples.

`ArgGroup` groups arguments so exactly-one (or at-least-one) must be present; `required_unless("other")` makes an argument required unless another is present; `conflicts_with("other")` forbids co-presence. Global flags propagate to subcommands via `global(true)`.

**Application to ids-cli:** `ids store write` requires either `--file` or `--stdin` (group, exactly one); `--encrypt` conflicts with `--plaintext`; `--carrier` is required unless `--config` supplies a default carrier.

#### 3.20 File permissions & executables (std::fs::set_permissions)

**Source:** std `fs::set_permissions` docs, `PermissionsExt::from_mode`, stackoverflow.

`fs::set_permissions(path, Permissions::from_mode(0o755))` sets executable bits on Unix; on Windows maps to `SetFileAttributes`. Used for installing completion scripts, making helper scripts executable, setting key-file modes (0600).

**Application to ids-cli:** `ids key new` writes the private key file with mode 0600; `ids completions install` writes to shell config directories with appropriate perms; build script may chmod completion files.

#### 3.21 Cross-platform clipboard (clipboard-rs, clipboard-win)

**Source:** clipboard-rs docs (`docs.rs/crate/clipboard-rs`), clipboard-win docs, rust-clipboard README.

`clipboard-rs` supports Windows/macOS/Linux/X11 for plain text, HTML, RTF, image, file list, custom types; `clipboard-win` is the Windows backend (`Clipboard::new().set/get`). `rust-clipboard` (Servo) is another cross-platform option.

**Application to ids-cli:** `ids key copy <id>` copies a public key or fingerprint to the clipboard; `ids token copy` copies a session token. Optional feature-gated (clipboard not available on headless/embedded).

#### 3.22 Terminal paging (pager2, lessify, sartorial)

**Source:** pager2 docs (`docs.rs/pager2`), lessify docs, sartorial `PagerMode` (`Auto`/`Always`/`Never`).

`Pager::with_pager("less -R").setup()` pipes stdout through an external pager (`less -R` on Unix, `more.com` on Windows); respects `PAGER` env var; `NOPAGER` disables. `sartorial::PagerMode::Auto` pages only on attended TTY when output exceeds terminal height.

**Application to ids-cli:** `ids store list` / `ids fragment log` pipe through pager when output is long and stdout is a TTY; `--no-pager` disables; `PagerMode::Auto` default respects `PAGER`/`NOPAGER`.

#### 3.23 Interactive TUI input loops (crossterm event poll/read)

**Source:** crossterm event docs, `event-poll-read.rs` example, termion `TermRead`/`read_passwd`.

`crossterm::event::{poll, read, Event, KeyCode}` reads key/mouse/resize events after `enable_raw_mode`; `poll(Duration)` gives non-blocking timeout. `read()` blocks until an event. Termion's `TermRead::read_passwd`/`read_key` similar.

**Application to ids-cli:** The TUI main loop polls events with a short timeout so it can redraw progress/status between keystrokes; `q`/`Ctrl+C` quits; arrow keys navigate tables; `Enter` confirms.

#### 3.24 Alternate screen & raw-mode lifecycle (termion/crossterm)

**Source:** termion `IntoAlternateScreen`/`IntoRawMode`, ratatui panic-hooks recipe, crossterm `EnterAlternateScreen`/`LeaveAlternateScreen`.

Alternate screen is a full-screen buffer with no scrollback; restored on drop. Raw mode disables echo/line-buffering/special keys. Both must be torn down on exit and panic (panic hook). `stdout().flush()` required after mode switches.

**Application to ids-cli:** `init_tui()` enables raw mode + alternate screen + hides cursor; `restore_tui()` reverses; a `std::panic::set_hook` wraps `restore_tui()` so a crash leaves a usable terminal.

#### 3.25 Pager + TTY detection for mode selection

Combining 3.3/3.18/3.22: `ids-cli` detects `atty`/stdout-is-TTY to choose interactive vs scriptable; when interactive and output is long, pages; when piped, never pages and never TUI.

**Application to ids-cli:** A `Console` abstraction holds `is_tty`, `color_support`, `pager_mode`; command handlers query it before rendering.

#### 3.26 External command passthrough (std::process::Command)

**Source:** std `process::Command` docs (referenced in signal/crossterm examples).

`Command::new("editor").arg(file).spawn()` launches `$EDITOR` for multi-line input; `ids config edit` opens the config file in the user's editor; `ids store import` may delegate to an external tool.

**Application to ids-cli:** `ids config edit` resolves `$EDITOR` (or `notepad.exe`/`vim` fallback) and spawns it, waiting for exit before reloading config. Output captured or inherited based on TTY.

#### 3.27 Subcommand help & long help text (clap about/long_about)

**Source:** clap derive docs (`#[command(about, long_about, version)]`), `next_line_help` attribute.

`#[command(about = "short")]` / `long_about = "..."` per struct and per subcommand; `help` subcommand auto-generated; `next_line_help` wraps options on their own line for readability.

**Application to ids-cli:** Every subcommand has `about` (one line) and `long_about` (paragraph + examples). `ids help <cmd>` shows full docs including carrier-agnostic notes and exit codes.

#### 3.28 Version propagation (clap propagate_version)

**Source:** clap derive docs (`#[command(propagate_version = true)]`).

`propagate_version = true` makes subcommands inherit the parent version; `--version` on any subcommand prints the crate version.

**Application to ids-cli:** `ids --version` and `ids <cmd> --version` both print `ids-cli 0.1.0` from `Cargo.toml` version; version also embedded via `env!("CARGO_PKG_VERSION")`.

#### 3.29 Default subcommand / no-subcommand behavior

**Source:** clap `arg_required_else_help(true)`, subcommand_required.

When no subcommand is given, `arg_required_else_help(true)` prints help and exits 0 (or 2 for misuse). A `default_subcommand` can route bare `ids` to `ids store list`.

**Application to ids-cli:** Bare `ids` with a TTY opens interactive TUI; bare `ids` piped prints help and exits 2. Configurable via `default_subcommand = Commands::StoreList` in config.

#### 3.30 Carrier-agnostic boundary enforcement (ids-carrier::Endpoint)

**Source:** ids-000-aa `types.rs` (verified: `CarrierType` ∈ {Loopback, Serial, Optical, Radio, Custom(u16)}, `IdsTime(pub u64)`, `RecordId([u8;32])`, `Checksum([u8;32])`), plan-040.md carrier boundary pattern.

`ids-carrier::Endpoint` = `{ CarrierType, Vec<u8> }` (address ≤1024 bytes); `send_datagram(endpoint, payload)` / `receive_datagram() -> (Endpoint, Vec<u8>)`. `ids-cli` holds an `Endpoint` target (from `--carrier` or config) and sends all outbound traffic through it. `ids-cli` imports only `ids-carrier`'s `Endpoint` type — never `std::net`, `tokio::net`, or any transport crate.

**Application to ids-cli:** A `CarrierAdapter` module wraps `send_datagram`/`receive_datagram`; commands like `ids carrier send <endpoint> <file>` and `ids carrier recv` exercise it directly; all other commands route cross-node traffic through the driver crates which use the same adapter. No network stack in `ids-cli`.

---

### Threat Model

|| Threat | Asset at risk | Mitigation (this crate) ||
|--------|-----------|-------------------------------|
| Accidental destructive command (purge/delete without confirm) | Data loss | Destructive subcommands require `--confirm` flag or interactive `Confirm` prompt; `--dry-run` previews effects |
| Malformed RecordId/CarrierType/IdsTime argument | Handler crash, invalid state | `TypedValueParser` rejects at parse time; clap error before any IDS crate call |
| Secret/key material leaked to stdout or logs | Key exposure | Secrets never printed; `rpassword`/dialoguer `Password` for input; key files mode 0600; `--format json` never includes private fields |
| TUI crash leaving raw mode enabled | Unusable terminal | Panic hook restores cooked mode + alternate screen exit; `finally`-style `restore_tui()` |
| Config file with missing/invalid keys | Misconfigured behavior | `#[serde(default)]` on all fields; confy creates defaults; validation at load with clear errors |
| Long output flooding a pipe/log | Scriptable output corruption | Progress bars draw to stderr; stdout is JSON/plain only; `--no-pager`/auto-detect non-TTY |
| Signal during TUI (SIGINT/SIGTERM) | Incomplete state, raw mode left on | ctrlc/signal_hook set shutdown flag; TUI loop checks and exits cleanly; restore on signal |
| Carrier target misconfiguration | Traffic sent to wrong endpoint | Endpoint validated at parse time (`CarrierTypeParser`); `--dry-run` shows target without sending |
| Clipboard paste of secret into visible field | Shoulder-surfing / log leak | Clipboard ops optional; secrets not echoed; `Password` masks input |
| Config directory not writable | Config cannot be saved | Fall back to cwd-relative `./.ids/`; clear error; never fail silently |

The carrier is never trusted: `ids-cli` signs/encrypts payloads via `ids-crypto` before handing bytes to `ids-carrier::Endpoint`, so payloads survive any carrier re-framing.

---

### Integration with Existing Crates

|| Crate | Role | Relevance to ids-cli ||
|-------|------|--------------------------|
| `ids-000-aa` | Master crate: canonical types + sealed trait contracts + errors | `RecordId`, `NodeId`, `RegionId`, `SnapshotId`, `Checksum`, `IdsTime`, `CarrierType`, `AnomalyType`, `Error` — consumed, never redefined |
| `ids-carrier` | Opaque `Endpoint` (`CarrierType` + `Vec<u8>`) datagram send/receive | All cross-node traffic from CLI-driven operations goes to `Endpoint`; `ids-cli` imports only `Endpoint` |
| `ids-crypto` | Allowed primitives: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 | `ids key`, `ids encrypt/decrypt`, signing of config/keys consumed via ids-crypto |
| `ids-store` | Primary record store | `ids store write/read/list/delete` route through its API |
| `ids-fragment` | Record fragments/shards | `ids fragment list/sync/reclaim` |
| `ids-replication` | Replica synchronization | `ids replicate status/pause/resume` |
| `ids-immutable` | Retention/legal-hold guard | `ids immutable hold/release/status` |
| `ids-reclaim` (Phase 041) | Fragment compaction & reclamation | `ids reclaim sweep/status` |

---

### Proposed File Structure (~90–1000 files in subfolders)

`C:/idfs/ids-cli/` (separate crate, not a folder inside an existing crate):

```
ids-cli/
├── Cargo.toml
├── build.rs                          # compile-time shell completion generation (clap_complete generate_to)
├── src/
│   ├── lib.rs                        # crate root, public API, feature gates
│   ├── main.rs                       # binary entry: parse, dispatch, exit codes, panic hook
│   ├── constants.rs                  # exit codes, default config name, pager defaults
│   ├── errors.rs                     # CliError (thiserror): Parse, Config, Dispatch, Io, Crypto, Carrier
│   ├── types.rs                      # CliOutput, CommandResult, Mode (Scriptable|Interactive)
│   │
│   ├── cli/                          # clap derive definitions (~15 files)
│   │   ├── mod.rs
│   │   ├── root.rs                   # Cli struct, GlobalArgs, Commands enum
│   │   ├── store.rs                  # store subcommand + args
│   │   ├── fragment.rs               # fragment subcommand + args
│   │   ├── replicate.rs              # replicate subcommand + args
│   │   ├── immutable.rs              # immutable subcommand + args
│   │   ├── reclaim.rs                # reclaim subcommand + args
│   │   ├── carrier.rs                # carrier subcommand (send/recv/endpoint)
│   │   ├── key.rs                    # key subcommand (gen/copy/show)
│   │   ├── config.rs                 # config subcommand (get/set/edit)
│   │   ├── version.rs                # version subcommand
│   │   ├── completion.rs             # --completion <SHELL> + build.rs hook
│   │   ├── parsers.rs                # TypedValueParser: RecordId, CarrierType, IdsTime, Endpoint
│   │   ├── groups.rs                 # ArgGroup definitions (encrypt/plaintext, file/stdin)
│   │   ├── help.rs                   # long_about text per subcommand
│   │   └── mod_private.rs            # internal re-exports
│   │
│   ├── dispatch/                     # command routing to IDS crates (~12 files)
│   │   ├── mod.rs
│   │   ├── store_cmd.rs
│   │   ├── fragment_cmd.rs
│   │   ├── replicate_cmd.rs
│   │   ├── immutable_cmd.rs
│   │   ├── reclaim_cmd.rs
│   │   ├── carrier_cmd.rs
│   │   ├── key_cmd.rs
│   │   ├── config_cmd.rs
│   │   ├── version_cmd.rs
│   │   └── dry_run.rs                # preview effects without executing
│   │
│   ├── output/                       # formatting & rendering (~10 files)
│   │   ├── mod.rs
│   │   ├── json.rs                   # serde_json pretty/compact to stdout
│   │   ├── human.rs                  # styled tables/panels (gilt/rich_rust vocabulary)
│   │   ├── console.rs                # TTY/color/pager detection
│   │   ├── pager.rs                  # pager2/lessify integration, PagerMode Auto/Always/Never
│   │   ├── progress.rs               # indicatif ProgressBar/MultiProgress to stderr
│   │   └── envelope.rs               # CliOutput<T> envelope type
│   │
│   ├── interactive/                  # TUI mode (ratatui/crossterm) (~14 files)
│   │   ├── mod.rs
│   │   ├── tui.rs                    # init/restore raw mode + alternate screen + panic hook
│   │   ├── app.rs                    # TUI application state + event loop
│   │   ├── layout.rs                 # Layout constraints (header/body/footer)
│   │   ├── widgets.rs                # table/list/paragraph widgets
│   │   ├── events.rs                 # crossterm event poll/read, key mapping
│   │   ├── confirm.rs                # interactive Confirm prompt
│   │   ├── password.rs               # dialoguer Password / rpassword hidden input
│   │   ├── select.rs                 # interactive Select prompt
│   │   ├── input.rs                  # interactive Input prompt
│   │   ├── renderer.rs               # render loop
│   │   ├── mode_switch.rs            # TUI <-> scriptable mode toggle
│   │   └── editor.rs                 # std::process::Command $EDITOR spawn
│   │
│   ├── config/                       # configuration loading (~8 files)
│   │   ├── mod.rs
│   │   ├── cli_config.rs             # CliConfig serde struct with #[serde(default)]
│   │   ├── loader.rs                 # confy load/load_path, defaults on first run
│   │   ├── resolver.rs               # ProjectDirs config/data/cache dir resolution
│   │   ├── watcher.rs                # optional config file reload on change
│   │   ├── editor_integration.rs     # config edit via $EDITOR
│   │   ├── validation.rs             # config validation at load
│   │   └── fallback.rs               # cwd-relative ./ .ids/ fallback
│   │
│   ├── carrier_adapter/              # ids-carrier::Endpoint wrapper (~6 files)
│   │   ├── mod.rs
│   │   ├── endpoint.rs               # Endpoint construction/validation from args
│   │   ├── send.rs                   # send_datagram wrapper
│   │   ├── recv.rs                   # receive_datagram wrapper
│   │   ├── resolver.rs               # --carrier <type>:<addr> parsing
│   │   └── carrier_agnostic.rs       # invariant: no std::net/tokio::net imports
│   │
│   ├── signals/                      # graceful shutdown (~4 files)
│   │   ├── mod.rs
│   │   ├── ctrlc_handler.rs          # SIGINT cross-platform
│   │   ├── signal_hook.rs            # SIGTERM/SIGHUP Unix
│   │   └── shutdown_flag.rs          # AtomicBool static
│   │
│   ├── io/                           # stdin/stdout/stderr & files (~6 files)
│   │   ├── mod.rs
│   │   ├── stdin_reader.rs           # read_line/read_to_string from pipe
│   │   ├── stdout_writer.rs          # write!/writeln! to stdout
│   │   ├── stderr_writer.rs          # progress/errors to stderr
│   │   ├── path_normalizer.rs        # canonicalize / absolutize paths
│   │   └── permissions.rs            # fs::set_permissions 0600/0755
│   │
│   ├── clipboard/                    # optional clipboard (~3 files)
│   │   ├── mod.rs
│   │   ├── clipboard_rs.rs           # clipboard-rs backend
│   │   └── clipboard_win.rs          # clipboard-win Windows backend
│   │
│   ├── contracts/                    # sealed trait contracts (~8 files)
│   │   ├── command_contract.rs, dispatch_contract.rs, output_contract.rs,
│   │   │   tui_contract.rs, config_contract.rs, carrier_contract.rs,
│   │   │   io_contract.rs, mod.rs
│   │
│   ├── invariants/                   # compile-time/runtime invariants (~8 files)
│   │   ├── no_network_stack.rs, carrier_only_traffic.rs, crypto_allowlist.rs,
│   │   │   secrets_not_printed.rs, progress_to_stderr.rs, tty_detection.rs,
│   │   │   exit_code_mapping.rs, mod.rs
│   │
│   ├── features/                     # cfg-gated optional subsystems (~6 files)
│   │   ├── mod.rs, tui.rs, clipboard.rs, pager.rs, interactive_prompts.rs, editor.rs
│   │
│   ├── tests/                        # integration tests (~18 files)
│   │   ├── mod.rs
│   │   ├── parse_tests.rs            # clap parse_from per subcommand
│   │   ├── dispatch_tests.rs         # command -> IDS crate routing
│   │   ├── output_tests.rs           # JSON vs human formatting
│   │   ├── config_tests.rs           # load/fallback/validation
│   │   ├── carrier_tests.rs          # Endpoint construction
│   │   ├── parser_tests.rs           # TypedValueParser validation
│   │   ├── dry_run_tests.rs          # preview without effects
│   │   ├── tui_tests.rs              # headless TUI render (no raw mode)
│   │   ├── signal_tests.rs           # shutdown flag behavior
│   │   ├── clipboard_tests.rs        # optional
│   │   ├── permission_tests.rs       # key file 0600
│   │   ├── invariant_tests.rs        # no-network-stack check
│   │   └── exit_code_tests.rs        # exit code mapping
│   ├── benches/                      # benchmarks (~8 files)
│   │   ├── mod.rs
│   │   ├── parse_bench.rs            # clap parse latency
│   │   ├── render_bench.rs           # TUI render FPS
│   │   ├── json_bench.rs             # serialization throughput
│   │   ├── progress_bench.rs         # indicatif overhead
│   │   └── config_load_bench.rs
│   ├── fuzz/                         # fuzzing harnesses (~6 files)
│   │   ├── mod.rs
│   │   ├── fuzz_cli_args.rs
│   │   ├── fuzz_endpoint.rs
│   │   ├── fuzz_config.rs
│   │   └── fuzz_output.rs
│   ├── docs/                         # documentation (~14 files)
│   │   ├── architecture.md, command_reference.md, config_reference.md,
│   │   │   interactive_mode.md, scriptable_mode.md, carrier_agnostic.md,
│   │   │   exit_codes.md, examples.md, completions.md, faq.md,
│   │   ├── man/                      # man pages (~6 files: ids.1, ids-store.1, ...)
│   │   └── contributing.md
│   ├── completions/                  # shell completion scripts (~5 files)
│   │   ├── ids.bash, ids.fish, ids.zsh, ids.ps1, ids.elvish
│   └── test_vectors/                 # deterministic test vectors (~6 files)
│       ├── parse_vectors.json
│   │   ├── endpoint_vectors.json
│   │   └── config_vectors.json
```

**Why ~90–1000 files:** one file per concern — clap subcommands, dispatch handlers, output formats, TUI widgets, config resolution, carrier adapter, signal handling, IO, contracts, invariants, plus tests/benches/fuzz/docs/man/completions/test_vectors. Production-grade: full error handling, documentation, unit tests per file. No stubs.

---

### Key Algorithms & Data Structures

#### 3.1 CLI parse → dispatch → output pipeline

```
args = std::env::args_os()                          // clap consumes internally
matches = Cli::parse()                              // derive Parser + Subcommand
config = confy::load::<CliConfig>("ids", None)      // TOML, #[serde(default)]
mode = matches.is_tty ? Interactive : Scriptable   // or --force-scriptable
handler = dispatch(matches.subcommand, config)     // route to IDS crate API
result = handler.run()                              // Result<CliOutput<T>, CliError>
render(result, mode)                                // JSON to stdout OR styled TUI
on_signal: shutdown_flag.store(true); restore_tui(); exit(130)
```

#### 3.2 TypedValueParser validation (parse-time, not runtime)

```
RecordIdParser: 32 hex bytes -> ids-000-aa::RecordId([u8;32]); reject non-hex / wrong length
CarrierTypeParser: "loopback"|"serial"|"optical"|"radio"|"custom:N" -> CarrierType
IdsTimeParser: u64 -> IdsTime(u64); reject negative
EndpointParser: "<type>:<addr>" -> ids-carrier::Endpoint; addr ≤ 1024 bytes
```

#### 3.3 Output envelope

```
CliOutput<T> { ok: bool, data: Option<T>, error: Option<CliErrorInfo>, ids_time: IdsTime }
--format json   -> serde_json::to_string_pretty(&envelope) to stdout
--format human  -> styled table/panel to stdout; errors in red to stderr
```

#### 3.4 TUI event loop

```
loop {
    if crossterm::event::poll(100ms)? { handle_event(read()?) }
    if shutdown_flag.load() { break }
    render(frame)                                     // ratatui draw
    progress_bars.tick()                              // indicatif, to stderr
}
restore_tui()                                         // cooked mode, main screen, cursor show
```

#### 3.5 Config resolution precedence

1. `--config <PATH>` (explicit, canonicalized/absolutized)
2. `IDS_CONFIG` env var
3. `ProjectDirs::config_dir()/ids.toml` (platform config dir)
4. `./.ids/ids.toml` (cwd fallback, headless)
Missing → defaults (`#[serde(default)]`), never auto-write without consent.

#### 3.6 Exit code mapping

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | Usage / argument error (clap) |
| 2 | IO error (file, config, permissions) |
| 3 | Protocol / crypto / carrier error |
| 130 | SIGINT / user abort |

---

### Dependencies

| Crate | Purpose |
|-------|---------|
| `ids-000-aa` | Canonical types (`RecordId`, `NodeId`, `RegionId`, `SnapshotId`, `Checksum`, `IdsTime`, `CarrierType`, `AnomalyType`) + sealed contracts + errors |
| `ids-carrier` | `Endpoint` (`CarrierType` + `Vec<u8>`) + `Carrier` trait — opaque datagram transport only |
| `ids-crypto` | Allowed primitives only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 |
| `clap` (derive) | Argument parsing, subcommands, ArgGroup, TypedValueParser |
| `clap_complete` | Shell completion generation (bash, fish, zsh, powershell, elvish) |
| `serde` + `toml` | Config struct serialization |
| `confy` | Per-user config dir resolution + load/store |
| `directories` | ProjectDirs config/data/cache dirs |
| `thiserror` | Library error types |
| `anyhow` | Binary main error context + backtrace |
| `once_cell` | Global config / shutdown flag |
| `indicatif` | Progress bars/spinners to stderr |
| `crossterm` | Raw mode, alternate screen, event polling (TUI) |
| `ratatui` | TUI widgets (Table, List, Paragraph, Gauge) + Layout constraints |
| `termion` | Fallback raw mode / alternate screen |
| `gilt` / `rich_rust` / `sparcli` | Styled terminal output (tables, panels, color) |
| `serde_json` | JSON output envelope |
| `dialoguer` | Interactive Confirm/Select/Input/Password prompts |
| `cliclack` | Alternative interactive prompts |
| `rpassword` | Hidden password input (no echo) |
| `pager2` / `lessify` | Terminal paging for long output |
| `walkdir` | Recursive directory iteration |
| `tempfile` | Secure temp files/dirs |
| `ctrlc` | Cross-platform SIGINT handler |
| `signal-hook` | Unix SIGTERM/SIGHUP handling |
| `std::process::Command` | `$EDITOR` spawn for config edit |
| `clipboard-rs` / `clipboard-win` | Optional clipboard (feature-gated) |
| `std::io` / `std::fs` / `std::env` / `std::path` | stdin/stdout/stderr, file IO, args, path normalization |

No `std::net`, `tokio::net`, libp2p, or any networking stack. No crypto beyond the allowed list.

---

### Measurable Acceptance Criteria

|| Criterion | Target | Measurement ||
|-----------|----------|----------|-------------|
| Subcommand set | `store`/`fragment`/`replicate`/`immutable`/`reclaim`/`carrier`/`key`/`config`/`version`/`help` all parse | unit test, `Cli::parse_from` per subcommand |
| Argument validation | `TypedValueParser` rejects malformed RecordId/CarrierType/IdsTime at parse time | clap error, non-zero exit |
| Scriptable mode | `--format json` emits valid pretty JSON to stdout, no TUI | JSON parse test, stdout capture |
| Interactive mode | TTY + no `--force-scriptable` → TUI render without crash | headless render test; real TUI on TTY |
| Output separation | Progress/errors to stderr; data to stdout | stream capture test |
| Config loading | `#[serde(default)]`; missing keys → defaults; no auto-write | load test with missing/partial config |
| Config precedence | `--config` > env > ProjectDirs > cwd fallback | precedence test |
| Carrier boundary | Zero `std::net`/`tokio::net`/libp2p/TCP/UDP/IP/HTTP/DNS references | grep/audit invariant test |
| Crypto allowlist | Only ed25519/xchacha20poly1305/sha256/blake3/chacha20poly1305 | dependency audit + source scan |
| Exit codes | 0/1/2/3/130 mapped correctly | exit-code test per error category |
| Destructive confirm | Purge/delete requires `--confirm` or interactive Confirm | prompt test, `--dry-run` preview |
| Secrets not printed | Private keys/passphrases never on stdout/logs | stdout scan; key file mode 0600 |
| Signal handling | SIGINT sets shutdown flag; TUI restores cooked mode | signal test + TUI restore test |
| Shell completions | `--completion <SHELL>` emits script for bash/fish/zsh/ps1/elvish | generation test per shell |
| Pager | Long output pages on TTY; no page when piped | pager-mode test |
| Clipboard (optional) | `ids key copy` writes to clipboard when feature enabled | clipboard feature test |
| Temp files | Key/staging files 0600, cleaned on drop | permission + drop test |
| Path normalization | `--config`/paths canonicalized or absolutized consistently | path test |
| Help text | Every subcommand has `about` + `long_about`; `--version` works | help/version test |

---

### Remaining Uncertainties

1. **TUI crate selection:** ratatui+crossterm (primary) vs termion (fallback) vs sparcli (crossterm-only, no ratatui). Ratatui offers richest widgets; sparcli is lighter. Decide at implementation; the `interactive/` module is cfg-gated so either can be swapped.
2. **Styled output crate:** gilt vs rich_rust vs richrs vs sparcli — all provide similar markup/table/panel APIs. Pick one; abstract behind a `Console` trait so it's swappable.
3. **Interactive prompt crate:** dialoguer vs cliclack vs custom crossterm. dialoguer is most mature; cliclack is newer with nicer UX. Abstract behind prompts trait.
4. **Config write policy:** confy auto-creates defaults on first load; ids-cli may prefer never writing automatically — load-only with explicit `ids config set` to write. Policy decision needed.
5. **Default subcommand:** bare `ids` → interactive TUI vs help vs `ids store list`. UX decision; config-gated.
6. **Editor command resolution:** `$EDITOR` fallback chain (`notepad.exe`/`vim`/nano) needs platform handling for `ids config edit`.
7. **Clipboard on headless:** clipboard feature must gracefully degrade (no-op) on systems without a clipboard; feature-gating needed.
8. **Multi-progress TUI interaction:** indicatif bars draw to stderr; TUI draws to stdout alternate screen — potential screen corruption when both active; need to route progress into the TUI body or disable bars during TUI.
9. **Windows raw mode + alternate screen:** crossterm/termion behavior on Windows Terminal vs legacy cmd/conhost may differ; needs testing.

---

### Engineering Decisions Summary

| Decision | Rationale |
|----------|-----------|
| Separate crate `ids-cli` (not a folder inside an existing crate) | Follows "separate crates instead of more folders" directive; clear dependency boundary |
| clap derive (`Parser`/`Subcommand`/`Args`) | Idiomatic Rust CLI; subcommands with own args; `subcommand_required`, `arg_required_else_help` |
| clap_complete for shell completions | Completions stay in sync with the CLI; bash/fish/zsh/powershell/elvish |
| ratatui+crossterm TUI + termion fallback | Rich TUI widgets; cross-platform raw mode/alternate screen |
| Scriptable + interactive modes, auto-detected via TTY | Same command set works piped and interactive; scriptable output never polluted by TUI |
| `#[serde(default)]` config via confy + directories | Missing keys never break; per-user config dir per platform; cwd fallback for headless |
| `TypedValueParser` for RecordId/CarrierType/IdsTime/Endpoint | Fail-fast at parse time; clear clap error messages |
| `CliOutput<T>` envelope + `--format json` | Machine-parseable output for pipelines |
| Progress/progress bars to stderr; data to stdout | Keeps piped stdout clean |
| thiserror (lib) + anyhow (binary main) | Library errors typed; app errors contextual |
| ctrlc (SIGINT) + signal_hook (SIGTERM/SIGHUP) | Cross-platform graceful shutdown; TUI state restored |
| No `std::net`/`tokio::net`/libp2p; all traffic via `ids-carrier::Endpoint` | Carrier-agnostic; preserves IDS architecture |
| Only ed25519/xchacha20poly1305/sha256/blake3/chacha20poly1305 | Complies with allowed-crypto rule |
| Panic hook restores TUI state | Never leaves terminal in raw mode after crash |

---

### Sources

1. clap derive API — https://docs.rs/clap/4.3.24/clap/_derive
2. clap derive tutorial — https://docs.rs/clap/latest/clap/_derive/_tutorial
3. clap `Arg` builder — https://docs.rs/clap/latest/clap/struct.Arg.html
4. clap `ArgGroup` — https://docs.rs/clap/4.0.32/clap/struct.ArgGroup.html
5. clap_complete Shell enum & generation — https://github.com/clap-rs/clap/blob/6982fb1c/clap_complete/src/aot/shells/shell.rs; https://docs.rs/clap_complete/4.6.11/clap_complete/aot
6. kbknapp shell completions blog — https://kbknapp.dev/shell-completions
7. ratatui layout — https://ratatui.rs/concepts/layout
8. ratatui table example — https://ratatui.rs/examples/widgets/table
9. crossterm terminal — https://docs.rs/crossterm/latest/crossterm/terminal
10. crossterm event — https://docs.rs/crossterm/latest/src/crossterm/event.rs.html
11. crossterm event-poll-read example — https://github.com/crossterm-rs/crossterm/blob/master/examples/event-poll-read.rs
12. termion alternate screen/raw mode — https://docs.rs/termion/4.0.3/termion/screen/trait.IntoAlternateScreen.html
13. ratatui panic hooks — https://ratatui.rs/recipes/apps/panic-hooks
14. gilt styled output — https://docs.rs/gilt
15. rich_rust — https://docs.rs/rich_rust
16. richrs — https://github.com/olirice/richrs
17. sparcli — https://github.com/dicklesworthstone/rich_rust (sparcli)
18. confy — https://github.com/rust-cli/confy; https://docs.rs/confy/2.0.0/src/confy/lib.rs.html
19. Rust Cookbook per-user config — https://rust-lang-nursery.github.io/rust-cookbook/configuration/confy.html
20. toml parsing — https://docs.rs/toml_config/latest/toml_config/struct.ConfigFactory.html; stackoverflow TOML struct/HashMap
21. directories ProjectDirs — https://docs.rs/directories/latest/directories/struct.ProjectDirs.html
22. ctrlc / signal-hook — https://docs.rs/signal-hook; https://rust-cli.github.io/book/in-depth/signals.html
23. tokio shutdown (ctrl_c) — https://tokio.rs/tokio/topics/shutdown
24. indicatif — https://docs.rs/indicatif/latest/indicatif
25. dialoguer — https://docs.rs/dialoguer
26. cliclack — https://docs.rs/cliclack
27. rpassword — https://docs.rs/crate/rpassword/7.3.1/source/src/lib.rs
28. clap TypedValueParser — https://docs.rs/clap/latest/clap/builder/trait.TypedValueParser.html
29. serde_json — https://docs.rs/serde_json/1.0.35/x86_64-apple-darwin/serde_json/ser
30. std::env::args_os — https://doc.rust-lang.org/std/env/fn.args_os.html
31. walkdir — https://docs.rs/walkdir
32. tempfile — https://docs.rs/tempfile/latest/tempfile
33. std::fs::canonicalize / PathBuf — https://doc.rust-lang.org/stable/std/fs/fn.canonicalize.html
34. thiserror — https://github.com/dtolnay/thiserror
35. anyhow — https://docs.rs/anyhow/latest/anyhow/struct.Error.html
36. once_cell — https://docs.rs/once_cell/0.1.1/once_cell
37. pager2 / lessify / sartorial — https://docs.rs/pager2; https://docs.rs/lessify; sartorial PagerMode
38. clipboard-rs / clipboard-win — https://docs.rs/crate/clipboard-rs/latest
39. std::fs::set_permissions — https://doc.rust-lang.org/std/fs/fn.set_permissions.html
40. ids-000-aa `src/types.rs` (verified): `CarrierType` {Loopback, Serial, Optical, Radio, Custom(u16)}, `IdsTime(pub u64)`, `RecordId([u8;32])`, `Checksum([u8;32])`, `AnomalyType` {ByteMismatch, ReplicaDrift, ChecksumFailure, StateCorruption}
41. plan-040.md (IDS plan format reference) — C:/idfs/plans/plan-040.md
42. plan-077.md, plan-080.md, plan-121.md (HX-241 plan format reference) — C:/hx-241/PLANS/

**Research status:** Complete. Ready for implementation phase. No code written, no files created, no repository changes made. The blueprint above defines all architecture, interfaces, data structures, algorithms, dependencies, integration points, acceptance criteria, and file structure for Phase 046.