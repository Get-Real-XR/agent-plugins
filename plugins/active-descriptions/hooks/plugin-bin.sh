# Sourced by the hooks: prints the path to this plugin's binary $1, building
# and installing it first if this version has none yet (see build-bin.sh).
#
# Codex runs the hooks from a checkout that updates in place, the
# agent-plugins marketplace clone, not from a per-version plugin cache. Every
# release bumps plugin.json, so a plugin.json newer than the binary means a
# new version to build.
plugin_bin() {
  local root
  root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
  if [ -x "$root/bin/$1" ] && [ "$root/.claude-plugin/plugin.json" -nt "$root/bin/$1" ]; then
    rm -f "$root/bin/$1"
  fi
  [ -x "$root/bin/$1" ] ||
    "$root/hooks/build-bin.sh" "$1" </dev/null >/dev/null 2>&1 ||
    return 1
  printf '%s\n' "$root/bin/$1"
}
