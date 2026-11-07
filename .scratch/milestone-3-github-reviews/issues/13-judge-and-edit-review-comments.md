# 13: Judge and edit draft comments

Status: ready-for-agent
Resolved: no
Blocked by: 08

**What to build:** Judge suggestions and original comments in context, edit review text inline, and retain exact choices and text without publishing anything.

**Acceptance coverage:** R4; R5 selection foundation.

## Acceptance criteria

- [ ] Generated suggestions, when available, begin undecided and support explicit Keep, Keep all and Discard. The draft also supports explicit publication selection for original user comments.
- [ ] Edit existing comments inline and add original location-bearing comments and overall text. Editing text alone does not select a suggestion, validate a stale anchor or choose the review outcome.
- [ ] Persist exact text, selection and comment identity/version through attributed acknowledgements; reopen after navigation/restart with identical confirmed content and choices.
- [ ] Distinguish a pending or failed save from confirmed saved state, retain recoverable input, and reject stale completions that would overwrite another draft/comment version.
- [ ] Selected-content inspection excludes undecided and discarded suggestions. Expose only local inspection at this stage; no GitHub publication control is enabled before ticket 17.
- [ ] Verify Keep/Discard/edit/add sequences and all selection states at the supervisor seam with durable storage, then exercise keyboard navigation and small-terminal presentation beside the matching diff.

## Scope and handoff

Do not block this local judgment slice on native generation: manual drafts and controlled suggestion fixtures provide complete behavior independently. External-editor round-trip follows in ticket 14.

## Comments
