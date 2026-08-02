# Experiment harnesses

One survivor. The rest served their purpose and were deleted — their
*results* live in
`docs/architecture/2026-07-routing-engine-experiment-results.md`, which
is what anyone actually needs. Keeping the harnesses around after the
question is answered is keeping a scaffold after the building is up.

## `e1_crossisa`

The only one with a job left to do. E1 established that the whole solver
is bit-identical across x86_64 and aarch64 under glibc, which is what
makes a route computed on a phone match one computed on the server.

**Bionic on real silicon is still unverified.** Run this on a device
before relying on server/device agreement — it is the Phase F3 gate and
the last open determinism question.

## Deleted, and what they settled

| Harness | Question | Answer |
|---|---|---|
| `e0_determinism` | Does `exp()` agree across architectures? | Yes, bit-identical |
| `e2_dispatch` | Does `dyn` dispatch cost enough to need generics? | No — below measurement resolution. Killed the monomorphisation rule |
| `e7_tobler` | Do the six Tobler copies agree? | No: two different physical models, 41.9% apart on descent |
| `e10_packslice` | Does a sliced pack reproduce its source? | Not then — surfaced D8, now fixed. Superseded by `tileserver slice-pack --verify` |
| `e11_conformance` | Does the proposed port API hold up? | Yes. Superseded by the real ports and `turbo-route-ffi`'s host tests |
