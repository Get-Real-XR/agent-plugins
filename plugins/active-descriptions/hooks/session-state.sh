# Sourced by the hooks: per-session state, kept outside any repo.

# Prints the state directory for session $1, creating it, and drops state
# from sessions untouched for 30 days.
session_state() {
  local base="${XDG_STATE_HOME:-$HOME/.local/state}/active-descriptions/sessions"
  mkdir -p "$base/$1"
  find "$base" -mindepth 1 -maxdepth 1 -type d -mtime +30 -exec rm -rf {} + 2>/dev/null
  printf '%s\n' "$base/$1"
}
