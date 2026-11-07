# 02: Prove restricted Claude review preparation

Status: ready-for-agent
Resolved: no
Blocked by: None

**What to build:** Establish that the supported native Claude integration can prepare structured findings from pinned repository context using existing authentication, while enforced restrictions prevent arbitrary commands, coding-checkout mutation and GitHub writes.

**Acceptance coverage:** Restricted-reviewer prerequisite for R3 and R10.

## Acceptance criteria

- [ ] Read the repository agent cost controls before any real-provider development or qualification launch. Use the required bounded cost profile and preserve exact tool/model/version and usage evidence.
- [ ] Demonstrate a real authenticated native inspection of pinned description, diff, relevant files and CI evidence that returns structured findings or a valid complete no-findings result.
- [ ] Actively exercise prohibited command execution, builds/tests, checkout writes and GitHub writes, including requests originating in repository guidance. Enforcement must prevent the effects; prompt instructions alone are insufficient.
- [ ] Confirm that coding setup, ordinary coding-launch permissions, inherited extensions/hooks or alternate tool paths cannot silently restore prohibited capabilities in the qualified execution configuration.
- [ ] Demonstrate interruption and process cleanup without stopping unrelated coding sessions, and distinguish valid complete output from truncated, malformed or interrupted output.
- [ ] Record the smallest reusable restricted execution approach and its resource/authentication assumptions, with executable checks and retained evidence. No product-wide coding-runner refactor is required.
- [ ] If the required contract cannot coexist with supported native authentication, document the evidence and return for a product decision. Do not resolve this prerequisite by weakening restrictions or claiming simulated checks prove native support.

## Scope and handoff

Supply the qualified Claude execution approach to ticket 10. Keep generated proof artifacts and instructions sufficient for a fresh implementation session to reproduce the result. This ticket authorizes no GitHub publication.

## Comments
