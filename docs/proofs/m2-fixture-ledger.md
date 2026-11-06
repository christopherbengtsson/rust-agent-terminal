# Scripted flood receipt framing

The integrated Milestone 2 validation exposed an intermittent failure in
`storage_saturation_keeps_navigation_retry_stop_and_exact_retained_bytes`:
parsing `proof-emitted.jsonl` after Stop found JSON EOF. An isolated rerun passed.
The fixture formatted a `serde_json::Value` directly into a file with `writeln!`.
Value formatting performs multiple writes, so stopping the fixture could leave a
partially formatted receipt even though its preceding 128-byte stdout write had
completed.

`flood13` now formats the complete receipt, including its newline, into a bounded
string before calling `File::write_all`. Its fixed fields contain only bounded
integer values. This removes the formatter's many individual file writes from
the Stop window. It does not provide a new crash-atomic filesystem or durability
guarantee, and it does not change product capture, Stop or persistence semantics.

The existing dashboard/supervisor/scripted-child saturation scenario remains the
regression seam. Its exact retained-prefix comparison, strict parsing of every
ledger row and one-in-flight-128-byte-write accounting bound are unchanged.
Malformed records are not filtered out. No deterministic scheduling hook was
added solely to interrupt the old formatter between its internal writes; the
original failure remains intermittent evidence rather than a deterministic red
regression.

Focused formatting, locked build and warnings-denied all-target Clippy passed.
PTY execution was deferred to the integration owner to avoid concurrent workloads
while the serialized integrated suite was running. That suite must establish the
runtime result after this fixture correction; this note does not claim a runtime
pass, native CLI qualification or whole-milestone completion.
