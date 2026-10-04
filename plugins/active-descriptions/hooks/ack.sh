#!/bin/bash

# Record that each change in the revset given as $1 (default @) still matches
# its description although its diff changed since it was described, for
# example after `jj fix` or regenerating code. The stale check accepts it
# until its diff changes again. Check the description first: this is for
# changes the description already covers, not a way to skip describing.

source "$(dirname "$0")/plugin-bin.sh"
bin=$(plugin_bin jj-stale-descriptions) || {
  echo "active-descriptions: could not build jj-stale-descriptions" >&2
  exit 1
}
exec "$bin" ack "${1:-@}"
