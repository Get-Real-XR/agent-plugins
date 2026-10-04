#!/bin/bash

# Builds this plugin's Rust binary $1 and installs it as bin/$1 in the
# plugin's folder, unless this installed version already has it.
#
# Hooks run bin/$1 directly. Through `cargo run`, every hook call paid for
# cargo's freshness check, and each new version rebuilt everything, jj-lib
# included, in a target folder of its own. Builds now share one target
# folder across versions and plugins, so a new version recompiles only what
# changed, while each installed version keeps its own binary: a session still
# on an older version never runs a newer binary, or the reverse.
#
# Each plugin links this file from its hooks/ folder; Claude Code copies
# plugins into its cache with links resolved, so the plugin folder is this
# script's parent's parent either way.

set -u
name=$1
root=$(cd "$(dirname "$0")/.." && pwd)
bin="$root/bin/$name"
[ -x "$bin" ] && exit 0

target="${XDG_CACHE_HOME:-$HOME/.cache}/agent-plugins/target"
mkdir -p "$target"

# One build and install at a time: two versions of a plugin build to the
# same path in the shared target folder, and each must install its own
# output. mkdir is atomic everywhere; macOS has no flock.
lock="$target/.install.lock"
until mkdir "$lock" 2>/dev/null; do
  # A build killed mid-way leaves its lock behind; clear it after 15 minutes.
  [ -n "$(find "$lock" -maxdepth 0 -mmin +15 2>/dev/null)" ] && rmdir "$lock" 2>/dev/null
  sleep 1
done
trap 'rmdir "$lock" 2>/dev/null' EXIT
[ -x "$bin" ] && exit 0

# Every version has the same crate name and version, so cargo gives them the
# same build identity in the shared folder and decides freshness by file
# times alone: a version whose files are older than another version's last
# build would be judged up to date and get that version's binary. Touching
# this version's sources makes cargo recompile the crate itself; its
# dependencies stay shared.
find "$root/src" -type f -exec touch {} +
touch "$root/Cargo.toml"
CARGO_TARGET_DIR="$target" cargo build --quiet --release --locked \
  --manifest-path "$root/Cargo.toml" --bin "$name" </dev/null || exit 1
mkdir -p "$root/bin"
tmp=$(mktemp "$root/bin/.$name.XXXXXX") || exit 1
cp "$target/release/$name" "$tmp" && chmod 755 "$tmp" && mv -f "$tmp" "$bin"
