# Server guide (`gl-server`)

One static Rust binary. Stores objects, commits and refs, serves the web UI.
No auth, no users, no merge logic. Run it behind a firewall, VPN or reverse
proxy.

## Install

```sh
curl -fsSL https://raw.githubusercontent.com/timwuhaotian/git-lite/main/install.sh | sh -s -- server
```

For a system directory:

```sh
curl -fsSL https://raw.githubusercontent.com/timwuhaotian/git-lite/main/install.sh | sudo sh -s -- server
```

The installer requires `curl` and prefers a prebuilt archive (`tar` required).
If none is available, it falls back to `cargo install --git`, which requires
Rust/Cargo; it does not install rustup. Set `GL_INSTALL_DIR` to override the
install directory.

### Source build

```sh
cargo build --release -p gl-server
./target/release/gl-server
```

## Run

```sh
PORT=7331 GL_DATA=./data gl-server
```

- Listens on `0.0.0.0:$PORT`, default `7331`.
- Data root `$GL_DATA`, default `./data`. Repos live in
  `$GL_DATA/repos/<name>/` (`refs.json`, `commits/<id>.json`,
  `objects/xx/<hash>`).
- Prints `gl-server: data=... port=...` to stderr on boot.
- Env only, no flags, no config file.

Health check:

```sh
curl localhost:7331/api/health   # {"status":"ok"}
```

Web UI: open `http://host:7331/`. The page is embedded in `gl-server`, so
release installs need no web files or source checkout.

## Deploy (systemd)

```sh
useradd -r -m git-lite                  # unit runs as this user
sudo ./deploy/install.sh                # build, install bins, enable service
systemctl status gl-server
```

`deploy/install.sh` installs `gl-server` and `gl` to `/usr/local/bin`, creates
`/var/lib/git-lite`, installs `deploy/gl-server.service`. The unit sets
`GL_DATA=/var/lib/git-lite`, `PORT=7331`, restarts on failure. Override with
`systemctl edit gl-server`.

Reverse proxy example (Caddy):

```caddy
git.example.com {
    reverse_proxy 127.0.0.1:7331
}
```

TLS and auth belong at the proxy, not in `gl-server`.

## API

| Method | Path | Notes |
| ------ | ---- | ----- |
| GET | `/api/health` | `{"status":"ok"}` |
| GET | `/api/repos` | repo name list |
| GET | `/api/repos/{name}/refs` | branch to commit map |
| GET | `/api/repos/{name}/log?branch=&n=` | newest first, `n` clamped 1..500 |
| GET | `/api/repos/{name}/commits/{id}` | one commit JSON |
| GET | `/api/repos/{name}/objects/{hash}` | raw bytes |
| POST | `/api/repos/{name}/objects/{hash}` | raw bytes body, idempotent |
| POST | `/api/repos/{name}/push` | `{refs, commits}`; `409` on non-fast-forward |
| GET | `/` | web UI |

Repo names: letters, digits, `-_.`, max 128 chars. CORS is permissive (the
bundled UI fetches it cross-origin when opened as a file).

## Ops

- **Backup:** stop the unit or snapshot live; every file is immutable once
  written (`rsync -a /var/lib/git-lite/` is enough).
- **Disk use:** `du -sh /var/lib/git-lite`; objects deduplicate by content
  across branches and repos.
- **Logs:** stdout/stderr via `journalctl -u gl-server -f`.
- **Push rejected (409):** client is behind. `gl pull` on the client, re-save
  if needed, push again. The server never merges.
