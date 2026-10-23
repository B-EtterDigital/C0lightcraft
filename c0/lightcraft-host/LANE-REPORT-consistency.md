# Lane report: tool modes and a foldable right dock (SOW consistency)

## Tool-mode decision

LightCraft's "toolbar" is the **tool strip**: the icon column at the far right, beside the develop
panels (Presets, Edit, Crop, Remove, Masking, Red Eye, Versions, History, More; Keywords and Info
anchored at the bottom). The app's habit is a fixed 48 pt column drawn by `panels/strip.rs`, with the
panel it opens docked to its left. It has no floating palette of its own. So:

- `toolbar_double`: the strip docked in two columns (96 pt); the buttons wrap two to a row, group rules
  take a full row, Keywords and Info sit side by side at the bottom.
- `toolbar_floating`: the same strip drawn by the same code inside a movable `egui::Area` (grip on top,
  constrained to the window below the title bar), beside the open dock by default. The docked panel is
  not drawn, so the canvas has the full width. Floating plus double is a two-column palette.
- `toolbar` is an alias of `tool_strip` (as `title_bar`/`top_bar` alias each other).
- `right_dock_collapsed`: closes the right dock the way the dock's own close does, remembering the open
  panel; the strip's icons still open it (any panel command ends the fold); `false` restores the
  remembered panel (Edit if the dock was already closed). The canvas widens at once: no animation, so no
  intermediate layout.

Nothing is saved; all default `false` (upstream). `c0.inspect().panes` reports `toolbar`,
`toolbar_double`, `toolbar_floating`, `right_dock_collapsed`.

## Commits (c0/main)

1. `a9a4b95` feat(c0light): tool-strip modes and a foldable right dock
2. `fcd514f` feat(c0light): tool-mode wrapper in the frame, README, patch debt, smoke

New code: `crates/ui-egui/src/c0_host/c0_strip.rs`, keys in `c0_panels.rs`, tests
`crates/ui-egui/tests/c0_modes.rs`. Upstream edits: `panels/strip.rs` (4), `lib.rs` (+1: the frame calls
`c0_host::c0_strip::show`). **Ratchet 69** (was 67; measured with
`git diff --numstat --diff-filter=M origin/main -- . ':!c0'`). The tool-strip guard moved out of
`strip.rs` into the wrapper to stay under 70.

## Tests

All Cargo runs under `smoac-slot run heavy 2`, `CARGO_BUILD_JOBS=2`.

| Check | Result |
|---|---|
| `tests/c0_modes.rs` (set, inspect, defaults = upstream, unknown/invalid, not saved, geometry of 1/2 columns docked and floating, fold/unfold, icon opens a folded dock) | 12 passed |
| `tests/c0_host.rs` | 14 passed |
| `lightcraft-ui-egui` other integration tests | 5+3+1 passed |
| `lightcraft-engine` unit tests (denoise feature) | 1142 passed, 6 ignored |
| `lightcraft-ui-egui` lib tests | 1111 passed, 1 failed under load, 4 ignored; the failure (`export_dialog_previews_the_watermark`) passes alone (see assumptions) |
| `cargo test -p lightcraft-web --features page-control` | 22 + 5 passed |
| clippy `-p lightcraft-ui-egui -p lightcraft-web --features page-control --all-targets` | clean |
| `cargo check -p lightcraft-web --features page-control --target wasm32-unknown-unknown` | pass |
| `c0/lightcraft-host/build.sh` | pass; `dist/c0host` |
| Headless Chromium `smoke.mjs` (now sets each mode and reads it back, plus an unknown key) | pass |

## wasm size

`lightcraft_web_bg.wasm` 24,559,718 bytes (24.6 MB, optimized; compressed siblings 8,743,809 and 5,779,024 bytes). I found no pin cap in the repo (`omastore.toml`,
`c0/`, `docs/web.md`, `xtask`), and the first lane did not record a size, so I cannot state margin.

## Machine load

Load average (1 min) before the first build 8.93; during tests 12.4; before the page build 16.9; after
the page build 20.24 (other lanes were running). Disk 79-80% throughout.

## Assumptions I could not confirm

- The wasm pin cap's value: not in the repo; size above is for the controller to compare.
- `export_dialog_previews_the_watermark` failed once in the full lib run (load ~20, 283 s) and passes
  alone; I did not run it against the base commit, so "flaky under load" is an inference. The strip
  change does not touch the export dialog.
- The remaining integration tests beyond `--test '*'` and the full `cargo xtask ci` were not run.
- Floating palette placement and dragging were verified by layout tests (inside the window, left of the
  dock, no overlaps) and the headless smoke's state round-trip, not by dragging in a real browser or a
  screenshot review.
- Palette heights (540/340 pt) are constants sized to the current button set; a taller strip would need
  them raised.
- A fold followed by a panel opened and closed again while the strip is hidden (`tool_strip:false`) is
  reconciled lazily (next `c0.set`/`c0.inspect`/frame with the strip), so it could report a stale fold
  in that corner.
- "Right dock" is the Tools panel; the Presets panel and the left panel have their own switches and are
  not folded.
