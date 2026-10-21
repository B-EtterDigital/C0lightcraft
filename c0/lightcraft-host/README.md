# C0light: LightCraft as a C0VIBE guest page

This page builds the **existing `apps/lightcraft-web` library**, with the optional
`page-control` feature. There is no separate app binary and no iframe transport.
LightCraft 0.10 uses `cargo xtask web` (Cargo, wasm-bindgen, optional wasm-opt), not
Trunk, so this directory supplies `build.sh` instead of a nonfunctional Trunk file.

```sh
./c0/lightcraft-host/build.sh           # optimized upstream web profile → dist/c0host/
python3 -m http.server --directory dist/c0host 18080
```

Run from this repository or invoke the script by absolute path. Cargo uses two
jobs, under `smoac-slot run heavy 2 3600`; an admission refusal falls back to
`gate 3 3600`. It never retries compiler failures as slot refusals. Install the
wasm-bindgen version pinned in Cargo.lock and the wasm32 target as upstream
documents in `docs/web.md`. `--dev` is supported for iteration. Do not pass
`--serve`: serve the assembled guest output separately.

All module, worker and optional font URLs are relative (`./`). Upstream bundling
keeps stable file names rather than Trunk's file hashes; serve HTML/config without
long-lived caching and deploy JS/WASM together. Compression siblings from upstream
are retained, except compressed upstream HTML, which would shadow our page.
The assembled site also carries the project notices and Inter's OFL text.
Serve `.wasm` as `application/wasm`. HTTPS or localhost is required. COOP
`same-origin` and COEP `require-corp` are recommended, as for upstream.

If `CRAFT_FONTS_DIR` names a missing directory, the script unsets it for this
build and reports the omission. A font-free build works but lacks Chinese and
Japanese fallback glyphs. With craft-fonts present, upstream ships its Chinese
font and OFL text as **separate files**; the generated `guest-config.json` tells
the page whether to fetch it. No Adobe assets, HEIC/HEVC feature, GPL RawNIND
weights or model downloads are enabled by this build.

The HTML/body stay transparent until egui paints. No demo photos, sample library
or host branding UI is loaded. A fresh browser library starts empty. Existing
user imports reopen from upstream's OPFS/IndexedDB storage; the page preserves
the normal render workers and URL options (`?store=memory`, `?workers=N`, etc.).
The compiled module lives at `lightcraft.module`; there is no extra
`lightcraftModule` global. Memory-only file rendering falls back to the main
thread because workers cannot read ephemeral originals. Direct upstream builds without the
feature keep their storage, demo, chrome and worker behaviour.
Browser boot also installs a real import clock; the engine's deterministic test
clock must not timestamp photos imported in a browser session.

## Page function and envelopes

Wait until `typeof window.__lightcraftControl === "function"`, then call:

```js
const reply = JSON.parse(await window.__c0hostRequest(JSON.stringify({
  method: "c0.inspect", params: {}
})));
```

The alias is exactly `window.__c0hostRequest = (json) =>
window.__lightcraftControl(json)`. Both accept **JSON text** and return a Promise
of JSON text: `{"ok":true,"result":...}` or `{"ok":false,"error":"..."}`.
Missing or null params become `{}`; other non-object params are rejected.
Malformed input returns an error envelope. There is no postMessage listener,
origin negotiation or global beyond `lightcraft`, `__lightcraftControl` and the
host alias (plus browser-provided DOM names).

Ordinary requests are capped at 1 MiB; `c0.openFile` requests at
100 MiB (104,857,600 bytes), matching the reference transport. Files are capped at
72 MiB. Native/memory-only host originals have a 288 MiB session budget;
persistent browser originals use upstream's cache and browser storage quota.
At most four original-storage operations may be pending. Names must be nonempty
basenames, at most 255 UTF-8 bytes, without slash, backslash or NUL. Bytes use
canonical, padded standard base64. Pointer click counts are capped at 256,
drag steps at 32, pointer arrays at 256 entries. Invalid values are refused
before input events are queued.

## When a request is answered

`__lightcraftControl` wraps the **existing `lightcraft.command` export / browser
INBOX**, using one reserved command to reach `control::handle`. The existing
command API still accepts command ids and JSON params and retains its original
Promise/rejection contract. `web.stats` is still available there.

Each page call borrows an **unpresented egui pass inside the call** via eframe's
WebRunner event listener. It can answer while animation frames and timers are
paused. Synthetic UI input borrows up to 40 additional passes so queued input
can take effect before the next presented frame. Read methods schedule no extra
passes; writes request a later repaint. Original storage/loading is asynchronous,
and its completion also pumps an unpresented pass. Rendering and encoding are
synchronous and may take time for large photos. Viewport screenshots return an unsupported error on the page;
capture the canvas/page through the browser instead.

## Host methods

| Method | Params | Result |
|---|---|---|
| `c0.inspect` | `{}` | Library, photo, history, panes and theme state described below |
| `c0.goTo` | `{photo?: id, collection?: id, view?: name}` | `{photo,collection,view}`; omitted collection is null |
| `c0.documentBytes` | `{format?: "jpeg"\|"png"\|"tiff"\|"xmp", photo?: id}` | `{name,format,bytes,revision,photo}`; default JPEG and active photo |
| `c0.openFile` | `{name,bytes}` | Engine import report `{imported,duplicates,failed,scanned,...}` |
| `c0.newDocument` | `{template?: "blank"}` | `{photoCount:0,template:"blank"}`; empty/omitted template also means blank |
| `c0.cancelFileDialog` | `{}` | `{cancelled:boolean}` |
| `c0.set` | `{themeTokens?,panels?}` | `{applied:string[],unknown:string[],invalid:string[]}` |

`c0.inspect` returns these fields (illustrative photo/collection):

```json
{
  "revision": 3, "dirty": false, "canUndo": true, "canRedo": false,
  "photoCount": 1, "selectedCount": 1,
  "activePhoto": {"id":1,"name":"photo.png","rating":3,"flag":"none"},
  "view": "detail", "collections": [{"id":1,"name":"Album","count":1}],
  "importing": false, "exporting": false, "unsaved": null,
  "theme": "charcoal", "fileDialog": false,
  "panes": {"title_bar":false,"top_bar":false,"start_screen":false,
    "community_links":false,"experimental_notice":false,"tool_strip":true,
    "bottom_bar":true,"left_panel":true,"filmstrip":true,"histogram":true,
    "filter_bar":true,"navigator":true}
}
```

`revision` is the catalog revision, scoped to its session and reset for a new
document. LightCraft automatically journals library edits: **dirty means a
failed save with unlogged operations**, not simply an edit since byte export.
`unsaved` is null or `{ops,error}`; an asynchronous browser flush failure adds
`storage:true` with `ops:0`. Memory sessions are ephemeral; `dirty:false`
does not mean the outer host saved them. The outer host should compare revisions
and persist both its chosen raster and XMP as needed. `activePhoto` is null
without an active photo; collections are selectable catalog albums, including
smart albums. `theme` is the current preset id: charcoal/midnight/silver/paper.
`importing` includes scans and Lightroom imports; `exporting` tracks background
export, while synchronous byte export finishes before its reply.

`goTo` accepts the real ViewMode names: `photoGrid`, `squareGrid`, `detail`,
`compare`, `survey`, `reference`, `people`; `grid` aliases `photoGrid`. Photo and
collection ids must exist, and a photo specified with a collection must belong
to it. Every argument is validated before changing navigation. It uses
`library.source` and `library.select`; photo-only navigation switches to All
Photos and clears filters so the target is visible. It does not silently
manufacture a second comparison/reference photo.

`openFile` **adds** an original to the current library through engine probing
and `library.import`; it does not replace the library. Use `goTo` for explicit
photo navigation. Probe/validation failures leave the catalog in place. Engine
duplicates and per-file import errors are preserved in its report. RAW, JPEG,
PNG, TIFF, WebP, PSD, JXL, GIF and BMP use upstream import support. HEIC is not
enabled. Upstream's picker accept-list omission for CR3/RAF/RW2/ORF/PEF/RWL/HEIC
is corrected separately; selecting a disabled codec still returns an error.

Before Open/New, the normal library transition checks refuse active import,
export or merge tasks. Failed journal writes are retried with the existing
closing-snapshot recovery path. If saving still fails, an error is returned and
the library stays intact; no host operation force-discards unsaved changes.
New clears the catalog, history and rendering state only after those checks.
The native/browser library being left stays saved; the new guest library is
temporary, and reload reopens the prior saved browser library. Whole-library
backup is disabled for that temporary library. Host byte imports require a memory document: native disk libraries
refuse them rather than journal references to ephemeral originals. Use normal
file-path import for those libraries, or create a blank guest document first.
Persistent browser imports write the original before its catalog reference;
storage failures return an error. Late completions cannot import into a changed
document. The browser picker is asynchronous; `fileDialog` covers the picker
and its read. Cancellation invalidates its generation, so late reads cannot
import. Native picker replies are abandoned too. Browser security does not
allow forcibly closing the OS chooser; cancelling makes its eventual answer
inert.

`documentBytes` is **read only**: it neither saves, marks clean, changes history
nor changes develop settings/selection. Raster formats use
`lightcraft_engine::export::export_photo`, the normal full-size develop/export
pipeline and encoder. XMP uses the engine's full sidecar packet. It never calls
the download writer. These are per-photo snapshots, **not a serialized whole
photo-library bundle**. XMP retains editable settings; reopening a rendered
raster imports its baked appearance. JPEG/PNG/TIFF are the host raster outputs;
upstream's other UI exports retain their platform limits (AVIF encoder is not
available on wasm; JXL encoding is native-only).

## Chrome settings

All `Chrome` fields default true in the upstream app and are not serialized.
The page opts out of title/top bars, Start/Home, community links and the
experimental startup notice. The host can set every field at runtime:

`title_bar`, `top_bar`, `start_screen`, `community_links`,
`experimental_notice`, `tool_strip`, `bottom_bar`.

Host setters treat `title_bar` and `top_bar` as aliases for the same in-window
bar and synchronize both switches; send one of these keys per request. The notice switch controls the
startup notice, not storage-error notices. Existing UiState panel switches are
also supported: `left_panel`, `filmstrip`, `histogram`, `filter_bar`, `navigator`.
Values must be booleans. Community-off hides Help-menu project links, the
Discord icon, About promotions, model-guide links and contributor profile links; direct project
link commands are refused. About still identifies LightCraft, the licence,
fonts, original icons and named contributors. The host can access all hidden
menu commands through the ordinary control methods.

## Runtime theme tokens

`themeTokens` supports every **Color32** field from upstream `Tokens`:

`chrome`, `canvas`, `grid_bg`, `cell`, `cell_selected`, `divider`, `inset`,
`field`, `field_border`, `button`, `button_border`, `hover`, `pressed`,
`tool_active`, `text`, `text_label`, `text_dim`, `text_disabled`, `icon`,
`track`, `thumb`, `thumb_hover`, `accent`, `star`, `pick`, `reject`,
`caution`, `mask_overlay`.

Colours are `#RRGGBB` or `#RRGGBBAA`; null removes an override. Dimensions are
not colour tokens. Overrides use their own egui id, survive light/dark preset
changes and are isolated between contexts. Egui's own widget palette and
LightCraft tokens receive the same overrides. Use existing appearance commands
to select a theme preset. Reporting keys are fully qualified:
`themeTokens.accent`, `panels.filmstrip`, etc. A malformed whole group is reported
as `themeTokens` / `panels`; valid sibling keys still apply.

## Ordinary control methods and verification

`engine.execute`, `engine.commands`, `ui.inspect`, `ui.set`, `ui.menu.list/tree`,
`ui.menu.invoke`, `ui.widgets`, synthetic keys/pointers and dialog controls use
the original handler. See `docs/control-protocol.md`. Unknown methods return an
error envelope. Hidden menu bars do not delete editing commands or panels.

Native contracts: `cargo test -p lightcraft-ui-egui -p lightcraft-engine
--features lightcraft-engine/denoise` (the desktop app's default runtime) and
`cargo test -p lightcraft-web --features page-control` (K-41). Also check the
wasm target with that feature, clippy on both touched crates, full UI/engine
suites and `cargo xtask ci`. Use the required slot wrapper and two Cargo jobs.
The runtime-free UI suite has six upstream model-setup tests that require that
native runtime; workspace CI enables it through the desktop package. It stays
off in the guest build, as does the separate `rawnind-model` feature.

`smoke.mjs` serves the actual dist and drives headless Chromium, including
calls with animation frames/timers paused, a real procedural PNG, all host
methods and formats, cancellation, a real browser picker, persistent reload,
memory-only rendering and screenshots:

```sh
npm install --prefix /tmp/c0light-browser playwright
PLAYWRIGHT_MODULE=/tmp/c0light-browser/node_modules/playwright/index.mjs \
  node c0/lightcraft-host/smoke.mjs
```

`PATCH-DEBT.md` records all modified upstream files and the added-line ratchet.
`LANE-REPORT.md` records measured build/gate/browser results and limitations.
Never commit generated `dist/`, photos or screenshots. The controller collects
lane commits; do not push or commit on upstream `main`.
