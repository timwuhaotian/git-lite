# git-lite

[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.80%2B-orange.svg)](https://www.rust-lang.org/)
[![Platform](https://img.shields.io/badge/server-linux-blue.svg)](deploy/)
[![UI](https://img.shields.io/badge/UI-single%20file-lightgrey.svg)](web/index.html)

Personal and small-team ultra-fast minimal git replacement. Save-anytime
snapshots plus branch isolation, without git's cost or GitLab's weight.

```sh
gl save -m "done for today"   # snapshot everything, no staging
gl push                        # home server has it in a second
```

## Why

- **Save first, think later.** No index, no staging. `gl save` snapshots the
  whole workdir, content-addressed with blake3.
- **Branches are cheap isolation.** One command per feature line, plus
  `worktree` to keep several checked out side by side.
- **Dumb fast server.** The server stores objects, commits and refs. All
  snapshot logic lives in the CLI. No accounts, no permissions, no CI.
- **One static page.** The web UI is a single dependency-free HTML file
  served by the server binary. Nothing to build, nothing to deploy.

## Quickstart

Server (Linux, one binary):

```sh
cargo build --release -p gl-server
PORT=7331 GL_DATA=./data ./target/release/gl-server
curl localhost:7331/api/health   # {"status":"ok"}
```

Client, daily flow:

```sh
cargo build --release -p gl-client
export PATH="$PWD/target/release:$PATH"

mkdir demo && cd demo
gl init --server http://your-host:7331
echo hello > a.txt
gl save -m "start"    # snapshot
gl push               # upload
```

New machine, same code:

```sh
mkdir demo && cd demo
gl init --server http://your-host:7331 --repo demo
gl pull               # files appear
```

Web UI: open `http://your-host:7331/` in any browser. Repo list, branch
pills, commit history, auto-refresh every 5s, light and dark theme.

## Commands

| Command | What it does |
| ------- | ------------ |
| `gl init [--server URL] [--repo NAME] [--branch B] [--author A]` | create `.gl` here, remember server |
| `gl save -m MSG [--author A]` | snapshot workdir as one commit |
| `gl log [-n N] [--branch B]` | local history, newest first |
| `gl status` | tip plus Added/Modified/Deleted vs workdir |
| `gl branch [NAME]` | list branches, or create one at current tip |
| `gl switch NAME` | checkout branch (workdir files swap) |
| `gl worktree add PATH BRANCH` | second checkout in another directory |
| `gl push` | upload missing objects and commits, move remote ref |
| `gl pull` | fast-forward to remote ref, fetch what is missing |
| `gl config --list` | show server, repo, branch, author |
| `gl config server URL` / `gl config repo NAME` | point elsewhere |

Config lives in `.gl/config.json`. The repo name defaults to the directory
name; override it when two checkouts share one remote repo.

## Worktree workflow

```sh
gl branch feat-login          # isolate
gl worktree add ../demo-login feat-login   # second checkout, same objects
cd ../demo-login
gl save -m "wip" && gl push   # independent line, shared server
```

Worktrees share the object store by copying it once, then diverge. Good for
keeping `main` running while a feature brews next door.

## Server and deploy

```sh
sudo ./deploy/install.sh   # builds release, installs bins + systemd unit
```

- Listens on `0.0.0.0:$PORT` (default `7331`), data in `$GL_DATA`
  (default `./data`), layout `$GL_DATA/repos/<name>/`.
- systemd unit: `deploy/gl-server.service` (user `git-lite`, restarts on
  failure). Create the user first: `useradd -r -m git-lite`.
- No auth. Run behind a firewall, VPN or reverse proxy.

## How it works

Snapshots are content-addressed (blake3 of file bytes). A commit is one JSON
file pointing at a tree of `{path: {hash, size}}`. Push sends missing objects
first, then commits, then moves the ref. The server rejects non-fast-forward
ref moves with `409`.

```
                ┌──────────┐
                │ web UI   │  web/index.html (no build)
                │ :7331 /  │  GET /api/repos, /log
                └────┬─────┘
                     │ HTTP JSON
┌──────────┐  push/pull  ┌──────────────────┐
│ gl-client│◄───────────►│ gl-server (:7331) │
│ (`gl`)   │  refs+push  │ dumb object store │
└────┬─────┘  objects/:h └────────┬─────────┘
     │                            │
     ▼                            ▼
 .gl/{config.json,HEAD,     $GL_DATA/repos/<name>/
   refs/heads/*,              refs.json
   objects/xx/<hash>,         objects/xx/<hash>
   commits/<id>.json}         commits/<id>.json}
```

## API

| Method | Path | Description |
| ------ | ---- | ----------- |
| GET | `/api/health` | liveness probe |
| GET | `/api/repos` | list repo names |
| GET | `/api/repos/{name}/refs` | branch to commit map |
| GET | `/api/repos/{name}/log?branch=&n=` | commit list, newest first (max 500) |
| GET | `/api/repos/{name}/commits/{id}` | one commit JSON |
| GET | `/api/repos/{name}/objects/{hash}` | download object bytes |
| POST | `/api/repos/{name}/objects/{hash}` | upload object bytes (raw body) |
| POST | `/api/repos/{name}/push` | body `{refs, commits}`, rejects non-ff with 409 |
| GET | `/` | the web UI |

## Non-goals

- No git compatibility (no packfiles, no git protocol, no `git clone`)
- No auth, users or permissions
- No server-side merge engine; `pull` is fast-forward only
- No `merge` / `diff` commands yet (next on the list)
- No hooks, LFS, submodules, partial clone
- Linux server only

## Contributing

Read [CONTRIBUTING.md](CONTRIBUTING.md). Scope is strict on purpose.

## License

MIT. See [LICENSE](LICENSE).
