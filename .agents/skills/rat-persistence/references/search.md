# Search scheduling

Budget work as well as result size on the sole storage worker. Interleave long
scans/indexing with capture and control; query identity must keep cancelled or
stale results from replacing a newer selection.

Choose indexes from measured access patterns. Index only committed history and
account for deletion/restart so an index cannot resurrect removed content.
Measure capture flush delay alongside search latency under active output using
the [measurement methodology](../../../../docs/proofs/17-session-measurements.md).
