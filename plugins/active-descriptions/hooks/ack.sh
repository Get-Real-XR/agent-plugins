#!/bin/bash

# Record that each change in the revset given as $1 (default @) still matches
# its description although its diff changed since it was described, for
# example after `jj fix` or regenerating code. The stale check accepts it
# until its diff changes again. Check the description first: this is for
# changes the description already covers, not a way to skip describing.

exec cargo run -q -r --manifest-path "$(dirname "$0")/../Cargo.toml" -- ack "${1:-@}"
