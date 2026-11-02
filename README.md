# Prime Sieve

A Rust program for finding and counting primes with several implementations of
the Sieve of Eratosthenes. It includes a simple baseline, a bit-packed
odd-number sieve, and a cache-aware segmented sieve that can use multiple
threads. At very large numbers (over 10^10) it is around ~3x slower 
than Kim Walisch's famous primesieve.

## Requirements and setup

- Rust and Cargo (the project uses Rust edition 2024).
- A `.env` file containing `CPU_L1D_CACHE_SIZE_PER_INSTANCE_KB` and
  `CPU_LOGICAL_CORES`. The collection scripts below can generate this file.
  If a value is missing or malformed, the program prints a warning and falls
  back to a default (32 KiB L1 data cache, 12 logical cores).

The program loads `.env` from the current working directory. Both values may be
comma-separated when the machine reports different CPU types; the program uses
the smallest value in each list. The L1 data-cache size is converted from KiB
to bytes and used as the basis for segment sizing. The logical core count sets
the worker-thread limit. Other values written by the collection scripts are
informational and are not currently used by the program.

For example, collect the host details and run the default benchmark:

```sh
bash collect_linux.sh
cargo run --release
```

On Windows, run the collector from PowerShell instead:

```powershell
.\collect_windows.ps1
cargo run --release
```

To build without running:

```sh
cargo build --release
```

The executable is `target/release/prime_sieve` on Linux and
`target\release\prime_sieve.exe` on Windows.

## Algorithms

### Naive Eratosthenes sieve

`eratosthenes_naive` keeps one boolean per integer, initially marks all
integers as potentially prime, then clears multiples of successive primes.
This implementation is used as a correctness and timing baseline in `compare`.

### Optimized in-memory sieve

`eratosthenes_algo_optimized` stores only odd candidates at one bit per number:
bit 0 represents 3, bit 1 represents 5, and so on. It handles 2 separately,
marks odd multiples starting at each prime's square, and extracts primes by
finding set bits. This reduces both memory use and work spent on even numbers.
It is used for smaller limits and as the base-prime generator for larger runs.

### Cache-aware segmented sieve

For larger limits, the program first generates the primes up to the square root
of the requested limit. It then partitions the remaining odd candidates into
segments. Each segment is sized from half the reported L1 data cache by default
(`cache_usage_factor = 0.5`), rounded to a whole number of 64-bit words. Each
worker reuses its own bit buffer while marking segment composites with the base
primes. An atomic counter distributes segments to workers, and results are
sorted back into segment order before being returned.

An initial repeating bit pattern pre-clears multiples of 3, 5, 7, 11, and 13.
The remaining base primes mark their odd multiples in each segment. The default
worker count is the reported logical core count, capped by the number of
segments available for the run.

### Count-only segmented sieve

`count` uses the same segmented approach but does not collect prime values.
Instead, it counts set bits in each completed segment with the machine's
population-count operation. This avoids allocating and merging a vector of
all the primes.

Workers claim chunks of consecutive segments (`--chunk-segments`, 1024 by
default) and carry per-prime state between them, so each prime's next multiple
is found without a division. Base primes at least as large as a segment hit it
at most once, so they are kept in buckets keyed by the segment they hit next and
are only visited when they actually have a multiple there.

### Saving all primes

`findall` runs the segmented sieve and writes discovered primes to text files
instead of retaining the full result in memory. Files contain comma-separated
values and are split when a worker's pending-prime buffer exceeds its configured
threshold (1,000,000 by default). Each file name records its first and last
prime, and each invocation gets its own run directory.

For limits below 10,000, the program uses the optimized in-memory sieve rather
than the segmented implementation.

## Commands

The command-line syntax is:

```text
prime_sieve [compare | scaling | count <n> | findall <n>] [--cargo]
            [--cache-usage-factor <f64>] [--max-prime-vector-size <n>]
            [--chunk-segments <n>]
```

`scaling` is the default when no mode is supplied. The `--cargo` option can be
placed alongside a mode and changes where report files are saved.

### `scaling`

```sh
cargo run --release -- scaling
```

Runs the count-only sieve for limits from 100 through 100,000,000,000,
increasing by powers of ten. It writes one line per limit to a results report.
This mode can take a long time at the largest limits.

### `compare`

```sh
cargo run --release -- compare
```

Compares the naive, optimized, and segmented in-memory sieves at fixed limits
from 100 through 100,000,000. It checks whether each implementation returns
the same prime list as the naive baseline and records timings while varying
thread counts, cache-usage factors, and prime-buffer thresholds. This is a
benchmark sweep and may run much longer than a single sieve.

### `count <n>`

```sh
cargo run --release -- count 1000000
```

Counts primes less than or equal to `n` without storing the full prime list.
The report includes the limit, count, cache size in bytes, cache-usage factor,
thread count, and elapsed time. Provide an integer limit.

### `findall <n>`

```sh
cargo run --release -- findall 1000000
```

Finds primes less than or equal to `n`, writes them to files, and prints both
the report path and the directory containing the prime files. Use this mode
when you need the prime values rather than only their count.

### Tuning options

| Option | Default | Meaning |
|---|---|---|
| `--cache-usage-factor` | 0.5 | Fraction of the L1 data cache used per segment |
| `--max-prime-vector-size` | 1000000 | Primes a worker holds before dumping to a file |
| `--chunk-segments` | 1024 | Consecutive segments a count worker claims at once |

Both `--option value` and `--option=value` are accepted.

### `--cargo`

```sh
cargo run --release -- count 1000000 --cargo
```

By default, reports are saved in a `results` directory beside the running
executable. With `--cargo`, reports go to the repository's `results` directory
instead. For `findall`, the prime-output directories are created under the
selected results directory as well. Report and prime-directory names include
a timestamp so separate runs do not overwrite one another.

## Performance

On an Intel Core i7-1255U (12 logical cores), `count 100000000000` (10^11,
4,118,054,813 primes) takes about 10 seconds. The release profile uses fat LTO,
a single codegen unit, and `target-cpu=native` (see `.cargo/config.toml`).
Segments that exceed the L1 data cache slow the sieve down considerably, so keep
`--cache-usage-factor` at or below about 1.

## Host-information scripts

### Linux: `collect_linux.sh`

```sh
bash collect_linux.sh [output.env]
```

Reads CPU model, physical and logical core counts, cache sizes and line sizes
from Linux system files, and total memory from `/proc/meminfo`. It writes
available values as quoted environment assignments. The default output is
`.env` beside the script; an optional path selects another output file.

### Windows: `collect_windows.ps1`

```powershell
.\collect_windows.ps1 [output.env]
```

Collects processor and memory information through Windows CIM and reads cache
topology through the Windows processor-topology API. The default output is
`.env` beside the script; an optional path selects another output file.

Both scripts may record multiple cache sizes as comma-separated values on
hybrid CPUs. The program uses the minimum value for the two settings it
requires, which selects a conservative cache-based segment size and worker
count.

## Results

Result reports are named `result_test_<mode>_<timestamp>.txt`. `scaling`
reports use the `scaling_count` label; `count` and `findall` include their
limit in the label; `compare` uses `test_compare`. The `results/` directory
is ignored by Git, so benchmark output is not committed by default.
