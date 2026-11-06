# Source from a bash CI step, from any folder: starts the Jellyfin replay server
# (replay_jellyfin.py) on the recording and exports ENCORE_REPLAY_URL for the smoke
# tests. The server stops when the step's shell exits. Use one per step: each smoke
# program signs in from the start.

start_replay() {
  local here repo url_file py
  here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
  repo="$(cd "$here/../.." && pwd)"
  url_file="$(mktemp -d)/replay-url"
  py=python3
  python3 -c '' >/dev/null 2>&1 || py=python # Windows runners: python3 may be a stub
  "$py" -I -B "$here/replay_jellyfin.py" "$repo/tools/fixtures/jellyfin/recorded.json" "$url_file" \
    2> "$url_file.log" &
  REPLAY_PID=$!
  trap 'kill "$REPLAY_PID" 2>/dev/null || true' EXIT
  for _ in $(seq 300); do # up to 30 s: a busy runner can be slow to start Python
    test -s "$url_file" && break
    sleep 0.1
  done
  test -s "$url_file" || { echo "the Jellyfin replay server did not start"; cat "$url_file.log"; return 1; }
  ENCORE_REPLAY_URL="$(cat "$url_file")"
  export ENCORE_REPLAY_URL
  echo "Jellyfin replay at $ENCORE_REPLAY_URL"
}
