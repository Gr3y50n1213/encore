#!/usr/bin/env bash
# TEMPORARY: what a real Emby answers, for writing the adapter (Phase 12 stage 3). Round 2.
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
pw2="$(openssl rand -hex 16)"; echo "::add-mask::$pw2"
e POST /emby/Startup/Configuration '{"UICulture":"en-US","MetadataCountryCode":"US","PreferredMetadataLanguage":"en"}'
e GET /emby/Startup/User
e POST /emby/Startup/User "{\"Name\":\"admin\",\"Password\":\"$pw\"}"
e POST /emby/Startup/Complete
LIMIT=0 e POST /emby/Users/AuthenticateByName "{\"Username\":\"admin\",\"Pw\":\"$pw\"}"
TOKEN="$(jq -r .AccessToken /tmp/b)"; echo "::add-mask::$TOKEN"
ADMIN="$(jq -r .User.Id /tmp/b)"
ADMIN_TOKEN="$TOKEN"
e POST /emby/Users/New '{"Name":"listener"}'
LISTENER="$(jq -r .Id /tmp/b)"
e POST "/emby/Users/$LISTENER/Password" "{\"Id\":\"$LISTENER\",\"CurrentPw\":\"\",\"NewPw\":\"$pw2\"}"
e POST "/emby/Library/VirtualFolders?name=Music&collectionType=music&refreshLibrary=true&paths=%2Fmusic" '{"LibraryOptions":{"EnableRealtimeMonitor":false}}'
for i in $(seq 1 60); do
  LIMIT=0 e GET "/emby/Items?Recursive=true&IncludeItemTypes=MusicAlbum&UserId=$ADMIN&EnableTotalRecordCount=true" > /dev/null
  echo "$i: $(jq -c '[.Items[] | {Name, ImageTags}]' /tmp/b)"
  jq -e '.Items[] | select(.ImageTags.Primary != null)' /tmp/b > /dev/null && break
  sleep 3
done
LIMIT=0 e GET /emby/ScheduledTasks; jq -c '.[] | {Name, State, LastExecutionResult: .LastExecutionResult.Status}' /tmp/b | head -40
LIMIT=0 e GET "/emby/Items?Recursive=true&IncludeItemTypes=Audio&UserId=$ADMIN&Fields=MediaStreams"
jq -c '.Items[] | {Id, Name, AlbumId, ImageTags, Streams: [.MediaStreams[]? | .Type]}' /tmp/b
ALBUM="$(jq -r '.Items[] | select(.Name=="Opening") | .AlbumId' /tmp/b)"
curl -sS -o /tmp/img -D /tmp/h -w 'png cover: %{http_code}\n' -H "X-Emby-Token: $TOKEN" "$B/emby/Items/$ALBUM/Images/Primary?maxWidth=64&maxHeight=64&format=Png"; grep -i content-type /tmp/h; head -c 8 /tmp/img | xxd
curl -sS -o /tmp/img -D /tmp/h -w 'png lower: %{http_code}\n' -H "X-Emby-Token: $TOKEN" "$B/emby/Items/$ALBUM/Images/Primary?maxWidth=64&maxHeight=64&format=png"; grep -i content-type /tmp/h; head -c 8 /tmp/img | xxd
curl -sS -o /tmp/img -D /tmp/h -w 'fill: %{http_code}\n' -H "X-Emby-Token: $TOKEN" "$B/emby/Items/$ALBUM/Images/Primary?fillWidth=64&fillHeight=64&quality=90"; grep -i content-type /tmp/h; head -c 8 /tmp/img | xxd
curl -sS -o /tmp/img -w 'no auth: %{http_code}\n' "$B/emby/Items/$ALBUM/Images/Primary?maxWidth=64"
echo "################ sessions"
LIMIT=0 e GET /emby/Sessions; jq -c '.[] | {UserId, UserName, DeviceId, Client}' /tmp/b
e GET /emby/Sessions/Current
e GET /emby/Users/Current
LIMIT=300 e GET /emby/System/Info
TOKEN=
A='MediaBrowser Client="Encore", Device="Phone", DeviceId="ci-listener", Version="0.1.0"'
LIMIT=0 e POST /emby/Users/AuthenticateByName "{\"Username\":\"listener\",\"Pw\":\"$pw2\"}"
TOKEN="$(jq -r .AccessToken /tmp/b)"; echo "::add-mask::$TOKEN"
LT="$TOKEN"
TOKEN="$ADMIN_TOKEN"; A='MediaBrowser Client="ci", Device="ci", DeviceId="ci-setup", Version="1.0"'
LIMIT=0 e GET /emby/Sessions; jq -c '.[] | {UserId, UserName, DeviceId, Client}' /tmp/b
echo "the listener's token, sent by another device (the companion):"
TOKEN="$LT"; A='MediaBrowser Client="Encore companion", Device="encore-server", DeviceId="encore-server", Version="0.1.0"'
LIMIT=0 e GET /emby/Sessions; jq -c '.[] | {UserId, UserName, DeviceId, Client}' /tmp/b
LIMIT=300 e GET "/emby/Users/$LISTENER"
echo "the admin's token, sent by the companion:"
TOKEN="$ADMIN_TOKEN"
LIMIT=0 e GET /emby/Sessions; jq -c '.[] | {UserId, UserName, DeviceId, Client}' /tmp/b
TOKEN="$LT"; A='MediaBrowser Client="Encore", Device="Phone", DeviceId="ci-listener", Version="0.1.0"'
echo "################ the listener's played items"
LIMIT=0 e GET "/emby/Items?UserId=$LISTENER&Recursive=true&IncludeItemTypes=Audio"
SONG="$(jq -r '.Items[] | select(.Name=="Opening") | .Id' /tmp/b)"
S2="$(jq -r '.Items[] | select(.Name=="Middle") | .Id' /tmp/b)"
S3="$(jq -r '.Items[] | select(.Name=="Closing") | .Id' /tmp/b)"
S4="$(jq -r '.Items[] | select(.Name=="Hum") | .Id' /tmp/b)"
e POST "/emby/Users/$LISTENER/PlayedItems/$S4"
e POST "/emby/Users/$LISTENER/PlayedItems/$S4"
LIMIT=0 e GET "/emby/Items?UserId=$LISTENER&Recursive=true&IncludeItemTypes=Audio&Filters=IsPlayed&EnableUserData=true"; jq -c '.Items[] | {Name, UserData}' /tmp/b
LIMIT=0 e GET "/emby/Items?UserId=$LISTENER&Recursive=true&IncludeItemTypes=Audio&Filters=IsPlayed&EnableUserData=true&Fields=UserData"; jq -c '.Items[] | {Name, UserData}' /tmp/b
LIMIT=0 e GET "/emby/Users/$LISTENER/Items?Recursive=true&IncludeItemTypes=Audio&Filters=IsPlayed"; jq -c '.Items[] | {Name, UserData}' /tmp/b
LIMIT=0 e GET "/emby/Users/$LISTENER/Items/$S4"; jq -c '.UserData' /tmp/b
e POST "/emby/Sessions/Playing" "{\"ItemId\":\"$S3\",\"PositionTicks\":0,\"PlaySessionId\":\"x1\",\"MediaSourceId\":\"mediasource_$S3\",\"CanSeek\":true,\"PlayMethod\":\"DirectStream\"}"
e POST "/emby/Sessions/Playing/Stopped" "{\"ItemId\":\"$S3\",\"PositionTicks\":190000000,\"PlaySessionId\":\"x1\",\"MediaSourceId\":\"mediasource_$S3\"}"
LIMIT=0 e GET "/emby/Users/$LISTENER/Items/$S3"; jq -c '.UserData' /tmp/b
echo "################ playlist entry ids around a move"
pl() { LIMIT=0 e GET "/emby/Playlists/$PL/Items?UserId=$LISTENER" > /dev/null; echo "  $1: $(jq -c '[.Items[] | [.Name, .PlaylistItemId]]' /tmp/b)"; }
e POST "/emby/Playlists?Name=Moves&Ids=$SONG,$S2,$S3&MediaType=Audio&UserId=$LISTENER"
PL="$(jq -r .Id /tmp/b)"
pl created
E3="$(jq -r '.Items[2].PlaylistItemId' /tmp/b)"
e POST "/emby/Playlists/$PL/Items/$E3/Move/0"
pl "after moving Closing to 0"
sleep 5
pl "5 s later"
LIMIT=0 e GET "/emby/Playlists/$PL/Items?UserId=$LISTENER&Fields=SortName"; echo "  other fields: $(jq -c '[.Items[] | [.Name, .PlaylistItemId]]' /tmp/b)"
LIMIT=0 e GET "/emby/Users/$LISTENER/Items?ParentId=$PL"; echo "  as children: $(jq -c '[.Items[] | [.Name, .PlaylistItemId]]' /tmp/b)"
E_MID="$(LIMIT=0 e GET "/emby/Playlists/$PL/Items?UserId=$LISTENER" > /dev/null; jq -r '.Items[] | select(.Name=="Middle") | .PlaylistItemId' /tmp/b)"
echo "removing Middle by its listed id $E_MID"
e DELETE "/emby/Playlists/$PL/Items?EntryIds=$E_MID"
pl "after removing Middle"
e POST "/emby/Playlists/$PL/Items?Ids=$S4&UserId=$LISTENER"
pl "after adding Hum"
e POST "/emby/Playlists/$PL/Items/$(jq -r '.Items[2].PlaylistItemId' /tmp/b)/Move/1"
pl "after moving the third to 1"
E_LAST="$(jq -r '.Items[2].PlaylistItemId' /tmp/b)"; N_LAST="$(jq -r '.Items[2].Name' /tmp/b)"
echo "removing $N_LAST by its listed id $E_LAST"
e DELETE "/emby/Playlists/$PL/Items?EntryIds=$E_LAST"
pl "after removing the third"
echo "################ a second playlist, two moves in a row"
e POST "/emby/Playlists?Name=Moves2&Ids=$SONG,$S2,$S3,$S4&MediaType=Audio&UserId=$LISTENER"
PL="$(jq -r .Id /tmp/b)"
pl created
e POST "/emby/Playlists/$PL/Items/$(jq -r '.Items[0].PlaylistItemId' /tmp/b)/Move/3"
pl "after moving the first to 3"
e POST "/emby/Playlists/$PL/Items/$(jq -r '.Items[0].PlaylistItemId' /tmp/b)/Move/2"
pl "after moving the first to 2"
docker restart emby > /dev/null; for _ in $(seq 1 60); do curl -fs "$B/emby/System/Info/Public" > /dev/null && break; sleep 2; done
pl "after a restart"
echo "################ public info errors"
TOKEN=
e GET /emby/System/Info/Public
e GET /emby/system/info/public
e POST /emby/Users/AuthenticateByName '{"Username":"listener","Pw":"wrong"}'
e GET /emby/Users/AuthenticateByName
docker logs emby 2>&1 | grep -iE "image|error|warn" | grep -viE "api_key|apikey|token" | tail -n 40
