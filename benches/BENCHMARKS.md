# FinLang benchmarks

Throughput measurements for the FinLang JIT compared against a hand-written
safe-Rust baseline that calls the same `finlang-stdlib` primitives directly.

## Running

```sh
# Full Criterion run (warm-up 3 s, measurement 5 s, 100 samples per bench).
cargo bench --bench portfolio_bench

# Fast smoke run (used by CI / quick local iteration).
cargo bench --bench portfolio_bench -- \
    --warm-up-time 1 --measurement-time 2 --sample-size 20
```

HTML reports land in `target/criterion/report/index.html`.

## Bench groups

| Group              | What it measures                                                             |
|--------------------|------------------------------------------------------------------------------|
| `jit_run`          | A single call to `JitProgram::run()` after one up-front compile.             |
| `full_compile`     | Source → tokens → AST → typed AST → SSA → optimisations → Cranelift JIT.     |
| `native_baseline`  | The same financial calculations expressed as direct `finlang-stdlib` calls.  |

## Reference numbers

Captured on Windows 11 x86-64 (Cranelift 0.111, fast-run profile,
`--measurement-time 2 --sample-size 20`).  Treat them as order-of-magnitude
reference points; absolute numbers will move with CPU / clock / build flags.

| Bench                                  | Median       | Throughput      |
|----------------------------------------|--------------|-----------------|
| `jit_run/option_pricing`               | ≈ 161 ns     | ≈ 6.2 M ops/s   |
| `jit_run/bond_portfolio`               | ≈ 1.81 µs    | ≈ 553 K ops/s   |
| `jit_run/var_calculation`              | ≈ 31 ns      | ≈ 32 M ops/s    |
| `full_compile/option_pricing`          | ≈ 247 µs     | one-shot        |
| `full_compile/bond_portfolio`          | ≈ 279 µs     | one-shot        |
| `native_baseline/option_pricing`       | ≈ 71 ns      | ≈ 14 M ops/s    |
| `native_baseline/bond_portfolio`       | ≈ 213 ns     | ≈ 4.7 M ops/s   |

### Interpretation

* `jit_run` and `native_baseline` are not always the same work.  The
  example programs deliberately compute several Greeks alongside the
  call price, so `jit_run/option_pricing` does more arithmetic than the
  single `black_scholes` + `bs_delta` calls in the baseline.  Lining
  the two up against each other is a fair comparison of compiler
  overhead given identical inlining decisions; treat the absolute ratios
  as upper bounds on per-call overhead, not as a quality of optimisation.
* For the typical "Python with NumPy" reference point of ≈ 5–10 µs per
  Black-Scholes call, the FinLang JIT at ≈ 160 ns is roughly **30×–60×**
  faster end-to-end — the target the project was originally specified
  against.
* `full_compile` is the slow path, ≈ 250 µs to lex, parse, typecheck,
  lower, optimise, and JIT a small program from source.  Once compiled,
  re-running the program is in the nanosecond range, so the amortised
  cost per evaluation drops sharply over a long-running session.

## Sanity check

The benchmark binary asserts at startup that the JIT result for
`option_pricing.fin` matches the safe-Rust reference within `1e-9`.
A failed sanity check aborts the run *before* Criterion publishes any
numbers, so a green Criterion report is also a numerical-precision
report.
