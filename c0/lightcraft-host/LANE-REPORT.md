# C0light guest-page lane

Status: complete. The guest builds, runs in Chromium and passes the required gates.

## Commits

- Base/upstream `main`: `abb49bdd76e64acd20afe2e4d7bb1cf71c548d1d`.
- Implementation and report: `feat(c0light): add the LightCraft guest page` (this commit).
- Lane branch: `c0/main`. Nothing is pushed. The controller can identify this
  report's commit with `git log main..c0/main`.

## Delivered

The existing web binary builds with `page-control`. The page wraps its command
queue, borrows unpresented egui passes and supplies all seven host methods.
Fresh libraries are empty; OPFS/IndexedDB, render workers, regular commands,
editing panels and original application credits remain available. New creates
a temporary document and preserves the previous saved library.

All 28 colour tokens have per-context overrides. Chrome defaults remain true
outside the page. The hosted defaults hide title/Home/promotions/startup notice.
Community policy covers menus, commands, Discord, About, contributor links and
the model-install dialog. Raster/XMP byte snapshots use upstream encoders and
sidecar generation without saving or changing history.

Browser original I/O is bounded, generation checked and completed through the
same queue; picker cancellation invalidates late reads. Failed journal writes
and browser flush failures block Open/New rather than discarding changes.

## Validation

All Cargo commands use `smoac-slot` and `CARGO_BUILD_JOBS=2`.

| Check | Result |
|---|---|
| Native `cargo test -p lightcraft-web --features page-control` (K-41) | PASS: 22 upstream + 5 new tests; 0 failures |
| wasm32 check with `page-control` | PASS |
| Clippy UI/web/xtask, all targets, page-control, `-D warnings` | PASS |
| New native `c0_host` integration binary | PASS: 14 tests; 0 failures |
| Workspace tests, including doctests | PASS: 4,192 passed, 0 failed, 35 ignored |
| Upstream engine suite (desktop runtime enabled) | PASS: 1,147 passed, 6 ignored |
| Upstream UI suite | PASS: 1,112 passed, 4 ignored |
| Full workspace `cargo xtask ci` | PASS after the crash fix: all 8 gates; 885.9 seconds. Initial cold run also passed in 3,570.1 seconds |
| Wasm web clippy, page-control, `--no-deps -- -D warnings` | PASS; no host-module warnings |
| Optimized guest bundle | PASS: upstream web profile, wasm-bindgen, wasm-opt and compression |
| Chromium smoke and screenshot inspection | PASS: all 7 methods; picker/cancel; PNG/JPEG/TIFF/XMP; reload; 4 ready workers / 4 completed remote jobs; memory rendering; session replacement |

A preliminary engine run exposed a same-clock-tick fixture listing-cache flake.
One test-only invalidation fixes it; the subsequent engine run passed 1,133
tests, with six ignored. No production template behaviour changed.

The runtime-free UI run passed 1,106 tests and failed six upstream model-setup
tests because that command omitted `lightcraft-engine/denoise`. The workspace
gate enables the desktop app's default runtime. It remains disabled in the
guest bundle; `rawnind-model` and `heif` also remain disabled there.

Workspace CI recorded 191 capability skips: 137 corpus, 25 GPU, 20 fonts,
eight model and one system-ICC check. These are separate from the 35 ignored
harness cases. Vulkan exposes only llvmpipe; upstream deliberately skips it for
the GPU pipeline. The wasm dependency checks retain existing warnings in
native-only code; the page module's own wasm clippy check passes.

The browser regression found a real upstream wasm crash: dropping a session
called `std::time::Instant::now()` in the idle face-download manager destructor.
Changing that one line to `web_time` fixes it without changing native behaviour.
The smoke's synthetic PNG → New → reopen sequence and memory-session New cover
the previously crashing path; no panic guard is used to mask it.

## Build and browser measurements

Final corrected build, with the missing-font fallback exercised:

| Measurement | Result |
|---|---:|
| WASM raw | 24,555,603 bytes |
| WASM gzip | 8,741,409 bytes |
| WASM Brotli | 5,775,754 bytes |
| Entire `dist/c0host/` | 39,223,071 bytes, 15 files, including compressed siblings |
| JPEG / PNG / TIFF / XMP snapshot | 2,267 / 1,760 / 2,033 / 3,797 bytes |

`cat /proc/loadavg` immediately before the final build:
`8.86 15.67 18.66 5/2420 68029`.
Immediately after:
`11.99 13.69 17.23 9/2250 181399`.

Chromium showed an empty initial library, then the real procedural PNG and the
About window with licence/contributors preserved and promotions hidden. Browser
errors: zero. Reload recovered exactly one original and its exposure +0.5 edit;
OPFS reported no save error; all four render workers became ready and completed
four remote jobs. The separate `?store=memory` run rendered inline.
Screenshots inspected: `/tmp/c0light-browser/guest-empty.png` and `guest.png`.
The smoke report is `/tmp/c0light-browser/smoke.json`. Generated media,
screenshots and dist files are not committed.

The final script's `ui.inspect.perf` sample reported `updateMs:48`, `logicMs:47`,
`maxUpdateMs:116` and `frameMs:1` on this software-rendered session. This is
diagnostic script timing, not a slider-latency or hardware GPU benchmark.

Host: Linux x86_64, Rust 1.99.0, wasm-bindgen 0.2.129, Chromium 155.0.8059.79.
No craft-fonts checkout is present; the build omits CJK fallback glyphs.

## Upstream patch debt

Ratchet: 67. Target: under 70 added lines. New files are excluded by the
requested `--diff-filter=M` measurement; all modified upstream files are listed
in [PATCH-DEBT.md](PATCH-DEBT.md).

| Modified upstream file | Added lines |
|---|---:|
| `Cargo.lock` | 7 |
| `apps/lightcraft-web/Cargo.toml` | 2 |
| `apps/lightcraft-web/src/lib.rs` | 2 |
| `apps/lightcraft-web/src/web.rs` | 12 |
| `apps/lightcraft-web/src/workers.rs` | 4 |
| `crates/engine/src/tests_slide_template.rs` | 1 |
| `crates/engine/src/face_download.rs` | 1 |
| `crates/ui-egui/Cargo.toml` | 1 |
| `crates/ui-egui/src/control.rs` | 1 |
| `crates/ui-egui/src/credits.rs` | 1 |
| `crates/ui-egui/src/lib.rs` | 3 |
| `crates/ui-egui/src/links.rs` | 3 |
| `crates/ui-egui/src/menus.rs` | 4 |
| `crates/ui-egui/src/panels/bottombar.rs` | 3 |
| `crates/ui-egui/src/panels/dialogs.rs` | 5 |
| `crates/ui-egui/src/panels/empty_library.rs` | 3 |
| `crates/ui-egui/src/panels/strip.rs` | 3 |
| `crates/ui-egui/src/panels/topbar.rs` | 6 |
| `crates/ui-egui/src/theme.rs` | 2 |
| `xtask/src/web.rs` | 3 |
| **Total** | **67** |

## Assumptions I could not confirm

- C0VIBE's final multi-guest bundle and host integration are outside this
  checkout. This lane verifies the standalone page contract.
- `craft-fonts` is absent. Chinese/Japanese fallback rendering cannot be
  verified here; its optional font remains separate when provided.
- The gitignored `plan/` documents referenced by AGENTS.md are absent. The
  checked-in rules, roadmap, web/control documentation and reference wraps
  govern this implementation.
- Per-camera RAW fidelity, real-camera corpora and hardware GPU performance
  are outside this wrapping task. No Lightroom parity score is claimed.

Upstreamable fixes are marked as such in PATCH-DEBT.md; none was submitted
remotely. No Adobe assets, model weights or media fixtures were added.
