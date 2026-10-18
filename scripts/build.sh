#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
cargo build --manifest-path engine/Cargo.toml --locked --target wasm32-unknown-unknown --release
wasm-bindgen engine/target/wasm32-unknown-unknown/release/spoofed_web.wasm --target web --out-dir site/pkg
python3 - <<'PY'
from pathlib import Path
files=list(Path('site/pkg/snippets').glob('*/transport.js'))
if len(files)!=1: raise SystemExit('Expected one transport module')
Path('site/transport-entry.js').write_text('export { openUSB, closeUSB } from "./'+str(files[0].relative_to('site'))+'";\n')
PY
