# Gate native-agent support on demonstrated status evidence

Accepted during the architecture interview on 2026-11-06. Each supported native
CLI must demonstrate dependable attention and lifecycle signals in tested macOS
scenarios, with an explicit unknown state when evidence is unavailable. Process
liveness, agent activity, requests for input or approval, turn completion, and
interruption are distinct observations; silence alone establishes none of them.

This makes compatibility depend on tested tool versions and may delay support
rather than ship misleading status indicators. It does not require perfect
detection. The native-session proof must validate the signals available while
preserving native CLI interaction; events available only in another integration
mode do not establish compatibility for the PTY design.
