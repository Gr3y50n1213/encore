#!/usr/bin/env bash
# Discovery only (removed before merge): what an unclaimed Plex Media Server answers.
set -u
base="$PLEX_URL"; j='Accept: application/json'
py() { python3 -c "$1" "${2:-}"; }
curl -sS -w ' %{http_code}\n' -X POST -H "$j" \
  "$base/library/sections?name=Music&type=artist&agent=tv.plex.agents.music&scanner=Plex%20Music&language=en-US&location=%2Fmusic"
id="$(curl -sS -H "$j" "$base/library/sections" | py 'import json,sys; print(json.load(sys.stdin)["MediaContainer"]["Directory"][0]["key"])')"
echo "section $id"
curl -sS -H "$j" "$base/library/sections/$id/refresh" -w ' %{http_code}\n'
for _ in $(seq 1 90); do
  n="$(curl -sS -H "$j" "$base/library/sections/$id/all?type=10" | py 'import json,sys; print(json.load(sys.stdin)["MediaContainer"].get("size",0))')"
  echo "tracks: $n"; [ "$n" = 6 ] && break; sleep 2
done
sleep 20
echo "--- sections"; curl -sS -H "$j" "$base/library/sections"; echo
echo "--- prefs"; curl -sS -H "$j" "$base/library/sections/$id/prefs"; echo
echo "--- artists"; curl -sS -H "$j" "$base/library/sections/$id/all?type=8"; echo
echo "--- albums"; curl -sS -H "$j" "$base/library/sections/$id/all?type=9"; echo
echo "--- tracks page"; curl -sS -H "$j" "$base/library/sections/$id/all?type=10&X-Plex-Container-Start=0&X-Plex-Container-Size=2"; echo
t="$(curl -sS -H "$j" "$base/library/sections/$id/all?type=10&title=Opening" | py 'import json,sys; print(json.load(sys.stdin)["MediaContainer"]["Metadata"][0]["ratingKey"])')"
echo "--- track $t"; curl -sS -H "$j" "$base/library/metadata/$t"; echo
meta="$(curl -sS -H "$j" "$base/library/metadata/$t?includeAllStreams=1")"
echo "--- track $t all streams"; echo "$meta"
part="$(py 'import json,sys; print(json.loads(sys.argv[1])["MediaContainer"]["Metadata"][0]["Media"][0]["Part"][0]["key"])' "$meta")"
pid="$(py 'import json,sys; print(json.loads(sys.argv[1])["MediaContainer"]["Metadata"][0]["Media"][0]["Part"][0]["id"])' "$meta")"
echo "--- part $part"
for u in "$part" "/library/parts/$pid/file.mp3" "/library/parts/$pid/0/file.mp3" "/library/parts/$pid/file" "/library/metadata/$t/file" "$part?download=1"; do
  echo "$u: $(curl -sS -o /dev/null -w '%{http_code} %{size_download} %{content_type}' "$base$u")"
done
echo "range: $(curl -sS -o /dev/null -w '%{http_code} %{size_download}' -H 'Range: bytes=0-99' "$base$part")"
for sk in $(py 'import json,sys
m=json.loads(sys.argv[1])["MediaContainer"]["Metadata"][0]
print(" ".join(s.get("key","") for md in m.get("Media",[]) for p in md.get("Part",[]) for s in p.get("Stream",[]) if s.get("streamType")==4))' "$meta"); do
  echo "--- lyrics $sk"; curl -sS -w ' %{http_code}\n' "$base$sk"; curl -sS -w ' %{http_code}\n' -H "$j" "$base$sk"
done
al="$(curl -sS -H "$j" "$base/library/sections/$id/all?type=9&title=First%20Light" | py 'import json,sys; print(json.load(sys.stdin)["MediaContainer"]["Metadata"][0]["ratingKey"])')"
echo "--- album $al"; curl -sS -H "$j" "$base/library/metadata/$al"; echo
for u in "/library/metadata/$al/thumb" "/photo/:/transcode?width=64&height=64&minSize=1&upscale=1&url=%2Flibrary%2Fmetadata%2F$al%2Fthumb" "/photo/:/transcode?width=64&height=64&minSize=1&upscale=1&format=png&url=%2Flibrary%2Fmetadata%2F$al%2Fthumb"; do
  curl -sS -o "$RUNNER_TEMP/img" -w "$u: %{http_code} %{size_download} %{content_type}\n" "$base$u"; head -c 8 "$RUNNER_TEMP/img" | od -An -tx1
done
mid="$(curl -sS -H "$j" "$base/identity" | py 'import json,sys; print(json.load(sys.stdin)["MediaContainer"]["machineIdentifier"])')"
keys="$(curl -sS -H "$j" "$base/library/sections/$id/all?type=10" | py 'import json,sys; print(",".join(m["ratingKey"] for m in json.load(sys.stdin)["MediaContainer"]["Metadata"]))')"
k1="${keys%%,*}"
uri() { echo "server%3A%2F%2F$mid%2Fcom.plexapp.plugins.library%2Flibrary%2Fmetadata%2F$1"; }
echo "--- create playlist from $keys"
curl -sS -w ' %{http_code}\n' -X POST -H "$j" "$base/playlists?type=audio&title=Probe&smart=0&uri=$(uri "$keys")"
pl="$(curl -sS -H "$j" "$base/playlists?playlistType=audio" | py 'import json,sys; print(json.load(sys.stdin)["MediaContainer"]["Metadata"][0]["ratingKey"])')"
echo "--- playlists"; curl -sS -H "$j" "$base/playlists?playlistType=audio"; echo
items="$(curl -sS -H "$j" "$base/playlists/$pl/items")"; echo "--- playlist items"; echo "$items"
ids="$(py 'import json,sys; print(" ".join(str(m["playlistItemID"]) for m in json.loads(sys.argv[1])["MediaContainer"]["Metadata"]))' "$items")"
set -- $ids
echo "--- move $3 after $1"; curl -sS -w ' %{http_code}\n' -X PUT -H "$j" "$base/playlists/$pl/items/$3/move?after=$1"
echo "--- move $2 to top"; curl -sS -w ' %{http_code}\n' -X PUT -H "$j" "$base/playlists/$pl/items/$2/move"
echo "--- remove $4"; curl -sS -w ' %{http_code}\n' -X DELETE -H "$j" "$base/playlists/$pl/items/$4"
echo "--- add $k1"; curl -sS -w ' %{http_code}\n' -X PUT -H "$j" "$base/playlists/$pl/items?uri=$(uri "$k1")"
echo "--- rename"; curl -sS -w ' %{http_code}\n' -X PUT -H "$j" "$base/playlists/$pl?title=Renamed"
curl -sS -H "$j" "$base/playlists/$pl/items" | py 'import json,sys; [print(m["playlistItemID"], m["ratingKey"], m["title"]) for m in json.load(sys.stdin)["MediaContainer"]["Metadata"]]'
curl -sS -H "$j" "$base/playlists/$pl"; echo
echo "--- rate"; curl -sS -w ' %{http_code}\n' -X PUT -H "$j" "$base/:/rate?key=$k1&identifier=com.plexapp.plugins.library&rating=10"
for f in "userRating>>=9" "userRating>=9" "userRating%3E%3E%3D9" "userRating=10" "userRating>>=8"; do
  echo "filter $f: $(curl -sS -H "$j" "$base/library/sections/$id/all?type=10&$f" | py 'import json,sys; c=json.load(sys.stdin)["MediaContainer"]; print(c.get("size"), [m.get("userRating") for m in c.get("Metadata",[])])')"
done
echo "--- scrobble"; curl -sS -w ' %{http_code}\n' -H "$j" "$base/:/scrobble?key=$k1&identifier=com.plexapp.plugins.library"
curl -sS -H "$j" "$base/library/metadata/$k1" | py 'import json,sys; m=json.load(sys.stdin)["MediaContainer"]["Metadata"][0]; print({k:m.get(k) for k in ("viewCount","lastViewedAt","userRating","lastRatedAt","updatedAt")})'
echo "--- viewCount filter: $(curl -sS -H "$j" "$base/library/sections/$id/all?type=10&viewCount>>=0" | py 'import json,sys; c=json.load(sys.stdin)["MediaContainer"]; print(c.get("size"))')"
echo "--- unrate"; curl -sS -w ' %{http_code}\n' -X PUT -H "$j" "$base/:/rate?key=$k1&identifier=com.plexapp.plugins.library&rating=-1"
curl -sS -H "$j" "$base/library/metadata/$k1" | py 'import json,sys; m=json.load(sys.stdin)["MediaContainer"]["Metadata"][0]; print({k:m.get(k) for k in ("viewCount","lastViewedAt","userRating")})'
echo "--- sections after"; curl -sS -H "$j" "$base/library/sections"; echo
echo "--- transcode"
curl -sS -o "$RUNNER_TEMP/tc" -w '%{http_code} %{size_download} %{content_type}\n' -H 'X-Plex-Client-Identifier: ci-probe' -H 'X-Plex-Product: Encore' -H 'X-Plex-Platform: Generic' \
  "$base/music/:/transcode/universal/start.mp3?path=%2Flibrary%2Fmetadata%2F$k1&mediaIndex=0&partIndex=0&protocol=http&directPlay=0&directStream=0&musicBitrate=64&maxAudioBitrate=64&session=ci1"
head -c 16 "$RUNNER_TEMP/tc" | od -An -tx1
echo "--- delete"; curl -sS -w ' %{http_code}\n' -X DELETE -H "$j" "$base/playlists/$pl"
docker logs plex 2>&1 | tail -n 20
find "$RUNNER_TEMP/plex-config" -name 'Plex Media Server.log' -exec grep -iE 'lyric|lrc| error|warn' {} \; | tail -n 60
