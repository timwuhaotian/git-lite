# Client guide (`gl`)

`gl` is the only thing you type day to day. It owns snapshots, branches,
worktrees and sync. Anything not in `gl --help` does not exist.

## Install

```sh
cargo build --release -p gl-client
export PATH="$PWD/target/release:$PATH"   # binary is named `gl`
gl --version
```

## Connect to a server

Every repo remembers its server in `.gl/config.json`. Set it at init or
later (init defaults to `http://127.0.0.1:3000`, so always pass `--server`
for real use):

```sh
gl init --server http://your-host:7331 [--repo NAME] [--branch main] [--author you]
gl config --list                       # server, repo, branch, author
gl config server http://new-host:7331  # repoint
gl config repo NAME                    # share one remote repo across checkouts
```

Repo name defaults to the directory name.

## Daily use

```sh
gl save -m "done for today"   # snapshot everything, no staging
gl push                        # upload objects, commits, move remote ref
gl log -n 10                   # local history
gl status                      # tip plus Added / Modified / Deleted
```

`save` hashes every file (except `.gl`), stores new blobs, writes one commit
on the current branch. Author defaults to `$USER`, override per save with
`gl save -m MSG --author NAME`.

Get code on a new machine:

```sh
mkdir demo && cd demo
gl init --server http://your-host:7331 --repo demo
gl pull
```

## Branches

```sh
gl branch              # list
gl branch feat-login   # create at current tip
gl switch feat-login   # workdir files swap to that branch
gl log --branch main   # history of another branch
```

## Worktrees

Second checkout in another directory, same server repo:

```sh
gl worktree add ../demo-login feat-login
cd ../demo-login
gl save -m "wip" && gl push
```

The new checkout copies local commits and objects once, then diverges. Keep
`main` running while a feature brews next door.

## Command reference

| Command | Notes |
| ------- | ----- |
| `gl init [--server URL] [--repo N] [--branch B] [--author A]` | create `.gl`, remember server |
| `gl save -m MSG [--author A]` | snapshot workdir as one commit |
| `gl log [-n N] [--branch B]` | local history, newest first (default 20) |
| `gl status` | tip plus `A`/`M`/`D` vs workdir |
| `gl branch [NAME]` | list, or create at current tip |
| `gl switch NAME` | checkout branch, swaps workdir files |
| `gl worktree add PATH BRANCH` | extra checkout elsewhere |
| `gl push` | upload missing objects and commits, move remote ref |
| `gl pull` | fast-forward only: fetch missing, move ref, checkout |
| `gl config --list` | show all four keys |
| `gl config KEY VALUE` | set `server`, `repo`, `branch` or `author` |

## Troubleshooting

| Symptom | Meaning | Fix |
| ------- | ------- | --- |
| `nothing to push` | current branch has no commit | `gl save` first |
| `no remote branch X` | server never saw branch X | push from the machine that owns it |
| `POST push failed: 409` | remote moved, you are behind | `gl pull`, re-save if needed, push again |
| `GET refs failed` / connection refused | wrong URL or server down | `gl config --list`, `curl host:7331/api/health` |
| `object ... checksum mismatch` | corrupt download or store | re-pull; check server disk |
| `not inside a gl repo` | no `.gl` above cwd | `cd` into the checkout or `gl init` |

`pull` never merges. If local and remote diverged, it errors and your files
stay untouched; reconcile by saving local work on a branch and pushing that.
