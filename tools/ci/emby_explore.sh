#!/usr/bin/env bash
# TEMPORARY: what a real Emby answers, for writing the adapter (Phase 12 stage 3). Round 3.
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
for i in $(seq 1 40); do
  LIMIT=0 e GET "/emby/Items?Recursive=true&IncludeItemTypes=MusicAlbum&UserId=$ADMIN&EnableTotalRecordCount=true" > /dev/null
  echo "$i: $(jq -c '[.Items[] | {Name, ImageTags}]' /tmp/b)"
  [ "$(jq '[.Items[] | select(.ImageTags.Primary != null)] | length' /tmp/b)" = 2 ] && break
  sleep 3
done
LIMIT=0 e GET "/emby/Items?Recursive=true&IncludeItemTypes=Audio,MusicArtist,Photo&UserId=$ADMIN"
jq -c '.Items[] | {Id, Name, Type, AlbumId, ImageTags, AlbumPrimaryImageTag}' /tmp/b
for name in "First Light" "Low Hum"; do
  LIMIT=0 e GET "/emby/Items?Recursive=true&IncludeItemTypes=MusicAlbum&UserId=$ADMIN" > /dev/null
  AL="$(jq -r --arg n "$name" '.Items[] | select(.Name==$n) | .Id' /tmp/b)"
  TAG="$(jq -r --arg n "$name" '.Items[] | select(.Name==$n) | .ImageTags.Primary' /tmp/b)"
  curl -sS -o /tmp/img -D /tmp/h -w "$name png: %{http_code}\n" -H "X-Emby-Token: $TOKEN" "$B/emby/Items/$AL/Images/Primary?maxWidth=64&maxHeight=64&format=Png&tag=$TAG"; grep -i content-type /tmp/h; head -c 8 /tmp/img | xxd
  curl -sS -o /tmp/img -D /tmp/h -w "$name fill: %{http_code}\n" -H "X-Emby-Token: $TOKEN" "$B/emby/Items/$AL/Images/Primary?fillWidth=64&fillHeight=64&quality=90"; grep -i content-type /tmp/h; head -c 8 /tmp/img | xxd
  curl -sS -o /tmp/img -w "$name, no token: %{http_code}\n" "$B/emby/Items/$AL/Images/Primary?maxWidth=64"
done
echo "################ playlist entry ids around a move, second by second"
LIMIT=0 e GET "/emby/Items?UserId=$ADMIN&Recursive=true&IncludeItemTypes=Audio"
SA="$(jq -r '.Items[] | select(.Name=="Opening") | .Id' /tmp/b)"
SB="$(jq -r '.Items[] | select(.Name=="Middle") | .Id' /tmp/b)"
SC="$(jq -r '.Items[] | select(.Name=="Closing") | .Id' /tmp/b)"
SD="$(jq -r '.Items[] | select(.Name=="Hum") | .Id' /tmp/b)"
pl() { LIMIT=0 e GET "/emby/Playlists/$PL/Items?UserId=$ADMIN$2" > /dev/null; echo "  $1: $(jq -c '[.Items[] | [.Name, .PlaylistItemId]]' /tmp/b)"; }
for round in 1 2 3; do
  e POST "/emby/Playlists?Name=Moves$round&Ids=$SA,$SB,$SC,$SD&MediaType=Audio&UserId=$ADMIN"
  PL="$(jq -r .Id /tmp/b)"
  pl created ""
  LAST="$(jq -r '.Items[3].PlaylistItemId' /tmp/b)"
  e POST "/emby/Playlists/$PL/Items/$LAST/Move/0"
  pl "at once" ""
  pl "at once, ParentId listing" "&ParentId=$PL"
  pl "at once, another query" "&StartIndex=0&Limit=50&Fields=Path"
  if [ "$round" = 1 ]; then
    for s in $(seq 1 12); do sleep 1; pl "$s s" ""; done
  elif [ "$round" = 2 ]; then
    MID="$(jq -r '.Items[] | select(.Name=="Middle") | .PlaylistItemId' /tmp/b)"
    echo "  removing Middle at once by its listed id $MID"
    e DELETE "/emby/Playlists/$PL/Items?EntryIds=$MID"
    pl "after" ""; sleep 8; pl "8 s later" ""
  else
    sleep 0.5
    MID="$(jq -r '.Items[] | select(.Name=="Middle") | .PlaylistItemId' /tmp/b)"
    echo "  moving Middle at once by its listed id $MID to 0"
    e POST "/emby/Playlists/$PL/Items/$MID/Move/0"
    pl "after" ""; sleep 8; pl "8 s later" ""
  fi
done
docker logs emby 2>&1 | grep -iE "playlist|image|error" | grep -viE "api_key|apikey|token" | tail -n 40
