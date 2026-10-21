# 配置库维护与性能

保留的 `配置` 是用户原稿。`entries/*.json` 是每个条目的摘要元数据，同名 `.md` 是正文；`catalog.json` 是生成的搜索索引。修改条目后运行：

```sh
python3 maimai-note-desinger-skill/scripts/build_index.py
```

脚本只解析条目元数据并 stat 正文/参考文件，不读取参考 Simai 内容。写入采用临时文件与原子替换；内容未变不重写。`--check` 验证索引是否最新。

新增一个 `entries/your-pattern.json`，格式如下（路径相对技能目录）：

```json
{
  "id": "your-pattern",
  "name": "配置名称",
  "aliases": ["别名"],
  "tags": ["交互", "TAP"],
  "summary": "简短定义，供检索；详细写法放正文。",
  "completeness": "complete",
  "source": "用户资料及出处",
  "body": "entries/your-pattern.md",
  "references": [
    {"id":"example-a","label":"来源、BPM 与相关小节","kind":"simai","path":"examples/example-a.simai"}
  ]
}
```

不带示例时 references 为 []。completeness 使用 complete、partial 或 name_only；这表示资料完整度，不表示定义已经过权威或实机验证。稳定 ID 使用小写英文、数字与连字符；引用 ID 不能为 entry。正文可包含定义、节奏、键位、用手、适用场景、注意点及来源，但不要求每条具备所有字段。

参考 Simai 是外置文件，可以很大；搜索不会读取它，读取时每次最多 8192 字节。为降低读取成本，建议按有意义的片段拆分参考谱，label 写清来源与小节。标签、别名和 summary 决定搜索命中；正文和 Simai 不参与全文搜索。当前为大小写不敏感的子串检索，空格分词取交集，精确名称/别名优先；没有向量或语义搜索。

索引最多 32 MiB / 50,000 条，每页最多 20 条；响应仍受通用 64 KiB 上限约束。首次后台加载索引后缓存摘要，后续仅检查索引元数据；索引变化时重新加载。每个编辑器使用一个有界队列和独立 worker，检索不会占用编辑器帧线程。热查询按条目数线性扫描摘要，排序匹配结果；无需扫描正文或参考文件目录。

读取校验文件大小；首次正文页返回 file_revision（大小与修改时间），后续页必须带上它，文件变化则要求重新读取。正文同字节长度修改无需重建索引即可读到；摘要、路径、参考列表或文件大小变化必须重建。跨搜索页使用 revision，避免混用两个版本索引。

从项目目录或其子目录运行时自动发现本库；也可在启动编辑器前设置进程环境变量 `LDX_NOTE_DESIGN_LIBRARY` 为技能目录的绝对路径，两种客户端共用该目录。此变量不走模型连接的 `.env` 配置。发布独立二进制时随程序分发整个技能目录并设置该变量；构建时仅将简短技能入口嵌入 Agent，不嵌入用户参考谱。
