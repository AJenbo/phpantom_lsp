#!/usr/bin/env python3
"""Convert Criterion bencher-format output (stdin) to customSmallerIsBetter JSON (stdout).

Parses lines like:
    test cold_start_completion ... bench:   2610870 ns/iter (+/- 10235)

and emits a JSON array with nanosecond values converted to milliseconds:
    [{"name": "cold_start_completion", "unit": "ms", "value": 2.611, "range": "± 0.010"}, ...]

Exits non-zero when nothing parsed, so a benchmark run that died before
reporting fails here instead of silently handing an empty array to the
benchmark-tracking action.
"""

import json
import re
import sys

_BENCH_RE = re.compile(
    r"test\s+(?P<name>\S+)\s+\.\.\.\s+bench:\s+(?P<value>\d+)\s+ns/iter\s+\(\+/-\s+(?P<range>\d+)\)"
)

# Criterion itself never colours the bencher reporter, but the output can
# still pick up escape sequences from whatever is sharing the pipe.
_ANSI_RE = re.compile(r"\x1b\[[0-9;]*[A-Za-z]")

NS_PER_MS = 1_000_000

# How many raw lines to echo back when nothing parsed.
_PREVIEW_LINES = 20


def main() -> None:
    results = []
    seen = []
    for line in sys.stdin:
        line = _ANSI_RE.sub("", line).replace("\r", "").strip()
        if line:
            seen.append(line)
        m = _BENCH_RE.search(line)
        if not m:
            continue
        value_ms = round(int(m.group("value")) / NS_PER_MS, 3)
        range_ms = round(int(m.group("range")) / NS_PER_MS, 3)
        results.append(
            {
                "name": m.group("name"),
                "unit": "ms",
                "value": value_ms,
                "range": f"± {range_ms:.3f}",
            }
        )

    if not results:
        preview = "\n".join(f"  | {line}" for line in seen[:_PREVIEW_LINES])
        sys.exit(
            f"no benchmark results parsed from {len(seen)} non-empty input "
            "lines -- the benchmark run probably failed to build or crashed "
            f"before reporting. First lines read:\n{preview}"
        )

    json.dump(results, sys.stdout, indent=2)
    sys.stdout.write("\n")


if __name__ == "__main__":
    main()
