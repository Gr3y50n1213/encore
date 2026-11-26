#!/usr/bin/env bash
# Discovery only (removed before merge): covers and lyrics on an unclaimed, offline Plex.
set -u
base="$PLEX_URL"; j='Accept: application/json'
py() { python3 -c "$1" "${2:-}"; }
curl -sS -o /dev/null -w 'create %{http_code}\n' -X POST -H "$j" \
  "$base/library/sections?name=Music&type=artist&agent=tv.plex.agents.none&scanner=Plex%20Music&language=en-US&location=%2Fmusic&prefs%5BrespectTags%5D=1"
id=1
for _ in $(seq 1 60); do
  c="$(curl -sS -H "$j" "$base/library/sections/$id/all?type=10" | py 'import json,sys; print(json.load(sys.stdin)["MediaContainer"].get("size",0))')"
  [ "$c" = 6 ] && break; sleep 2
done
sleep 10
echo "--- prefs"; curl -sS -H "$j" "$base/library/sections/$id/prefs" | py 'import json,sys; [print(s["id"], s.get("value")) for s in json.load(sys.stdin)["MediaContainer"]["Setting"]]'
echo "--- agent config"; curl -sS -H "$j" "$base/system/agents/tv.plex.agents.none/config/8" | head -c 1500; echo
echo "--- providers"; curl -sS -H "$j" "$base/media/providers/metadata" | head -c 1500; echo
show() {
  albums="$(curl -sS -H "$j" "$base/library/sections/$id/all?type=9")"
  py 'import json,sys
for m in json.loads(sys.argv[1])["MediaContainer"].get("Metadata",[]): print("album", m["ratingKey"], m["title"], m.get("thumb"))' "$albums"
  t="$(curl -sS -H "$j" "$base/library/sections/$id/all?type=10&title=Opening" | py 'import json,sys; print(json.load(sys.stdin)["MediaContainer"]["Metadata"][0]["ratingKey"])')"
  curl -sS -H "$j" "$base/library/metadata/$t?includeAllStreams=1" | py 'import json,sys
m=json.load(sys.stdin)["MediaContainer"]["Metadata"][0]
print("track", m["ratingKey"], m.get("thumb"), m.get("parentThumb"))
for md in m.get("Media",[]):
  for p in md.get("Part",[]):
    for s in p.get("Stream",[]): print("  stream", json.dumps(s))'
}
show
echo "--- refresh the section, forced"
curl -sS -o /dev/null -w '%{http_code}\n' -X PUT -H "$j" "$base/library/sections/$id/refresh?force=1"
sleep 30; show
al="$(curl -sS -H "$j" "$base/library/sections/$id/all?type=9&title=First%20Light" | py 'import json,sys; print(json.load(sys.stdin)["MediaContainer"]["Metadata"][0]["ratingKey"])')"
echo "--- refresh album $al"
curl -sS -o /dev/null -w '%{http_code}\n' -X PUT -H "$j" "$base/library/metadata/$al/refresh?force=1"
sleep 20; show
echo "--- album posters"; curl -sS -H "$j" "$base/library/metadata/$al/posters" | head -c 1500; echo
echo "--- upload the cover"
curl -sS -w ' %{http_code}\n' -X POST -H "$j" -H 'Content-Type: image/jpeg' --data-binary "@$RUNNER_TEMP/music/Tone Choir/First Light/cover.jpg" "$base/library/metadata/$al/posters"
sleep 5; show
thumb="$(curl -sS -H "$j" "$base/library/metadata/$al" | py 'import json,sys; print(json.load(sys.stdin)["MediaContainer"]["Metadata"][0].get("thumb") or "")')"
echo "thumb: $thumb"
for u in "/library/metadata/$al/thumb" "$thumb" "/photo/:/transcode?width=64&height=64&minSize=1&upscale=1&url=%2Flibrary%2Fmetadata%2F$al%2Fthumb" "/photo/:/transcode?width=64&height=64&minSize=1&upscale=1&format=png&url=%2Flibrary%2Fmetadata%2F$al%2Fthumb" "/photo/:/transcode?width=64&height=64&minSize=1&upscale=1&format=png&url=$(py 'import sys,urllib.parse; print(urllib.parse.quote(sys.argv[1], safe=""))' "$thumb")"; do
  [ -n "$u" ] || continue
  curl -sS -o "$RUNNER_TEMP/img" -w "$u: %{http_code} %{size_download} %{content_type}\n" "$base$u"; head -c 8 "$RUNNER_TEMP/img" | od -An -tx1
done
echo "--- filter with an encoded key"
curl -sS -H "$j" "$base/library/sections/$id/all?type=10&userRating%3E%3E=8" -w ' %{http_code}\n' | head -c 300; echo
echo "--- transcode decision with the profile extra"
k="$(curl -sS -H "$j" "$base/library/sections/$id/all?type=10&title=Hum" | py 'import json,sys; print(json.load(sys.stdin)["MediaContainer"]["Metadata"][0]["ratingKey"])')"
extra='add-transcode-target(type%3DmusicProfile%26context%3Dstreaming%26protocol%3Dhttp%26container%3Dmp3%26audioCodec%3Dmp3)'
q="path=%2Flibrary%2Fmetadata%2F$k&mediaIndex=0&partIndex=0&protocol=http&directPlay=0&directStream=0&musicBitrate=64&maxAudioBitrate=64&session=ci-d&X-Plex-Client-Identifier=ci-probe&X-Plex-Product=Encore&X-Plex-Platform=Generic&X-Plex-Client-Profile-Extra=$extra"
curl -sS -H "$j" "$base/music/:/transcode/universal/decision?$q" | head -c 1200; echo
ls -la "$RUNNER_TEMP/plex-config/Library/Application Support/Plex Media Server/Codecs" 2>&1 | head
find "$RUNNER_TEMP/plex-config" -name 'Plex Media Server*.log' -exec grep -iE 'lrc|lyric|cover|poster|LocalMedia|local media' {} \; | cut -c1-300 | tail -n 40
