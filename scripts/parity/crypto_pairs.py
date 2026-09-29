"""Wraps each side's sealed library with the passphrase, ready for the other side to open."""
import json, sys

fixture, work = json.load(open(sys.argv[1])), sys.argv[2]
for who in ("rust", "swift"):
    sealed = json.load(open(f"{work}/sealed-by-{who}.json"))
    json.dump({"passphrase": fixture["passphrase"], "sealed": sealed}, open(f"{work}/open-{who}.json", "w"))
