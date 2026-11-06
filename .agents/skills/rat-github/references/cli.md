# GitHub CLI transport

Use explicit repository/host targeting, structured JSON and bounded pagination.
Consult the [gh API manual](https://cli.github.com/manual/gh_api) for method/input
behavior and the [review API](https://docs.github.com/en/rest/pulls/reviews) for
inline payloads. Fields can change `gh api`'s default method; specify read methods
where needed. Pass multiline payloads as structured arguments/files.

Document the remaining revision race: these APIs alone do not establish atomic
compare-and-submit.
