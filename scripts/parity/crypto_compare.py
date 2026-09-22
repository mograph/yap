"""Fails unless Rust and Swift derive the same keys, judge passphrases the same, and each opens
exactly the library the other sealed."""
import json, sys

fixture, work = json.load(open(sys.argv[1])), sys.argv[2]
load = lambda name: json.load(open(f"{work}/{name}"))
bad = 0

rust, swift = load("derive-rust.json"), load("derive-swift.json")
for r, s in zip(rust["keys"], swift["keys"]):
    for field in ("k16", "k32", "vault"):
        if r[field] != s[field]:
            bad += 1
            print(f"✗ {field} for {r['pass']!r} / {r['salt']!r}\n  rust : {r[field]}\n  swift: {s[field]}")
for r, s in zip(rust["checks"], swift["checks"]):
    if r["verdict"] != s["verdict"]:
        bad += 1
        print(f"✗ passphrase rule for {r['pass']!r}\n  rust : {r['verdict']!r}\n  swift: {s['verdict']!r}")
print(f"{len(rust['keys']) * 3} derived keys and {len(rust['checks'])} passphrase verdicts compared")


def norm(lib):
    """What matters, not formatting: an empty `list` may be written or left out, and a float
    may come back through an f32."""
    lib = json.loads(json.dumps(lib))
    for d in lib["history"]:
        d.setdefault("list", "")
        d["keptPct"] = round(float(d["keptPct"]), 3)
        d["audioSecs"] = round(float(d["audioSecs"]), 3)
    return lib


want = norm(fixture["library"])
for name in ("rust-opened-by-swift.json", "swift-opened-by-rust.json"):
    if norm(load(name)) == want:
        print(f"✓ {name[:-5].replace('-', ' ')}: the same library that went in")
    else:
        bad += 1
        print(f"✗ {name} doesn't match the library that was sealed")
print("Crypto agrees." if not bad else f"{bad} crypto differences")
sys.exit(1 if bad else 0)
