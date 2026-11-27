#!/bin/sh
# Regenerates the local-file fixtures (Phase 12 stage 4): one-second sine tones with
# tags, made here with ffmpeg, so they carry no one's music and no licence terms.
set -e
cd "$(dirname "$0")"
ffmpeg -v error -y -f lavfi -i "color=c=0x5b3fd0:s=8x8:d=1" -frames:v 1 cover.png
tone="sine=frequency=440:duration=1"
ffmpeg -v error -y -f lavfi -i "$tone" -i cover.png -map 0:a -map 1:v -ac 1 -ar 22050 \
  -c:a libmp3lame -b:a 32k -c:v png -disposition:v attached_pic \
  -metadata:s:v title="Front cover" -metadata:s:v comment="Cover (front)" -id3v2_version 3 \
  -metadata title="Morning Tone" -metadata artist="Encore Fixtures; Second Voice" \
  -metadata album="Test Tones" -metadata album_artist="Encore Fixtures" \
  -metadata track="3/12" -metadata disc="1/1" -metadata date="2019" -metadata genre="Ambient" \
  tagged.mp3
ffmpeg -v error -y -f lavfi -i "$tone" -ac 1 -ar 22050 -c:a libmp3lame -b:a 32k \
  -map_metadata -1 -id3v2_version 0 -write_xing 0 untagged.mp3
ffmpeg -v error -y -f lavfi -i "$tone" -i cover.png -map 0:a -map 1:v -ac 1 -ar 8000 \
  -c:a flac -c:v png -disposition:v attached_pic -metadata:s:v comment="Cover (front)" \
  -metadata TITLE="Evening Tone" -metadata ARTIST="Encore Fixtures" -metadata ALBUM="Test Tones" \
  -metadata TRACKNUMBER=4 -metadata DATE=2020-03-01 \
  -metadata MUSICBRAINZ_TRACKID=00000000-0000-4000-8000-000000000004 \
  -metadata LYRICS="$(printf '[00:00.50]First line\n[00:00.90]Second line')" \
  tagged.flac
ffmpeg -v error -y -f lavfi -i "$tone" -ac 1 -ar 22050 -c:a aac -b:a 32k \
  -metadata title="Noon Tone" -metadata artist="Encore Fixtures" -metadata album="Test Tones" \
  -metadata track=5 tagged.m4a
ffmpeg -v error -y -f lavfi -i "$tone" -ac 1 -ar 22050 -c:a libvorbis -q:a 0 \
  -metadata TITLE="Night Tone" -metadata ARTIST="Encore Fixtures" -metadata GENRE="Ambient; Drone" \
  tagged.ogg
rm cover.png
