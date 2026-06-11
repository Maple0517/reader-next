# 本地 TXT 小说服务端书架导入设计

## 目标

允许用户上传本地 `.txt` 小说到服务端书架。上传后小说像普通书籍一样出现在书架里，可打开阅读、查看目录、保存阅读进度，并能在同一服务端账号下跨设备读取。

## 推荐方案

采用“服务端本地书籍”方案：

1. 前端提供上传 `.txt` 文件入口。
2. 后端接收文件，校验扩展名和内容大小。
3. 后端把原始文本保存到 `storage/local_books/`。
4. 后端解析章节并保存章节索引。
5. 后端创建或更新一个书架 `Book`，使用本地协议标识：`local-txt:<hash>`。
6. 现有阅读器通过新增本地 TXT 分支读取目录和章节内容。

## 数据模型

本地 TXT 书籍复用现有 `Book` 和 `BookChapter`：

- `Book.bookUrl`: `local-txt:<hash>`
- `Book.origin`: `local-txt`
- `Book.originName`: `本地 TXT`
- `Book.canUpdate`: `false`
- `Book.totalChapterNum`: 解析出的章节数量
- `Book.latestChapterTitle`: 最后一章标题
- `Book.name`: 文件名去扩展名，后续可支持用户编辑
- `Book.author`: 默认 `本地导入`

本地文件存储建议：

```text
storage/local_books/<hash>/book.txt
storage/local_books/<hash>/chapters.json
```

`chapters.json` 记录每章标题、章节序号、正文起止字节或字符区间。实现优先选择字符区间，简单稳定。

## 章节解析规则

章节标题按常见中文小说格式识别：

- `第十二章 标题`
- `第12章 标题`
- `第十二回 标题`
- `第12节 标题`
- `卷一 标题`

规则要求：

- 标题必须出现在单独一行。
- 标题行长度限制在 80 个字符以内，避免误切正文。
- 识别不到章节时，把整本书作为 `正文` 一章。
- 每章内容保留原文本换行。

## 后端接口

新增接口：

```http
POST /reader3/uploadTxtBook
Content-Type: multipart/form-data
field: file
```

返回上传后创建的 `Book`。

后端阅读分支：

- 获取目录时，如果 `bookSourceUrl/origin` 是 `local-txt`，读取 `chapters.json`。
- 获取正文时，如果章节 URL 是 `local-txt:<hash>#<index>`，读取 `book.txt` 对应区间。

错误处理：

- 非 `.txt`：返回 400。
- 空文件：返回 400。
- 文件过大：返回 400，限制由实现中的常量控制，默认 50MB。
- 解析或保存失败：返回现有统一错误格式。

## 前端交互

在书架/阅读器入口提供“上传 TXT”按钮：

1. 选择 `.txt` 文件。
2. 调用 `/reader3/uploadTxtBook`。
3. 上传成功后刷新书架。
4. 默认打开新导入书籍的第一章。

显示约束：

- 上传中按钮禁用并显示“上传中”。
- 上传失败显示现有错误提示风格。
- 本地 TXT 书籍没有封面时继续使用现有占位封面。

## 测试策略

后端优先测试纯解析逻辑：

- 能切分 `第1章` / `第二章`。
- 识别不到章节时生成单章。
- 章节 URL 使用 `local-txt:<hash>#<index>`。

接口测试聚焦：

- 上传 TXT 返回 Book。
- 非 TXT 被拒绝。

前端测试或构建聚焦：

- API 封装存在。
- 类型通过。
- `npm run build` 通过。

## 范围外

本次不做：

- EPUB/PDF/Word 导入。
- 封面提取。
- TXT 编码手动选择 UI。
- 在线更新本地 TXT。
- 删除书籍时自动物理删除本地文件。
