#!/usr/bin/env bash
# Runs the Mac's Rust cleanup and list logic and the iPhone's Swift port over the same sentences
# and fails on any difference. The two are meant to be the same program in two languages.
#
#   scripts/ios-parity.sh              # the committed corpus
#   scripts/ios-parity.sh --history    # plus every dictation in this Mac's Yap history (read-only)
set -euo pipefail
cd "$(dirname "$0")/.."

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

cp scripts/parity/corpus.json "$work/in.json"
if [[ "${1:-}" == "--history" ]]; then
  history="$HOME/Library/Application Support/io.tinkerstudio.yap/history.json"
  python3 - "$work/in.json" "$history" <<'PY'
import json, sys
said = json.load(open(sys.argv[1]))
for d in json.load(open(sys.argv[2])):
    if d.get("raw", "").strip() and d["raw"] not in said:
        said.append(d["raw"])
json.dump(said, open(sys.argv[1], "w"), ensure_ascii=False)
print(f"{len(said)} inputs, including this Mac's history")
PY
fi

echo "Rust…"
cp scripts/parity/profiles.json "$work/profiles.json"
(cd src-tauri && YAP_PARITY_IN="$work/in.json" YAP_PARITY_OUT="$work/rust.json" YAP_PARITY_PROMPTS="$work/profiles.json" \
  cargo test --quiet --lib parity_dump -- --ignored >/dev/null)

echo "Swift…"
swiftc -O -o "$work/parity" \
  ios/Shared/Store.swift ios/Yap/Polish.swift ios/Yap/Lists.swift scripts/parity/main.swift
"$work/parity" "$work/in.json" "$work/swift.json" "$work/profiles.json"
python3 - "$work/profiles.json" <<'PY'
import json, sys
rust, swift = (json.load(open(sys.argv[1] + ext)) for ext in (".rust", ".swift"))
same = sum(r == s for r, s in zip(rust, swift))
for i, (r, s) in enumerate(zip(rust, swift)):
    if r != s:
        print(f"✗ speaker prompt {i} differs\n--- rust\n{r}\n--- swift\n{s}")
print(f"{same}/{len(rust)} speaker prompts identical")
sys.exit(0 if same == len(rust) else 1)
PY

python3 - "$work/rust.json" "$work/swift.json" <<'PY'
import json, sys
rust, swift = (json.load(open(p)) for p in sys.argv[1:3])
fields = ["clean", "list", "groups", "listDirect", "groupsDirect", "words", "keptPct"]
bad = 0
for r, s in zip(rust, swift):
    for f in fields:
        if r[f] != s[f]:
            bad += 1
            print(f"\n✗ {f} differs for: {r['input'][:90]!r}")
            print(f"  rust : {r[f]!r}")
            print(f"  swift: {s[f]!r}")
total = len(rust) * len(fields)
print(f"\n{total - bad}/{total} fields agree across {len(rust)} inputs")
sys.exit(1 if bad else 0)
PY

echo
echo "Crypto: key derivation and passphrase rules…"
swiftc -O -o "$work/crypto" \
  ios/Shared/Store.swift ios/Yap/Polish.swift ios/Yap/Lists.swift ios/Yap/Argon2.swift ios/Yap/CloudCrypto.swift \
  scripts/parity/crypto/main.swift
rust_crypto() {
  (cd src-tauri && YAP_CRYPTO_MODE="$1" YAP_PARITY_IN="$2" YAP_PARITY_OUT="$3" \
    cargo test --quiet --lib parity_crypto -- --ignored >/dev/null)
}
fixture=scripts/parity/crypto.json
rust_crypto derive "$PWD/$fixture" "$work/derive-rust.json"
"$work/crypto" derive "$fixture" "$work/derive-swift.json"

echo "Crypto: sealing on each side and opening on the other…"
rust_crypto seal "$PWD/$fixture" "$work/sealed-by-rust.json"
"$work/crypto" seal "$fixture" "$work/sealed-by-swift.json"
python3 scripts/parity/crypto_pairs.py "$fixture" "$work"
"$work/crypto" unseal "$work/open-rust.json" "$work/rust-opened-by-swift.json"
rust_crypto unseal "$work/open-swift.json" "$work/swift-opened-by-rust.json"
python3 scripts/parity/crypto_compare.py "$fixture" "$work"
