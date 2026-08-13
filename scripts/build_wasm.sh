#!/usr/bin/env bash
# Build the browser module for the website.
#
# The site serves without a build step; this script is what rebuilds the module
# when the Rust changes. The artifact under site/pkg/ is committed on purpose:
# a gitignored pkg/ is how a sibling project shipped an artifact that went 79
# commits stale without anything noticing.
#
# The size gate is inside this script rather than in a separate report, so a
# module that outgrows its budget fails the build. Raising the budget is a
# deliberate edit with a line in the raise history below, never a silent bump.
#
#   raise history
#   2026-08-12  initial budget 220 KB gzip, set from the first green build
#
set -euo pipefail

cd "$(dirname "$0")/.."

BUDGET_GZIP_BYTES=225280   # 220 KB
OUT_DIR="site/pkg"

if ! command -v wasm-pack >/dev/null 2>&1; then
    echo "wasm-pack is required: cargo install wasm-pack" >&2
    exit 1
fi

echo "==> building mic-wasm for wasm32-unknown-unknown"

# --no-default-features keeps the Franken* adapters out of the browser build;
# they are feature-gated git dependencies and the browser needs none of them.
wasm-pack build crates/mic-wasm \
    --release \
    --target web \
    --out-dir "../../${OUT_DIR}" \
    --out-name mic \
    -- --no-default-features

# wasm-pack writes a .gitignore into the output directory, which would hide the
# very artifact the site serves. Remove it every time.
rm -f "${OUT_DIR}/.gitignore"

# wasm-pack already runs wasm-opt at -O on a release build. This second pass is
# specifically for size (-Oz), and it enables the same six-feature browser
# baseline the module is compiled against, so an optimiser that does not know a
# feature cannot quietly reject the module or strip something it needs.
if command -v wasm-opt >/dev/null 2>&1; then
    echo "==> wasm-opt -Oz (size pass)"
    wasm-opt -Oz \
        --enable-bulk-memory --enable-mutable-globals --enable-nontrapping-float-to-int \
        --enable-sign-ext --enable-reference-types --enable-multivalue \
        -o "${OUT_DIR}/mic_bg.size.wasm" "${OUT_DIR}/mic_bg.wasm"
    cp "${OUT_DIR}/mic_bg.size.wasm" "${OUT_DIR}/mic_bg.wasm"
    rm -f "${OUT_DIR}/mic_bg.size.wasm"
else
    echo "wasm-opt not found; shipping the wasm-pack module unchanged" >&2
fi

RAW=$(wc -c < "${OUT_DIR}/mic_bg.wasm" | tr -d ' ')
GZIP=$(gzip -9 -c "${OUT_DIR}/mic_bg.wasm" | wc -c | tr -d ' ')

printf '==> module size: %s bytes raw, %s bytes gzip (budget %s)\n' \
    "${RAW}" "${GZIP}" "${BUDGET_GZIP_BYTES}"

if [ "${GZIP}" -gt "${BUDGET_GZIP_BYTES}" ]; then
    echo "SIZE GATE FAILED: ${GZIP} > ${BUDGET_GZIP_BYTES} bytes gzip." >&2
    echo "Shrink the module or raise the budget deliberately, with a line in the raise history." >&2
    exit 1
fi

# Record what the module was built from.
#
# Committing the artifact makes staleness visible but does not make it detectable:
# nothing fails when someone edits the engine and does not rebuild, and the module
# the site serves then quietly stops being the code the repository describes. That
# already happened here — the committed module lagged its own source by five hours
# and the engine it calls by eight, while every check passed.
#
# The digest below covers every crate the module links, so `check_repo.py` can compare
# it against the current sources and fail when they diverge. `SOURCE_REVISION` is
# advisory; the digest is the part that decides.
SOURCE_CRATES=(mic-wasm mic-core mic-data mic-design mic-engine mic-sim)
DIGEST_INPUT=""
for crate in "${SOURCE_CRATES[@]}"; do
    while IFS= read -r file; do
        DIGEST_INPUT="${DIGEST_INPUT}$(shasum -a 256 "${file}" | cut -d' ' -f1)"
    done < <(find "crates/${crate}" -name '*.rs' -o -name 'Cargo.toml' | sort)
done
SOURCE_DIGEST=$(printf '%s' "${DIGEST_INPUT}" | shasum -a 256 | cut -d' ' -f1)
SOURCE_REVISION=$(git rev-parse HEAD 2>/dev/null || echo unknown)

cat > "${OUT_DIR}/BUILD_INFO.json" <<JSON
{
  "schema_version": "1.0.0",
  "source_digest_sha256": "${SOURCE_DIGEST}",
  "source_revision": "${SOURCE_REVISION}",
  "source_crates": ["mic-wasm", "mic-core", "mic-data", "mic-design", "mic-engine", "mic-sim"],
  "raw_bytes": ${RAW},
  "gzip_bytes": ${GZIP}
}
JSON

printf '==> source digest %s\n' "${SOURCE_DIGEST}"

echo "==> ok. Regenerate the manifest before committing:"
echo "    uv run python scripts/generate_repository_manifest.py"
