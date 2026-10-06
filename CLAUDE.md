# otelo

Deploys, runs, and observes the apps on one Linux server, as one binary. The design and the
work live in openplan docs and tasks in the git ref `refs/openplan/tasks`. Read and write
them only with the `openplan` CLI. Start with `openplan doc get design`.

The Rust workspace is in `crates/`. Run everything through `mise`:

- `mise check`: fmt, clippy with every warning a failure, tests. Must pass before a commit.
- `mise tasks`: the full list.

The `otelo` skill lives in `skills/`, and `mise run skills` installs it into `.agents/skills` and
`.claude/skills` after a change. The `openplan` skills come from the openplan repository, and
`npx skills update -p` brings them up to date.
