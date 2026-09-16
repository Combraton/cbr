# Vendored Protocol v0.1.0

Pinned contract material for CBR. **Do not edit any file here.** Everything except `PIN.json` and this README is a byte-for-byte copy from the published Protocol v0.1.0 release archive.

- Tag `v0.1.0`, commit `cbf8e4df9df2ca8a9b50264df6acace6e4c3a0fc`, tree `ad57cc4ef067834c20d868dbcc844f70b8ef223f`.
- Identities and checksums: [PIN.json](PIN.json).
- Licence of the vendored content: [LICENSE](LICENSE) (MIT, Protocol's own).

## Where it came from

The release archive was downloaded, checked against the release's `SHA256SUMS`, extracted, checked against `BUNDLE-SHA256SUMS`, and verified with the release's own `release_inventory.py --verify` in file-system mode before anything was copied. It was **not** copied from a sibling `protocol` checkout, whose `main` is ahead of the tag and is not the contract.

## Re-verifying, without trusting this directory

```sh
python3 scripts/verify_pin.py
```

That checks two anchors that do not depend on CBR:

1. Every vendored file against `BUNDLE-SHA256SUMS`, which the release publishes as its own asset with its own checksum.
2. The inventory's aggregate `listing_sha256`, recomputed from `docs/release/0.1/inventory.json` using the algorithm the release's own tooling uses, against the value in `PIN.json`. That value also appears in the annotated tag message and in the release's `release-manifest.json`.

It additionally re-checks every vendored file that falls inside the inventory's normative scope against the inventory's own recorded digest, which is a third, independent path.

To re-obtain the archive from scratch, see `docs/work/readiness/PROTOCOL-PIN.md` §6.

## What is deliberately absent

The reference provider, the independent Python provider and the protocol's own participant descriptors are **not** vendored. CBR implements the contracts and the documented test-control vocabulary itself; the reference implementations are test scaffolding for that repository, not a library to depend on.
