# Measurement runs

One directory per benchmark run, written by `cargo xtask bench` and committed
so the published tables can be reproduced and compared:

```text
runs/<yyyymmdd>-<hhmmss>-<host>/results.json
```

Ids sort chronologically; `docs/src/results.md` is generated from the newest
one by `cargo xtask report`, and `cargo xtask compare <a> <b>` prints the
change between any two. The file records the host, toolchain, commit and
exact command alongside Criterion's statistics for every benchmark, so a run
from a different machine is still useful as long as it is read as such. The
layout is described on the
[benchmarking page](../docs/src/benchmarking.md#the-run-file).

Do not edit a run after the fact; record a new one.
