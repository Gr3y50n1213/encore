#!/usr/bin/env bash
# Discovery only (removed before merge): what an unclaimed, offline Plex Media Server answers.
set -u
base="$PLEX_URL"; j='Accept: application/json'
py() { python3 -c "$1" "${2:-}"; }
H=(-H 'X-Plex-Client-Identifier: ci-probe' -H 'X-Plex-Product: Encore' -H 'X-Plex-Platform: Generic' -H 'X-Plex-Version: 0.1.0')

# One library per agent and scanner, each over the same folder: which gives covers and lyrics offline?
n=0
for combo in "com.plexapp.agents.none|Plex%20Music%20Scanner" "tv.plex.agents.none|Plex%20Music" "com.plexapp.agents.none|Plex%20Music"; do
  n=$((n+1)); agent="${combo%%|*}"; scanner="${combo##*|}"
  echo "=== library $n: $agent / $scanner"
  curl -sS -o /dev/null -w 'create %{http_code}\n' -X POST -H "$j" \
    "$base/library/sections?name=M$n&type=artist&agent=$agent&scanner=$scanner&language=en-US&location=%2Fmusic&prefs%5BrespectTags%5D=1"
  id="$(curl -sS -H "$j" "$base/library/sections" | py 'import json,sys; d=json.load(sys.stdin)["MediaContainer"].get("Directory",[]); print(d[-1]["key"] if d else "")')"
  echo "section $id"
  [ -n "$id" ] || continue
  start=$(date +%s)
  for _ in $(seq 1 120); do
    c="$(curl -sS -H "$j" "$base/library/sections/$id/all?type=10" | py 'import json,sys; print(json.load(sys.stdin)["MediaContainer"].get("size",0))')"
    r="$(curl -sS -H "$j" "$base/library/sections" | py "import json,sys; print([d.get('refreshing') for d in json.load(sys.stdin)['MediaContainer']['Directory'] if d['key']=='$id'])")"
    [ "$c" = 6 ] && [ "$r" = "[False]" -o "$r" = "[None]" ] && break; sleep 2
  done
  echo "tracks $c after $(( $(date +%s) - start ))s, refreshing $r"
  curl -sS -H "$j" "$base/library/sections" | py "import json,sys; print([d for d in json.load(sys.stdin)['MediaContainer']['Directory'] if d['key']=='$id'])"
  albums="$(curl -sS -H "$j" "$base/library/sections/$id/all?type=9")"
  py 'import json,sys
for m in json.loads(sys.argv[1])["MediaContainer"].get("Metadata",[]): print("album", m["ratingKey"], m["title"], m.get("thumb"), m.get("updatedAt"))' "$albums"
  t="$(curl -sS -H "$j" "$base/library/sections/$id/all?type=10&title=Opening" | py 'import json,sys; print(json.load(sys.stdin)["MediaContainer"]["Metadata"][0]["ratingKey"])')"
  meta="$(curl -sS -H "$j" "$base/library/metadata/$t?includeAllStreams=1")"
  echo "track $t: $meta" | cut -c1-2500
  for sk in $(py 'import json,sys
m=json.loads(sys.argv[1])["MediaContainer"]["Metadata"][0]
print(" ".join(s.get("key","") for md in m.get("Media",[]) for p in md.get("Part",[]) for s in p.get("Stream",[]) if s.get("streamType")==4))' "$meta"); do
    echo "--- lyrics $sk"; curl -sS -w ' %{http_code}\n' "$base$sk"; curl -sS -w ' %{http_code}\n' -H "$j" "$base$sk"; curl -sS -w ' %{http_code}\n' "$base$sk?format=lrc"
  done
  al="$(py 'import json,sys
for m in json.loads(sys.argv[1])["MediaContainer"].get("Metadata",[]):
  if m["title"]=="First Light": print(m["ratingKey"])' "$albums")"
  thumb="$(py 'import json,sys
for m in json.loads(sys.argv[1])["MediaContainer"].get("Metadata",[]):
  if m["title"]=="First Light": print(m.get("thumb") or "")' "$albums")"
  for u in "/library/metadata/$al/thumb" "$thumb" "/photo/:/transcode?width=64&height=64&minSize=1&upscale=1&url=%2Flibrary%2Fmetadata%2F$al%2Fthumb" "/photo/:/transcode?width=64&height=64&minSize=1&upscale=1&format=png&url=%2Flibrary%2Fmetadata%2F$al%2Fthumb"; do
    [ -n "$u" ] || continue
    curl -sS -o "$RUNNER_TEMP/img" -w "$u: %{http_code} %{size_download} %{content_type}\n" "$base$u"; head -c 8 "$RUNNER_TEMP/img" | od -An -tx1
  done
done

id="$(curl -sS -H "$j" "$base/library/sections" | py 'import json,sys; print(json.load(sys.stdin)["MediaContainer"]["Directory"][0]["key"])')"
keys="$(curl -sS -H "$j" "$base/library/sections/$id/all?type=10" | py 'import json,sys; print(",".join(m["ratingKey"] for m in json.load(sys.stdin)["MediaContainer"]["Metadata"]))')"
k1="${keys%%,*}"; rest="${keys#*,}"; k2="${rest%%,*}"
echo "--- rate $k1 at 8, $k2 at 9"
curl -sS -o /dev/null -w '%{http_code}\n' -X PUT -H "$j" "$base/:/rate?key=$k1&identifier=com.plexapp.plugins.library&rating=8"
curl -sS -o /dev/null -w '%{http_code}\n' -X PUT -H "$j" "$base/:/rate?key=$k2&identifier=com.plexapp.plugins.library&rating=9"
for f in "userRating>>=8" "userRating>=8" "userRating>>=9" "userRating<<=9" "userRating=8" "userRating!=0"; do
  echo "filter $f: $(curl -sS -H "$j" "$base/library/sections/$id/all?type=10&$f" | py 'import json,sys; c=json.load(sys.stdin)["MediaContainer"]; print(c.get("size"), [m.get("userRating") for m in c.get("Metadata",[])])' 2>&1 | tail -1)"
done
mid="$(curl -sS -H "$j" "$base/identity" | py 'import json,sys; print(json.load(sys.stdin)["MediaContainer"]["machineIdentifier"])')"
echo "--- empty playlist"
curl -sS -w ' %{http_code}\n' -X POST -H "$j" "$base/playlists?type=audio&title=Empty&smart=0"
curl -sS -w ' %{http_code}\n' -X POST -H "$j" "$base/playlists?type=audio&title=Empty2&smart=0&uri="
curl -sS -H "$j" "$base/playlists?playlistType=audio"; echo
echo "--- transcodes"
for path in "/music/:/transcode/universal/start.mp3" "/music/:/transcode/universal/start" "/audio/:/transcode/universal/start.mp3" "/music/:/transcode/universal/start.m4a" "/music/:/transcode/universal/start.aac" "/music/:/transcode/universal/start.ogg" "/music/:/transcode/universal/start.opus"; do
  q="path=%2Flibrary%2Fmetadata%2F$k1&mediaIndex=0&partIndex=0&protocol=http&directPlay=0&directStream=0&musicBitrate=64&maxAudioBitrate=64&session=ci$RANDOM&X-Plex-Client-Identifier=ci-probe&X-Plex-Product=Encore&X-Plex-Platform=Chrome"
  curl -sS -o "$RUNNER_TEMP/tc" -w "$path: %{http_code} %{size_download} %{content_type}\n" "${H[@]}" "$base$path?$q" --max-time 20
  head -c 16 "$RUNNER_TEMP/tc" | od -An -tx1; head -c 300 "$RUNNER_TEMP/tc" | tr -cd '[:print:]\n' | head -c 300; echo
done
echo "--- decision"
curl -sS -w ' %{http_code}\n' -H "$j" "${H[@]}" "$base/music/:/transcode/universal/decision?path=%2Flibrary%2Fmetadata%2F$k1&mediaIndex=0&partIndex=0&protocol=http&directPlay=0&directStream=0&musicBitrate=64&maxAudioBitrate=64&session=ci-d" | head -c 3000; echo
echo "--- with a profile extra"
curl -sS -o "$RUNNER_TEMP/tc" -w "%{http_code} %{size_download} %{content_type}\n" "${H[@]}" --max-time 20 \
  "$base/music/:/transcode/universal/start.mp3?path=%2Flibrary%2Fmetadata%2F$k1&mediaIndex=0&partIndex=0&protocol=http&directPlay=0&directStream=0&musicBitrate=64&maxAudioBitrate=64&session=ci-x&X-Plex-Client-Profile-Extra=add-transcode-target(type%3DmusicProfile%26context%3Dstreaming%26protocol%3Dhttp%26container%3Dmp3%26audioCodec%3Dmp3)"
head -c 16 "$RUNNER_TEMP/tc" | od -An -tx1
find "$RUNNER_TEMP/plex-config" -name 'Plex Media Server.log' -exec grep -iE 'transcode|lyric|lrc|ERROR' {} \; | cut -c1-300 | tail -n 60
