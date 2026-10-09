# Install command and exe location (card 7faab330)

base: 7307b4ec6c3656cc008e688cac02bb884d2672f6

## Context
design.md > Architecture > Windows exe lock. Running MCP servers lock the installed exe.
Windows refuses to overwrite or delete a running exe but allows renaming it. So install
renames the locked exe aside instead of the `.new.exe` swap-on-next-start in the card: same
outcome, no deferred step, new sessions get the new build immediately.
Other cards running in parallel: MCP tools (src/mcp), GUI shell (src/ui), Import
(src/import, src/store, src/main.rs). You own src/setup and the `install` lines of
src/main.rs. Expect a trivial main.rs rebase conflict with Import's subcommand; keep both.

## Design
- `src/setup/mod.rs`: `pub mod install;`
- `src/setup/install.rs`:
  - `pub fn bin_dir() -> PathBuf`: `%LOCALAPPDATA%\FuelStation\bin`.
  - `pub fn installed_exe() -> PathBuf`: `bin_dir().join("fuel-station.exe")`. Add Project
    (later card) writes this path into `.mcp.json`.
  - `pub fn install_to(src: &Path, dir: &Path) -> io::Result<PathBuf>`: create dir; remove
    a stale `fuel-station.old.exe` (ignore errors); if `fuel-station.exe` exists, try
    `fs::copy` over it; on failure rename it to `fuel-station.old.exe` (if that name is still
    taken by a running exe, use `fuel-station.old-<unix secs>.exe`) then copy. Return the
    installed path.
  - `pub fn install() -> io::Result<PathBuf>`: `install_to(current_exe, bin_dir())`.
  - `pub fn cleanup_old(dir: &Path)`: delete every `fuel-station.old*.exe` in dir, ignore
    errors (locked ones stay until their sessions close).
- `src/main.rs`: subcommand `Install`; prints `installed <path>` or the error and exits 1.
  At startup of every subcommand except `mcp`, call `setup::install::cleanup_old(&bin_dir())`.
- Installing the running exe onto itself (src == dest) -> print "already installed" and
  return Ok.

## Tests (temp dirs)
Fresh install copies; reinstall over an existing unlocked file replaces content;
cleanup_old removes `.old` files; src == dest is a no-op. A real lock test: spawn the
installed copy with a long-running arg is not possible with this binary portably; skip it,
the manual check covers it.

## Verification
Gate: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`.
Manual (user): with a Claude session running the station MCP from the installed exe,
`cargo build && target\debug\fuel-station.exe install` succeeds; a new session gets the new
build; the old exe is cleaned up after the old session closes and Station starts again.

## Out of scope
Agent file install (own card), Add Project.

Tier:  1 implementer. Why: plan names every function; small std::fs code.
Gate:  cargo fmt/clippy/test. Why: lock behaviour is manual.
Scope: 2 files in src/setup + main.rs. Asks: none
