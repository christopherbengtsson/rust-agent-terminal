# 12: Configure and enable automatic preparation

Status: ready-for-agent
Resolved: no
Blocked by: 11

**What to build:** Complete reviewer setup, understand usage costs, choose repository-first fallback behavior and control automatic review preparation through settings retained after restart.

**Acceptance coverage:** R1, R2 automation settings.

## Acceptance criteria

- [ ] Setup checks GitHub CLI authentication and reviewer availability with actionable guidance; the reviewer is selected independently of the coding-agent default.
- [ ] Explain background account/API usage and the enabled-by-default automation setting before setup completion, with an immediate opt-out and later editable setting. No automatic preparation starts before informed setup completes.
- [ ] When enabled, outstanding and newly discovered eligible personal requests enter the serial queue, including repositories without local registration.
- [ ] Disabling automation prevents queued automatic starts while preserving requests, drafts and explicit manual preparation; it does not implicitly cancel an active attempt. Re-enabling admits outstanding eligible work.
- [ ] Explain repository-first guidance and offer the bundled or optional custom fallback during setup and afterward. Validate configuration failures without silently changing precedence.
- [ ] Persist settings through the sole writer and apply only confirmed changes. Fallback changes affect the next explicit preparation, not existing drafts or agent usage by themselves.
- [ ] Verify setup/default/opt-out and enable/disable races with durable restart and failure cases, plus real existing CLI authentication and independent reviewer selection on macOS.

## Scope and handoff

This ticket completes reviewer-specific setup only. General onboarding, coding-agent configuration redesign and macOS notification permission flows remain outside M3.

## Comments
