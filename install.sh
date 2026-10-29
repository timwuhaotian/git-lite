#!/bin/sh
set -eu

fail() {
    printf 'install: %s\n' "$*" >&2
    exit 1
}

need() {
    command -v "$1" >/dev/null 2>&1 || fail "required command not found: $1"
}

case "${1:-client}" in
    client)
        binary=gl
        package=gl-client
        ;;
    server)
        binary=gl-server
        package=gl-server
        ;;
    *)
        fail "usage: sh install.sh [server]"
        ;;
esac

case "$(uname -s)" in
    Darwin) os=apple-darwin ;;
    Linux) os=unknown-linux-gnu ;;
    *) fail "unsupported operating system: $(uname -s)" ;;
esac

case "$(uname -m)" in
    x86_64|amd64) arch=x86_64 ;;
    arm64|aarch64) arch=aarch64 ;;
    *) fail "unsupported architecture: $(uname -m)" ;;
esac

target="$arch-$os"
need curl
need mktemp

tmp_dir=$(mktemp -d "${TMPDIR:-/tmp}/git-lite-install.XXXXXX")
trap 'rm -rf "$tmp_dir"' EXIT
trap 'exit 1' HUP INT TERM

archive="$tmp_dir/$binary-$target.tar.gz"
url="https://github.com/timwuhaotian/git-lite/releases/latest/download/$binary-$target.tar.gz"

if curl -fsSL "$url" -o "$archive"; then
    need tar
    tar -xzf "$archive" -C "$tmp_dir"
    source="$tmp_dir/$binary"
    [ -f "$source" ] || fail "archive does not contain $binary"
else
    printf 'install: no prebuilt binary available; falling back to cargo install\n' >&2
    need cargo
    cargo install --git https://github.com/timwuhaotian/git-lite.git --locked --root "$tmp_dir/cargo" "$package"
    source="$tmp_dir/cargo/bin/$binary"
fi

if [ -n "${GL_INSTALL_DIR:-}" ]; then
    install_dir=$GL_INSTALL_DIR
elif [ -d /usr/local/bin ] && [ -w /usr/local/bin ]; then
    install_dir=/usr/local/bin
elif [ -d /opt/homebrew/bin ] && [ -w /opt/homebrew/bin ]; then
    install_dir=/opt/homebrew/bin
else
    [ -n "${HOME:-}" ] || fail 'HOME is not set; set GL_INSTALL_DIR instead'
    install_dir=$HOME/.local/bin
fi

mkdir -p "$install_dir"
destination="$install_dir/$binary"
install -m 0755 "$source" "$destination"
printf 'Installed %s to %s\n' "$binary" "$destination"

case ":${PATH:-}:" in
    *":$install_dir:"*) ;;
    *) printf 'Add it to PATH: export PATH="%s:$PATH"\n' "$install_dir" ;;
esac
