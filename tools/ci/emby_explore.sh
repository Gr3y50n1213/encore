#!/usr/bin/env bash
# TEMPORARY: what a real Emby answers, for writing the adapter (Phase 12 stage 3). Round 4.
set -u
B=http://127.0.0.1:8096
A='MediaBrowser Client="ci", Device="ci", DeviceId="ci-setup", Version="1.0"'
TOKEN=
e() { # method path [json body]
  local args=(-sS -X "$1" "$B$2" -H "X-Emby-Authorization: $A" -D /tmp/h -o /tmp/b -w '%{http_code}')
  [ -n "$TOKEN" ] && args+=(-H "X-Emby-Token: $TOKEN")
  [ $# -ge 3 ] && args+=(-H 'Content-Type: application/json' -d "$3")
  code="$(curl "${args[@]}")"
  echo "== $1 $2 -> $code ($(grep -i '^content-type' /tmp/h | tr -d '\r'))"
  head -c "${LIMIT:-1500}" /tmp/b
  echo
}
pw="$(openssl rand -hex 16)"; echo "::add-mask::$pw"
e POST /emby/Startup/Configuration '{"UICulture":"en-US","MetadataCountryCode":"US","PreferredMetadataLanguage":"en"}'
e GET /emby/Startup/User
e POST /emby/Startup/User "{\"Name\":\"admin\",\"Password\":\"$pw\"}"
e POST /emby/Startup/Complete
LIMIT=0 e POST /emby/Users/AuthenticateByName "{\"Username\":\"admin\",\"Pw\":\"$pw\"}"
TOKEN="$(jq -r .AccessToken /tmp/b)"; echo "::add-mask::$TOKEN"
ADMIN="$(jq -r .User.Id /tmp/b)"
e POST "/emby/Library/VirtualFolders?name=Music&collectionType=music&refreshLibrary=true&paths=%2Fmusic" '{"LibraryOptions":{"EnableRealtimeMonitor":false,"EnablePhotos":false}}'
for i in $(seq 1 20); do
  LIMIT=0 e GET "/emby/Items?Recursive=true&IncludeItemTypes=Audio&UserId=$ADMIN&EnableTotalRecordCount=true" > /dev/null
  [ "$(jq .TotalRecordCount /tmp/b)" = 6 ] && break
  sleep 3
done
sleep 10
LIMIT=0 e GET "/emby/Items?Recursive=true&IncludeItemTypes=MusicAlbum&UserId=$ADMIN&Fields=Path"
jq -c '.Items[] | {Id, Name, Path, ImageTags}' /tmp/b
AL="$(jq -r '.Items[] | select(.Name=="First Light") | .Id' /tmp/b)"
AL2="$(jq -r '.Items[] | select(.Name=="Low Hum") | .Id' /tmp/b)"
LIMIT=3000 e GET "/emby/Items/$AL/RemoteImages/Providers"
LIMIT=3000 e GET "/emby/Items/$AL/Images"
LIMIT=3000 e GET "/emby/Items/$AL/MetadataEditor"
e POST "/emby/Items/$AL/Refresh?Recursive=true&MetadataRefreshMode=FullRefresh&ImageRefreshMode=FullRefresh&ReplaceAllMetadata=false&ReplaceAllImages=true"
e POST "/emby/Items/$AL2/Refresh?Recursive=true&MetadataRefreshMode=FullRefresh&ImageRefreshMode=FullRefresh&ReplaceAllMetadata=false&ReplaceAllImages=true"
for i in $(seq 1 20); do
  LIMIT=0 e GET "/emby/Items?Recursive=true&IncludeItemTypes=MusicAlbum,Audio&UserId=$ADMIN" > /dev/null
  echo "$i: $(jq -c '[.Items[] | {Name, ImageTags, AlbumPrimaryImageTag}]' /tmp/b)"
  [ "$(jq '[.Items[] | select(.ImageTags.Primary != null)] | length' /tmp/b)" -ge 1 ] && break
  sleep 3
done
LIMIT=3000 e GET "/emby/Items/$AL/Images"
echo "################ upload a cover through the API"
base64 -w0 "$MUSIC/Tone Choir/First Light/cover.jpg" > /tmp/cover.b64
curl -sS -X POST "$B/emby/Items/$AL/Images/Primary" -H "X-Emby-Token: $TOKEN" -H 'Content-Type: image/jpeg' --data-binary @/tmp/cover.b64 -w 'upload: %{http_code}\n'
sleep 2
LIMIT=0 e GET "/emby/Items?Recursive=true&IncludeItemTypes=MusicAlbum&UserId=$ADMIN"; jq -c '.Items[] | {Id, Name, ImageTags}' /tmp/b
TAG="$(jq -r '.Items[] | select(.Name=="First Light") | .ImageTags.Primary' /tmp/b)"
curl -sS -o /tmp/img -D /tmp/h -w "png: %{http_code}\n" -H "X-Emby-Token: $TOKEN" "$B/emby/Items/$AL/Images/Primary?maxWidth=64&maxHeight=64&format=Png&tag=$TAG"; grep -i content-type /tmp/h; head -c 8 /tmp/img | xxd
curl -sS -o /tmp/img -D /tmp/h -w "fill: %{http_code}\n" -H "X-Emby-Token: $TOKEN" "$B/emby/Items/$AL/Images/Primary?fillWidth=64&fillHeight=64&quality=90"; grep -i content-type /tmp/h; head -c 8 /tmp/img | xxd
curl -sS -o /tmp/img -w "no token: %{http_code}\n" "$B/emby/Items/$AL/Images/Primary?maxWidth=64"
echo "################ the listing cache"
LIMIT=0 e GET "/emby/Items?UserId=$ADMIN&Recursive=true&IncludeItemTypes=Audio"
SA="$(jq -r '.Items[] | select(.Name=="Opening") | .Id' /tmp/b)"
SB="$(jq -r '.Items[] | select(.Name=="Middle") | .Id' /tmp/b)"
SC="$(jq -r '.Items[] | select(.Name=="Closing") | .Id' /tmp/b)"
pl() { LIMIT=0 e GET "/emby/Playlists/$PL/Items?userId=$ADMIN$2" > /dev/null; echo "  $1: $(jq -c '[.Items[] | [.Name, .PlaylistItemId]]' /tmp/b)"; }
e POST "/emby/Playlists?name=Cache&ids=$SA,$SB,$SC&mediaType=Audio&userId=$ADMIN"
PL="$(jq -r .Id /tmp/b)"
pl created ""
pl "created, nonce 1" "&encoreFresh=1"
e POST "/emby/Playlists/$PL/Items/$(jq -r '.Items[2].PlaylistItemId' /tmp/b)/Move/0"
pl "moved, same query" ""
pl "moved, same query and nonce 1" "&encoreFresh=1"
pl "moved, nonce 2" "&encoreFresh=2"
e DELETE "/emby/Playlists/$PL/Items?entryIds=$(jq -r '.Items[1].PlaylistItemId' /tmp/b)"
pl "removed the second, nonce 3" "&encoreFresh=3"
for s in 1 2 3 4 5 6 8 10 15; do sleep 1; pl "$s" "&encoreFresh=r$s"; done
docker logs emby 2>&1 | grep -iE "image|jpg|cover|folder" | grep -viE "api_key|apikey|token" | tail -n 40
