# Patch debt: C0light integration

Our implementation lives in new `c0_host`, `page_control`, theme/host and C0 test
files. This table accounts for every modified upstream file, including replaced
and reindented lines. New files are excluded by the requested modified-file
ratchet.

Measurement (`Lines` is the added column):
`git diff --numstat --diff-filter=M main c0/main -- . ':!c0'`.
Before committing, omit `c0/main` to measure the working tree instead.

| File | Lines | What | Why | Offered upstream |
|---|---:|---|---|---|
| `Cargo.lock` | 7 | Pin base64 0.22.1 and add the UI dependency edge | Reproducible pure-Rust file transport | No |
| `apps/lightcraft-web/Cargo.toml` | 2 | Optional page-control feature; serde_json borrowed raw params | Native K-41 and bounded large file envelopes | No, host transport |
| `apps/lightcraft-web/src/lib.rs` | 2 | Feature-gated page_control module | Keep ordinary web build unchanged | No |
| `apps/lightcraft-web/src/web.rs` | 12 | Picker accept-list correction (1); real browser import clock (1); feature-controlled demo/notice, attach, queue dispatch and runner hooks (10) | Correct web bugs; wrap the existing browser INBOX and avoid hosted demos | Picker extension and clock fixes: **yes, as is**; host hooks: no |
| `apps/lightcraft-web/src/workers.rs` | 4 | Feature-selected compiled-module location (3); memory-only File rendering fallback (1) | Self-contained module; workers cannot read originals without storage | Fallback: **yes, as is**; module hook: no |
| `crates/engine/src/tests_slide_template.rs` | 1 | Invalidate listing after external fixture writes | Avoid same-clock-tick directory-mtime flake; production unchanged | **Yes, as is** |
| `crates/engine/src/face_download.rs` | 1 | Use web_time in the download manager destructor | Dropping any session panicked on wasm, even without a download | **Yes, as is** |
| `crates/ui-egui/Cargo.toml` | 1 | Pure-Rust base64 dependency | Bounded standard base64 file transport | No |
| `crates/ui-egui/src/control.rs` | 1 | One c0. prefix arm | Reuse the app's control handler | No |
| `crates/ui-egui/src/credits.rs` | 1 | Policy-aware contributor label/link | Keep names while removing GitHub profile links | No |
| `crates/ui-egui/src/lib.rs` | 3 | Module, per-app host state and default construction | Unsaved host policy; upstream defaults intact | No |
| `crates/ui-egui/src/links.rs` | 3 | Guard community URLs | Prevent hidden promotions from being opened by command | No |
| `crates/ui-egui/src/menus.rs` | 4 | Disable and omit community entries | Menu/list consistency | No |
| `crates/ui-egui/src/panels/bottombar.rs` | 3 | Default-true chrome guard | Optional host-owned bottom bar | No |
| `crates/ui-egui/src/panels/dialogs.rs` | 5 | About helper (1); guard model guide (3); plain licence label when links hidden (1) | Cover every community/GitHub link; preserve licence text and upstream defaults | No |
| `crates/ui-egui/src/panels/empty_library.rs` | 3 | Default-true Start/Home guard | The host owns the guest empty screen | No |
| `crates/ui-egui/src/panels/strip.rs` | 3 | Default-true tool-strip guard | Optional host chrome | No |
| `crates/ui-egui/src/panels/topbar.rs` | 6 | Title/top guard and Discord guard | Host title bar; optional promotions | No |
| `crates/ui-egui/src/theme.rs` | 2 | New host module and one token-resolution replacement | Apply overrides to tokens and egui visuals across theme changes | No |
| `xtask/src/web.rs` | 3 | Forward --page-control as the app's feature | Build using the upstream bundler rather than inventing a second pipeline | No |

Total: 67 added lines in 20 modified upstream files.

Ratchet: 67

## Seams checked during porting

| Seam | Dependency on upstream behaviour | Evidence |
|---|---|---|
| Page bridge | WebApp INBOX drains in logic, eframe WebRunner lends unpresented event-listener passes | Bridge source; browser smoke must freeze animation frames and timers |
| Byte import | Engine probe/load hooks and library.import remain the only importer | Native round-trip tests and real browser PNG/picker smoke |
| Open/New | Library transitions retry journal writes and closing snapshots | Synthetic unplugged-store regression |
| Byte export | export_photo uses the complete develop/export path; sidecar_packet supplies XMP | Raster round trips and immutable-state assertions |
| Themes | apply_kind derives egui visuals from Tokens; overrides need their own id | Complete Color32-field registry and theme/context-isolation tests |
| Chrome | About promotions, credits, link commands, top icon and menus are separate surfaces | Policy checks and headless rendered About/chrome tests |

No Lightroom parity score or roadmap claim is changed by this host integration.
Final committed counts and runtime results belong in LANE-REPORT.md.
