# Player Song Library

将每首歌放到 `songs/` 下的一个独立子目录中。播放器只会读取包含
`maidata.txt` 的子目录。

```text
songs/
  my-song/
    maidata.txt
    track.mp3
    bg.jpg
```

`track.mp3`（也可为 `track.wav`、`music.mp3` 或 `music.wav`）和封面图为可选文件。
运行时可用 `MAI2_SONGS_DIR` 指向另一个曲库目录。
