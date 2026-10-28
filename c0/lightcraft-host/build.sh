#!/usr/bin/env bash
# Use upstream's bundler, with the optional page bridge enabled. No alternate binary.
set -euo pipefail
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
cd "$root"
export CARGO_BUILD_JOBS=2
export BINARYEN_CORES=2
if [[ -n ${CRAFT_FONTS_DIR:-} && ! -d $CRAFT_FONTS_DIR ]]; then
  echo "C0light: CRAFT_FONTS_DIR is missing; building without CJK fonts." >&2
  unset CRAFT_FONTS_DIR CRAFT_FONTS_REQUIRED
fi
run_slot() {
  local result=0
  smoac-slot run heavy 2 3600 -- "$@" || result=$?
  # Admission refusals are 13 (capacity/load) or 75 (deferred/unavailable).
  # Cargo compilation failures use 101 and are never retried in a different pool.
  if [[ $result == 13 || $result == 75 ]]; then
    smoac-slot run gate 3 3600 -- "$@"
  else
    return "$result"
  fi
}
echo "Load before build: $(cat /proc/loadavg)"
run_slot cargo xtask web --page-control "$@"
target_dir=${CARGO_TARGET_DIR:-target}
site="$target_dir/web-site"
out="$root/dist/c0host"
mkdir -p "$out"
# Only clean the directory owned by this page.
find "$out" -maxdepth 1 -type f -delete
/bin/cp -rf "$site"/. "$out/"  # upstream now also writes snippets/ (wasm-bindgen inline JS)
/bin/cp -f c0/lightcraft-host/index.html "$out/index.html"
/bin/cp -f LICENSE-MIT LICENSE-APACHE NOTICE assets/fonts/OFL-Inter.txt "$out/"
# Upstream's compressed HTML describes its own page and must not shadow the guest.
rm -f "$out/index.html.gz" "$out/index.html.br"
if [[ -f $out/lightcraft_zh_hans.otf ]]; then
  printf '%s\n' '{"chineseFont":"./lightcraft_zh_hans.otf"}' > "$out/guest-config.json"
else
  printf '%s\n' '{"chineseFont":null}' > "$out/guest-config.json"
fi
echo "Load after build: $(cat /proc/loadavg)"
echo "C0light site: $out"
