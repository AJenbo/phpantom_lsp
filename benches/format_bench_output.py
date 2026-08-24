#!/usr/bin/env python3
"""Convert Criterion bencher-format output (stdin) to customSmallerIsBetter JSON (stdout).

Parses lines like:
    test cold_start_completion ... bench:   2610870 ns/iter (+/- 10235)

and emits a JSON array with nanosecond values converted to milliseconds:
    [{"name": "cold_start_completion", "unit": "ms", "value": 2.611, "range": "± 0.010"}, ...]
"""

import json
import re
import sys

_BENCH_RE = re.compile(
    r"^test\s+(?P<name>\S+)\s+\.\.\.\s+bench:\s+(?P<value>\d+)\s+ns/iter\s+\(\+/-\s+(?P<range>\d+)\)$"
)

NS_PER_MS = 1_000_000


def main() -> None:
    results = []
    for line in sys.stdin:
        m = _BENCH_RE.match(line.strip())
        if not m:
            continue
        value_ms = round(int(m.group("value")) / NS_PER_MS, 3)
        range_ms = round(int(m.group("range")) / NS_PER_MS, 3)
        results.append(
            {
                "name": m.group("name"),
                "unit": "ms",
                "value": value_ms,
                "range": f"\u00b1 {range_ms:.3f}",
            }
        )
    if not results:
        sys.exit(
            "no benchmark results parsed from stdin -- the benchmark run "
            "probably failed to build or crashed before reporting"
        )

    json.dump(results, sys.stdout, indent=2)
    sys.stdout.write("\n")


if __name__ == "__main__":
    main()