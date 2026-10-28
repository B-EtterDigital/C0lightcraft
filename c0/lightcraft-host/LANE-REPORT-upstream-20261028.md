# Upstream pull 20261028: lightcraft 37b2e523ad40 into c0/main

Branch `fix`. Merge `d551055d` (parents `a30d6f73` = c0/main, `37b2e523` = upstream). The merge descends from `origin/c0/main`; no fork commit was rebased or rewritten. Commits after the merge:

- `27b478ee` chore(c0): keep upstream trademarks and CI out of c0/main
- `a627f815` fix(c0): use upstream's base64 0.23 instead of a second copy
- `cf2dbcd5` fix(c0): follow upstream's egui 0.37 control outcome and Id API
- `156fa0cf` fix(c0): refuse gesture requests in the page; drop a one-item loop in the test
- `7e93e5d8` docs(c0): recount upstream patch debt after the merge (61 of 67)
- this report

Note on history: the first merge commit I made captured upstream's versions of `lib.rs` and `dialogs.rs` because the hooks were edited after staging. I rebuilt the merge and chore commits from explicit trees before anything was shared; the commits above are the final ones.

## Conflicts and resolution

| File | Conflict | Resolution |
|---|---|---|
| `.github/workflows/{ci,freebsd,packaging-lint,release,sonarcloud,windows-arm64}.yml` | modify/delete (ours deleted) | Upstream copies in the merge, removed by the chore commit as the sync procedure requires |
| `Cargo.lock` | content | Took upstream, then re-resolved. Our pin was changed to base64 0.23 (see below). Lock diff is now one line |
| `crates/ui-egui/src/lib.rs` | content (upstream split `lib.rs`) | Took upstream. Re-applied: `pub mod c0_host;` and the `c0` field. Re-applied the construction in `app.rs` and the strip call in `app/frame.rs` (upstream moved both) |
| `crates/ui-egui/src/dialogs.rs` (was `panels/dialogs.rs`) | content (rename + upstream rewrite) | Took upstream. Re-applied the About helper call. The SAM hunks moved with upstream's `dialogs/sam_model.rs` and were re-applied there |
| `crates/ui-egui/src/credits.rs` → `dialogs/credits.rs` | auto-merged (rename) | Our one line landed in the renamed file |

Nothing was dropped. Every hook is still in place. None was moved out to a `c0_*` file: each one is a call site inside upstream code (a struct field, a constructor, a match arm, a frame call), so there was no non-invasive alternative. The debt listing in `PATCH-DEBT.md` records the new paths.

Moved hooks cost the same lines in their new files (construction and strip call: 4 lines before, 4 after).

## Final-position review

Every one of our hunks was read in its final position against upstream: `app.rs`, `app/frame.rs`, `lib.rs`, `control.rs`, `dialogs.rs`, `dialogs/sam_model.rs`, `dialogs/credits.rs`, `links.rs`, `menus.rs` (both the `ui_enabled` guard and the `menu_entries` filter), `panels/{bottombar,empty_library,strip,topbar}.rs`, `theme.rs`, `apps/lightcraft-web/{Cargo.toml,src/lib.rs,src/web.rs,src/workers.rs}`, `crates/ui-egui/Cargo.toml`, `xtask/src/web.rs`. All are in the right function. None needed moving.

## Patch debt

- Before (c0/main vs 38f8d250): 67 added lines in 18 modified upstream files.
- After (fix vs 37b2e523): **61** added lines in 21 modified upstream files. Ratchet 67 not exceeded.

Changes:
- `Cargo.lock` 7 → 1. Our old pin on base64 0.22.1 created a second copy and rewrote three other lock edges. Upstream already locks base64 0.23.1, so our requirement is now `0.23`. The page transport only uses the `Engine`/`STANDARD` API, which 0.23 keeps. The c0 base64 tests pass.
- `panels/dialogs.rs` (5) split into `dialogs.rs` (1) and `dialogs/sam_model.rs` (4).
- `credits.rs` (1) → `dialogs/credits.rs` (1).
- `lib.rs` 4 → 2; new rows `app.rs` (1) and `app/frame.rs` (1). Net for the two moved hooks: unchanged.
- All others unchanged.

No line growth was forced by upstream.

## Tests

Command set: `cargo test` for the two crates with suites, with the `page-control` feature for web. All under `smoac-slot run gate`, `CARGO_BUILD_JOBS=4`.

| Suite | Result |
|---|---|
| `lightcraft-ui-egui` lib | 1590 passed, 0 failed, 4 ignored |
| `lightcraft-ui-egui` `tests/c0_host.rs` | 20 passed |
| `lightcraft-ui-egui` `tests/c0_modes.rs` | 12 passed |
| `lightcraft-ui-egui` `tests/shutdown.rs` | 1 passed |
| `lightcraft-ui-egui` `tests/it` | 9 passed, 1 failed (upstream-red, below) |
| `lightcraft-ui-egui` doctests | 0 passed, 3 ignored |
| `lightcraft-web` (`--features page-control`) lib | 37 passed |
| `lightcraft-web` `tests/c0_page_control.rs` | 5 passed |

The `c0_*` test binaries were run in full. The c0 host has no separate Rust crate.

**Upstream-red:** `accessibility::every_interactive_node_has_a_name_in_every_view_and_tool` panics with `No glyph for "▸" (U+25B8) in Proportional`. The same failure reproduces on a clean worktree of 37b2e523 with the same command, so it is not ours. Likely cause: the craft-fonts checkout is absent here, so the fallback font set lacks that glyph. Not fixed.

**Our failures fixed during the run:** a non-exhaustive `Outcome` match in two test files (upstream added `Outcome::Gesture`) and in the page bridge; a deprecated `egui::Id::new` in two places, now `Id::unique` with fixed names (upstream's own idiom); a clippy single-element loop in a test.

## Build and lint

- wasm: `cargo check --target wasm32-unknown-unknown -p lightcraft-web --features page-control`: pass.
- Release wasm size: **not measured** (I did not build the page). The cap 29360128 is unchanged and unverified for this merge.
- clippy: `cargo clippy -p lightcraft-ui-egui -p lightcraft-web --features lightcraft-web/page-control --all-targets`: exit 0, no warnings from our code.
- rustfmt `--check` on every file we touched: clean.
- Host load: 25.3 (1 min) while the gate was queued at 13:51; 25.2 after the first full ui-egui run; 12.3 at the end of the web run. Not recorded for every step.

## New upstream commands

Command IDs that exist upstream but not at 38f8d250 (diffed across the whole crate namespace):

- `app.checkForUpdates` (Help ▸ "Check for Updates…")
- `keyword.nextSet`, `keyword.previousSet` (Alt+0 / Alt+Shift+0)
- `panel.addKeywords` (Cmd/Ctrl+K)
- `print.template.export`, `print.template.import`
- `view.infoSelectedPhotos`

Upstream also adds new menu entries that reuse existing command IDs: `library.shuffle`, `library.cancelPreviews`, `library.clearPreviews`, `view.theme`, `view.appearance`, `view.sidebarSection`, `view.sidebarReset`, `view.chooseLayoutImage`, `view.layoutImageOpacity`, `view.layoutImageMatte`, `view.toolOverlay`, `view.gridInfo`, `app.openLibrary`, `app.clearRecentLibraries`, `file.addFromDevice`, `metadata.applyPreset`, `dialog.labelNames`, `photo.rate`, `photo.flag`, `photo.label`, `library.sort`.

## What upstream added

Of 227 commits since 38f8d250: about 103 are refactors and most of the rest are fixes or PR merges.

- **Feature / behaviour:** egui 0.37 migration. Docking and the workspace move to craft-ui; side panels can sit in dock columns; the title bar uses craft-window's title_bar. A per-user default install folder and install-scope policy (installer, desktop only). An opt-in startup update prompt with self-update (desktop only, off by default). The Info panel can show metadata for the selected photos. Print templates can be exported and imported. A manual slideshow waits for arrow keys. A Sony α7R V camera profile.
- **Performance:** export speed (serial work cut from raw development and GPU transfers; 8-bit encoding without copies or libm calls).
- **Fixes in the crash / freeze / save / undo area:** queued file drops survive dialogs; a print template change that can't be saved is undone; a dropdown menu no longer stays shrunk after an empty search; several Originals fixes (Windows file identity, hard links, refusing a screenshot target whose original can't be read); Flatpak sources restored.
- **Security:** bounded memory for hostile RAW and HEIF input (several commits).

## Clashes with our UI

1. **Help ▸ "Check for Updates…"** (`app.checkForUpdates`) stays visible on the hosted page. The community-link guard covers only URL-backed entries. On wasm the updater is compiled out, but I did not verify what the menu item does there. Decide whether it should be hidden on the page.
2. **Title bar:** upstream now drives the drag area from craft-window. Our `Chrome.title_bar` / `top_bar` guards still compile; behaviour in the browser was not checked.
3. **Docking and side panels:** upstream's docking changes are not covered by our `c0_panels` seams. Not checked in a browser this run.
4. **Theme:** our `theme/host.rs` keeps its own egui id (`Id::unique("c0light-theme-tokens")`). The egui 0.37 change to `Id` was applied to it.
5. **Agent server / community links / Home:** no changes to those seams in this merge beyond the ones above.

## Assumptions I could not confirm

- No browser run this pass: the Chromium smoke and screenshot inspection from the earlier lane were not repeated. The glyph test is the only font-related check, and it fails upstream too.
- The release wasm size and the 29360128 cap are unverified for this merge.
- craft-fonts is absent, so CJK and missing-glyph paths are not exercised; the upstream-red glyph test is in this set.
- `LANE-REPORT.md` (the older lane report) has a stale debt table: it lists two engine files and different per-file counts that no longer match the measured ledger. I did not rewrite it; this report and `PATCH-DEBT.md` are authoritative for this merge.
- `theme/host.rs` is a new file outside the `c0_*` naming rule. It was already in c0/main and is not counted as debt; I left it as is.
- The README still references `docs/brand/` logos, which this fork does not carry. Unchanged from before this merge.
- `apps/lightcraft-web` `page_control` is feature-gated, so its native behaviour is covered only through the `c0_page_control` tests.
