# Benchmarks

PHPantom is benchmarked on every commit to track performance
regressions. All numbers below were measured on a production codebase:
21K PHP files, 1.5M lines of code (vendor + application).

## Headline Numbers

| Metric | PHPantom | Intelephense | PHP Tools | Phpactor | PHPStorm |
| --- | --- | --- | --- | --- | --- |
| Time to ready | 5 s | 1 min 25 s | 3 min 17 s | 15 min 39 s | 17 min 55 s |
| RAM usage | 360 MB | 520 MB | 3.9 GB | 498 MB | 1.7 GB |
| Disk cache | 0 | 45 MB | 0 | 4.1 GB | 551 MB |

Time to ready is CPU time consumed until full type intelligence is
available on a cold start (first index). Tools with a disk cache launch
faster on subsequent starts.

## Live Charts

Latency and memory usage are tracked on every commit and plotted over
time. These charts are useful for catching regressions and observing
trends across releases.

- [Latency Benchmarks](https://phpantom-dev.github.io/phpantom_lsp/dev/bench/) -- completion response time per commit
- [Memory Benchmarks](https://phpantom-dev.github.io/phpantom_lsp/dev/memory/) -- resident memory after full indexing

## What We Measure

**Latency benchmarks** run `cargo bench` on the completion engine,
measuring end-to-end response time for completion requests against
real-world fixtures. Results are tracked with
[github-action-benchmark](https://github.com/benchmark-action/github-action-benchmark).

**Memory benchmarks** measure peak resident memory (RSS) of
`phpantom_lsp` after fully indexing the benchmark project. THP (Transparent
Huge Pages) is disabled during measurement for consistent results.
