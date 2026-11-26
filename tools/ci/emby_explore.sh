#!/usr/bin/env bash
# TEMPORARY: what a real Emby answers, for writing the adapter (Phase 12 stage 3).
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
e GET /System/Info/Public
e GET /emby/System/Info/Public
grep -i '^date' /tmp/h
e GET /emby/Startup/Configuration
e POST /emby/Startup/Configuration '{"UICulture":"en-US","MetadataCountryCode":"US","PreferredMetadataLanguage":"en"}'
e GET /emby/Startup/User
e POST /emby/Startup/User "{\"Name\":\"admin\",\"Password\":\"$pw\"}"
e POST /emby/Startup/RemoteAccess '{"EnableRemoteAccess":false,"EnableAutomaticPortMapping":false}'
e POST /emby/Startup/Complete
e GET /emby/System/Info/Public
LIMIT=0 e POST /emby/Users/AuthenticateByName "{\"Username\":\"admin\",\"Pw\":\"$pw\"}"
TOKEN="$(jq -r .AccessToken /tmp/b)"; echo "::add-mask::$TOKEN"
jq 'del(.AccessToken) | .User |= {Id, Name, ServerId, Policy: (.Policy|{IsAdministrator,EnableAllFolders})} | del(.SessionInfo)' /tmp/b
ADMIN="$(jq -r .User.Id /tmp/b)"
e GET /emby/Users/Me
e GET /Users/Me
LIMIT=600 e GET "/emby/Users/$ADMIN"
e GET /emby/QuickConnect/Enabled
LIMIT=400 e GET /emby/Plugins
e POST /emby/Users/New '{"Name":"listener"}'
LISTENER="$(jq -r .Id /tmp/b)"
e POST "/emby/Users/$LISTENER/Password" "{\"Id\":\"$LISTENER\",\"CurrentPw\":\"\",\"NewPw\":\"$pw2\"}"
e GET /emby/Library/VirtualFolders
e POST "/emby/Library/VirtualFolders?name=Music&collectionType=music&refreshLibrary=true&paths=%2Fmusic" '{"LibraryOptions":{"EnableRealtimeMonitor":false}}'
e GET /emby/Library/VirtualFolders
for _ in $(seq 1 60); do
  LIMIT=0 e GET "/emby/Items?Recursive=true&IncludeItemTypes=Audio&UserId=$ADMIN&Limit=0&EnableTotalRecordCount=true" > /dev/null
  n="$(jq -r .TotalRecordCount /tmp/b)"; echo "songs: $n"
  [ "$n" = 6 ] && break
  sleep 3
done
sleep 20
echo "################ as the admin"
LIMIT=6000 e GET "/emby/Users/$ADMIN/Items?Recursive=true&IncludeItemTypes=Audio&Fields=SortName,Genres,ProviderIds,DateCreated,MediaStreams,MediaSources,Path&Limit=1&SortBy=SortName"
LIMIT=4000 e GET "/emby/Items?UserId=$ADMIN&Recursive=true&IncludeItemTypes=MusicAlbum&Fields=SortName,Genres,ProviderIds,DateCreated&EnableTotalRecordCount=true"
LIMIT=4000 e GET "/emby/Items?UserId=$ADMIN&Recursive=true&IncludeItemTypes=MusicArtist&Fields=SortName,ProviderIds,DateCreated"
LIMIT=3000 e GET "/emby/Artists/AlbumArtists?UserId=$ADMIN"
echo "################ as the listener"
TOKEN=
A='MediaBrowser Client="Encore", Device="Sam%27s%20Pixel", DeviceId="ci-listener", Version="0.1.0"'
LIMIT=0 e POST /emby/Users/AuthenticateByName "{\"Username\":\"listener\",\"Pw\":\"$pw2\"}"
TOKEN="$(jq -r .AccessToken /tmp/b)"; echo "::add-mask::$TOKEN"
jq 'del(.AccessToken) | .User |= {Id, Name, ServerId, PrimaryImageTag, Policy: (.Policy|{IsAdministrator,EnableAllFolders})} | del(.SessionInfo)' /tmp/b
LIMIT=0 e GET /emby/Sessions
jq -c '.[] | {UserId, UserName, DeviceId, DeviceName, Client}' /tmp/b
LIMIT=600 e GET "/emby/Users/$ADMIN"
e GET /emby/Users/Me
LIMIT=600 e GET "/emby/Users/$LISTENER"
e GET "/emby/Users/$ADMIN/Items?Recursive=true&IncludeItemTypes=Audio&Limit=1"
LIMIT=0 e GET "/emby/Items?UserId=$LISTENER&Recursive=true&IncludeItemTypes=Audio&Fields=SortName,MediaStreams,DateCreated&EnableTotalRecordCount=true&SortBy=SortName"
jq -c '.TotalRecordCount, (.Items[] | {Id, Name, Type, AlbumId, HasLyrics: .HasLyrics, Streams: [.MediaStreams[]? | {Type, Codec, Index, IsExternal, Path, DisplayTitle}]})' /tmp/b
SONG="$(jq -r '.Items[] | select(.Name=="Opening") | .Id' /tmp/b)"
OTHER="$(jq -r '.Items[] | select(.Name=="Hum") | .Id' /tmp/b)"
THIRD="$(jq -r '.Items[] | select(.Name=="Drone") | .Id' /tmp/b)"
LIMIT=0 e GET "/emby/Items?UserId=$LISTENER&Recursive=true&IncludeItemTypes=Audio&Fields=MediaStreams,MediaSources&Ids=$SONG"
jq '.Items[0] | {MediaStreams, MediaSources: [.MediaSources[]? | {Id, Container, MediaStreams: [.MediaStreams[]? | {Type, Codec, Index, IsExternal, DeliveryMethod, DeliveryUrl, IsTextSubtitleStream, SupportsExternalStream}]}]}' /tmp/b
MS="$(jq -r '.Items[0].MediaSources[0].Id' /tmp/b)"
IDX="$(jq -r '[.Items[0].MediaStreams[]? | select(.Type=="Subtitle" or .Type=="Lyrics")][0].Index' /tmp/b)"
echo "media source $MS lyric stream index $IDX"
e GET "/emby/Audio/$SONG/Lyrics"
e GET "/emby/Items/$SONG/$MS/Subtitles/$IDX/Stream.lrc"
e GET "/emby/Items/$SONG/$MS/Subtitles/$IDX/Stream.txt"
e GET "/emby/Items/$SONG/$MS/Subtitles/$IDX/Stream.js"
e GET "/emby/Videos/$SONG/$MS/Subtitles/$IDX/Stream.lrc"
e GET "/emby/Items/$SONG/$MS/Subtitles/$IDX/0/Stream.js"
LIMIT=2500 e GET "/emby/Items/$SONG/PlaybackInfo?UserId=$LISTENER"
LIMIT=100 e GET "/emby/Audio/$SONG/stream?static=true"
LIMIT=100 e GET "/emby/Audio/$SONG/stream.mp3?static=true"
LIMIT=100 e GET "/emby/Audio/$SONG/universal?UserId=$LISTENER&DeviceId=ci-listener&Container=flac&MaxStreamingBitrate=128000&AudioBitRate=128000&AudioCodec=aac&TranscodingContainer=aac&TranscodingProtocol=http"
grep -iE '^(content-type|content-length|accept-ranges)' /tmp/h
LIMIT=100 e GET "/emby/Audio/$SONG/universal?UserId=$LISTENER&DeviceId=ci-listener&Container=mp3&MaxStreamingBitrate=128000"
grep -iE '^(content-type|content-length|accept-ranges)' /tmp/h
TOKEN_SAVE="$TOKEN"; TOKEN=
LIMIT=100 e GET "/emby/Audio/$SONG/stream?static=true&api_key=$TOKEN_SAVE"
LIMIT=100 e GET "/emby/Audio/$SONG/stream?static=true&ApiKey=$TOKEN_SAVE"
TOKEN="$TOKEN_SAVE"
curl -sS -o /dev/null -w 'ranged: %{http_code} %{size_download}\n' -H "X-Emby-Token: $TOKEN" -H 'Range: bytes=100-199' "$B/emby/Audio/$SONG/stream?static=true"
curl -sS -o /dev/null -w 'Authorization header only: %{http_code}\n' -H "Authorization: MediaBrowser Client=\"Encore\", Device=\"x\", DeviceId=\"ci-listener\", Version=\"0.1.0\", Token=\"$TOKEN\"" "$B/emby/Users/$LISTENER"
curl -sS -o /dev/null -w 'Authorization header, Emby scheme: %{http_code}\n' -H "Authorization: Emby Client=\"Encore\", Device=\"x\", DeviceId=\"ci-listener\", Version=\"0.1.0\", Token=\"$TOKEN\"" "$B/emby/Users/$LISTENER"
curl -sS -o /dev/null -w 'X-Emby-Authorization with Token: %{http_code}\n' -H "X-Emby-Authorization: MediaBrowser Client=\"Encore\", Device=\"x\", DeviceId=\"ci-listener\", Version=\"0.1.0\", Token=\"$TOKEN\"" "$B/emby/Users/$LISTENER"
curl -sS -o /dev/null -w 'no prefix: %{http_code}\n' -H "X-Emby-Token: $TOKEN" "$B/Users/$LISTENER"
curl -sS -o /dev/null -w 'no auth: %{http_code}\n' "$B/emby/Users/$LISTENER"
LIMIT=0 e GET "/emby/Items?UserId=$LISTENER&Recursive=true&IncludeItemTypes=MusicAlbum&Fields=DateCreated"
jq -c '.Items[] | {Id, Name, ImageTags, AlbumArtist, AlbumArtists, ProductionYear}' /tmp/b
ALBUM="$(jq -r '.Items[] | select(.Name=="First Light") | .Id' /tmp/b)"
TAG="$(jq -r '.Items[] | select(.Name=="First Light") | .ImageTags.Primary' /tmp/b)"
curl -sS -o /tmp/img -D /tmp/h -w 'png cover: %{http_code}\n' -H "X-Emby-Token: $TOKEN" "$B/emby/Items/$ALBUM/Images/Primary?maxWidth=64&maxHeight=64&format=Png&tag=$TAG"; grep -i content-type /tmp/h; head -c 8 /tmp/img | xxd
curl -sS -o /tmp/img -D /tmp/h -w 'fill cover: %{http_code}\n' -H "X-Emby-Token: $TOKEN" "$B/emby/Items/$ALBUM/Images/Primary?fillWidth=64&fillHeight=64&quality=90"; grep -i content-type /tmp/h; head -c 8 /tmp/img | xxd
curl -sS -o /tmp/img -w 'cover no auth: %{http_code}\n' "$B/emby/Items/$ALBUM/Images/Primary?maxWidth=64"
echo "################ favourites"
e POST "/emby/Users/$LISTENER/FavoriteItems/$SONG"
e POST "/emby/UserFavoriteItems/$SONG?userId=$LISTENER"
LIMIT=300 e GET "/emby/Items?UserId=$LISTENER&Recursive=true&IncludeItemTypes=Audio&Filters=IsFavorite"
e DELETE "/emby/Users/$LISTENER/FavoriteItems/$SONG"
e GET "/emby/Items?UserId=$LISTENER&Recursive=true&IncludeItemTypes=Audio&Filters=IsFavorite&Limit=0&EnableTotalRecordCount=true"
e POST "/emby/Users/$LISTENER/FavoriteItems/$SONG/Delete"
e POST "/emby/Users/$ADMIN/FavoriteItems/$SONG"
echo "################ played"
e POST "/emby/Users/$LISTENER/PlayedItems/$OTHER"
LIMIT=0 e GET "/emby/Items?UserId=$LISTENER&Recursive=true&IncludeItemTypes=Audio&Filters=IsPlayed&EnableUserData=true"
jq -c '.Items[] | {Id, Name, UserData}' /tmp/b
e POST "/emby/Sessions/Playing" "{\"ItemId\":\"$THIRD\",\"PositionTicks\":0}"
e POST "/emby/Sessions/Playing/Stopped" "{\"ItemId\":\"$THIRD\",\"PositionTicks\":190000000}"
LIMIT=0 e GET "/emby/Users/$LISTENER/Items/$THIRD"; jq -c '.UserData' /tmp/b
echo "################ playlists"
e POST /emby/Playlists "{\"Name\":\"Body list\",\"Ids\":[\"$SONG\"],\"UserId\":\"$LISTENER\",\"MediaType\":\"Audio\"}"
e POST "/emby/Playlists?Name=Query%20list&Ids=$SONG,$OTHER,$THIRD&MediaType=Audio&UserId=$LISTENER"
PL="$(jq -r .Id /tmp/b)"
LIMIT=0 e GET "/emby/Playlists/$PL/Items?UserId=$LISTENER"
jq -c '.Items[] | {Id, Name, PlaylistItemId}' /tmp/b
E2="$(jq -r '.Items[1].PlaylistItemId' /tmp/b)"
E3="$(jq -r '.Items[2].PlaylistItemId' /tmp/b)"
e POST "/emby/Playlists/$PL/Items/$E3/Move/0"
LIMIT=0 e GET "/emby/Playlists/$PL/Items?UserId=$LISTENER"; jq -c '[.Items[] | {Name, PlaylistItemId}]' /tmp/b
e POST "/emby/Playlists/$PL/Items?Ids=$SONG&UserId=$LISTENER"
LIMIT=0 e GET "/emby/Playlists/$PL/Items?UserId=$LISTENER"; jq -c '[.Items[] | {Name, PlaylistItemId}]' /tmp/b
e DELETE "/emby/Playlists/$PL/Items?EntryIds=$E2"
LIMIT=0 e GET "/emby/Playlists/$PL/Items?UserId=$LISTENER"; jq -c '[.Items[] | {Name, PlaylistItemId}]' /tmp/b
e POST "/emby/Playlists/$PL/Items/Delete?EntryIds=$E3"
LIMIT=0 e GET "/emby/Playlists/$PL/Items?UserId=$LISTENER"; jq -c '[.Items[] | {Name, PlaylistItemId}]' /tmp/b
e POST "/emby/Playlists/$PL" '{"Name":"Renamed"}'
LIMIT=0 e GET "/emby/Users/$LISTENER/Items/$PL"; jq -c '{Name, Type, CanDelete, Id}' /tmp/b
jq '.Name = "Renamed by item update"' /tmp/b > /tmp/upd
e POST "/emby/Items/$PL" "$(cat /tmp/upd)"
LIMIT=0 e GET "/emby/Users/$LISTENER/Items/$PL"; jq -c '{Name, Type, CanDelete, Id}' /tmp/b
LIMIT=3000 e GET "/emby/Items?UserId=$LISTENER&Recursive=true&IncludeItemTypes=Playlist&Fields=DateCreated,CanDelete"
LIMIT=3000 e GET "/emby/Users/$LISTENER/Items?Recursive=true&IncludeItemTypes=Playlist"
echo "################ delta"
e GET "/emby/Items?UserId=$LISTENER&Recursive=true&IncludeItemTypes=Audio&MinDateLastSaved=2030-01-01T00:00:00.0000000Z&EnableTotalRecordCount=true&Limit=1"
LIMIT=600 e GET "/emby/Items?UserId=$LISTENER&Recursive=true&IncludeItemTypes=Audio&MinDateLastSaved=2001-01-01T00:00:00.0000000Z&EnableTotalRecordCount=true&Limit=1&Fields=DateLastSaved"
LIMIT=600 e GET "/emby/Items/$SONG/Similar?UserId=$LISTENER&Limit=2"
echo "################ deletes"
e DELETE "/emby/Items/$PL"
e POST "/emby/Items/Delete?Ids=$PL"
LIMIT=600 e GET "/emby/Users/$LISTENER/Items?Recursive=true&IncludeItemTypes=Playlist"
e POST /emby/Sessions/Logout
e GET "/emby/Users/$LISTENER"
docker logs emby 2>&1 | grep -viE "api_key|apikey|token" | tail -n 60
