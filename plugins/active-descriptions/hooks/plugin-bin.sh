# Sourced by the hooks: prints the path to this plugin's binary $1, building
# and installing it first if this version has none yet (see build-bin.sh).
plugin_bin() {
  local root
  root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
  [ -x "$root/bin/$1" ] ||
    "$root/hooks/build-bin.sh" "$1" </dev/null >/dev/null 2>&1 ||
    return 1
  printf '%s\n' "$root/bin/$1"
}
