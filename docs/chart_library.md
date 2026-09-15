# Chart Library

The player does not bundle any charts. On launch it reads the **eligible chart
list** from a single chart library directory on disk.

## Location

| Order | Source | Notes |
|---:|---|---|
| 1 | `$MAI2_SONGS_DIR` | Explicit override, useful for tests and alternate libraries. |
| 2 | `$HOME/.maichart` | Default library root. |

The directory is created when missing, so a first launch always has a place to
put charts:

```text
~/.maichart/
  my-song/
    maidata.txt
    track.mp3
    bg.jpg
```

`track.mp3` (also `track.wav`, `music.mp3`, or `music.wav`) and a cover image are
optional. `bg.jpg`, `bg.png`, `bg.jpeg`, `cover.*`, and `jacket.*` are all
recognized as covers.

## Eligible charts

A directory is eligible when it directly contains a `maidata.txt`. Directories
are searched recursively up to three levels below the library root, so both flat
and grouped libraries work:

```text
~/.maichart/
  Original/
    my-song/
      maidata.txt      # eligible, two levels below the root
```

Directories whose name starts with `.` are skipped, so tool caches such as
`.MajdataPlay` are never scanned. Group folders that contain no `maidata.txt`
themselves contribute their children instead. An empty library is a normal
state: the song list shows the library path and the player stays launchable.

## Importing

`导入歌曲到曲库` copies a picked chart, its audio, and its cover into the library
root, so imported songs are scanned like any other library entry.
