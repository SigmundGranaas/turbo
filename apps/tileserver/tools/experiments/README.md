# Routing engine validation experiments

Standalone harnesses for the assumption audit
(`docs/architecture/2026-07-routing-engine-assumption-audit.md`).
Results are recorded in
`docs/architecture/2026-07-routing-engine-experiment-results.md`.

Each is its own tiny cargo workspace so it builds without the tileserver
workspace and without artifacts.

| Dir | Experiment | Needs data? |
|---|---|---|
| `e7_tobler` | Do the six duplicated Tobler implementations agree? | no |
| `e0_determinism` | Is the routing float path bit-reproducible across ISAs? | no |
| `e11_conformance` | Does the proposed port API compile and hold up, driven from memory? | no |
| `e2_dispatch` | What does the elevation port's dispatch actually cost? | **yes** — a real `norway.dem` |
| `e1_crossisa` | Is the whole solver bit-reproducible across ISAs? | **yes** — a full artifacts dir |

## Running

```sh
cd e7_tobler && cargo run --release

cd e0_determinism
cargo run --release                                  # x86_64 digest
cargo run --release --target aarch64-unknown-linux-gnu   # aarch64 digest
diff x86.txt arm.txt

cargo run --release --bin ulp     # size the divergence
cargo run --release --bin bench   # cost of the libm mitigation
```

aarch64 needs:

```sh
rustup target add aarch64-unknown-linux-gnu
apt-get install -y qemu-user-static gcc-aarch64-linux-gnu
```

`.cargo/config.toml` wires the cross-linker and the qemu runner.

Note: qemu + cross-glibc bounds the problem but does not settle Android,
which links bionic. See the results doc.

## e2_dispatch

Needs a built DEM artifact:

```sh
cd e2_dispatch
cargo run --release -- /path/to/norway.dem
```

Measures `sample()` and `slope_aspect()` through concrete, `dyn` and
monomorphised-generic call paths over 2 M corridor-ordered points, then
derives the solve-level penalty from the per-call delta and the known
lookup counts. This exists because the corpus harness cannot resolve a 2%
effect (phase 0 measured 16-19% run-to-run range); see the results doc.

## e1_crossisa

Solves the same routes on both architectures and diffs the geometry hashes.

```sh
cd e1_crossisa
cargo run --release -- /path/to/artifacts               > x86.txt
cargo run --release --target aarch64-unknown-linux-gnu \
      -- /path/to/artifacts                            > arm.txt
diff x86.txt arm.txt
```

Standalone rather than cross-compiling `tileserver`, whose sqlx/rustls/axum
C dependencies are unrelated to the question. Bounds the ISA question only —
Android links bionic, not glibc.
