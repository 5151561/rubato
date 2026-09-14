# Rubato 数据存储与 Legado 导入现代化规划

> 日期：2026-08-31  
> 范围：Legado 数据导入、Rubato 全量数据分类与持久化、备份/恢复、缓存、同步预留、性能与数据安全。  
> 性质：设计与迁移计划；本轮不改生产数据结构和运行代码。
>
> 2026-09-01 复核修订：补入 `(name, author)` 唯一索引与 `REPLACE` 组成的删书链路、
> `caches` 前缀查询的真实 query plan、项目差分门禁，以及 SecretStore/备份缺项等项目约束；
> 同时把连接池、writer actor、SQLite 依赖升级和性能阈值改成“先取基线再决策”。
>
> 2026-09-01 第二轮复核：补入 `books.origin` 悬空引用、与裁判实体的有意分歧需显式固定、
> `content:` 拼串键的分隔符歧义；差分门禁按“是否触及裁判对照路径”分档；
> 阶段 0 的身份语义拆成“不等产品决策的止损”与“可延后的合并 UI 决策”两半。

## 1. 结论

Rubato 当前的存储实现已经能支撑最小产品闭环，但还不能安全承接“完整 Legado 数据导入”和长期数据演进：

- 当前只支持粘贴单个或数组形式的 `bookSource` JSON，不支持 Legado 备份 ZIP、书架、设置、Cookie、运行时书源变量等分类导入。
- 当前 `books(name, author)` 唯一索引与 `INSERT OR REPLACE` 组合，会在加入“另一来源的同名同作者书”时静默删除旧书；Rubato 又没有照搬上游 chapters 外键，旧目录和正文缓存随即成为孤儿。这是开始任何导入工作前必须解除的数据正确性阻断。
- `books / chapters / caches / book_sources / cookies` 共用一个 SQLite 文件是合理起点；当前最明确的性能问题不是“单文件”本身，而是每次创建 JsEnv 都在全局 `Stores` 锁下全扫同时包含所有正文的 `caches`。应先修查询并测量，再决定是否需要连接池或 writer actor。
- schema 目前依赖 `CREATE TABLE IF NOT EXISTS` 和表形状探测，没有正式 schema 版本、顺序迁移、升级前校验和发布回滚约束。
- `INSERT OR REPLACE` 会先删除再插入；`save_book` 又只写部分列。以后导入 Legado 的自定义封面、阅读配置、同步时间等字段后，普通保存可能把未写列重置为默认值。
- 当前 bundled SQLite 是 3.46.0，同时应用使用 WAL 和多个连接。SQLite 官方的 WAL-reset 公告把该版本列在受影响范围，但 rusqlite 0.32 到含修复 bundled SQLite 的版本存在破坏性升级成本；先验证各平台发布产物、并发条件和可行升级台阶，再安排实际升级。扩大数据库并发之前必须完成这项决策。

目标不是简单换一个数据库库，而是建立以下能力：

1. SQLite 是结构化主数据的唯一事实源；UI 和网络不能绕过 Repository 直接拥有数据。
2. 主数据、敏感状态、可重建缓存和临时导入物有不同的保存、备份、加密与清理策略。
3. Legado 导入可以预演、取消、续跑、审计、回滚，不因单条坏数据破坏现有数据。
4. 数据 schema、备份格式和同步协议各自版本化，不能把内部数据库文件当长期交换格式。
5. 性能优化由现有差分/探针基础设施和新增 store benchmark 驱动，先解决已被 query plan 证明的扫描，再评估锁粒度、批量事务、热路径写放大、无界缓存与大对象内存峰值。

## 2. 当前基线与证据

### 2.1 已有的正确基础

- SQLite 使用 rusqlite bundled 构建，Android 与桌面行为一致：`rust/crates/store/Cargo.toml:9`。
- 主库已有配置 WAL、外键检查和 busy timeout 的意图：`rust/crates/store/src/db.rs:193`；但前两项当前被同一个 `.ok()` 包住，是否真正生效必须按 §2.2 修复并读回验证。
- 书源批量保存、目录整体替换和 JS 缓存差量回写均使用事务：`rust/crates/store/src/source_store.rs:24`、`rust/crates/store/src/book_store.rs:122`、`rust/crates/store/src/book_store.rs:259`。
- 书源保留原始 JSON，并只投影启用和排序字段，能避免当前实体模型未识别字段在往返后丢失：`rust/crates/store/src/source_store.rs:1`。
- 旧版 29 列书源表迁移到 JSON 表时使用单事务，已有迁移不丢数据的意识和测试：`rust/crates/store/src/db.rs:102`、`rust/crates/store/src/db.rs:125`。
- 阅读进度只更新热路径需要的列，而不是整行覆盖：`rust/crates/store/src/book_store.rs:116`。
- JS CacheManager 的并发写回已经从整段替换改成差量提交，避免不同搜索 worker 相互覆盖：`rust/crates/engine/src/lib.rs:225`。

这些模式应该保留并上升为统一存储契约，而不是推倒重来。

### 2.2 当前阻塞点

| 优先级 | 分类 / 证据 | 发现与失败模式 | 最小目标与验证 |
|---|---|---|---|
| **P0** | 现实风险，E2/高 | `books` 同时以 `bookUrl` 为主键、以 `(name, author)` 为唯一键，而 `save_book` 使用 `INSERT OR REPLACE`。写入不同 `bookUrl` 的同名同作者书时，SQLite 会删除造成唯一冲突的旧行再插入新行；Rubato 的 `chapters` 没有上游 `ON DELETE CASCADE`，旧 chapters 与 `content:` 缓存成为孤儿。证据：`db.rs:18-75`、`book_store.rs:48-88`；上游 FK：`BookChapter.kt:29-41`。 | 先确定身份语义。按本文“不同 URL 不自动合并”的产品选择，推荐保留 `bookUrl` 唯一身份，将 `(name, author)` 改为普通索引，仅用于重复提示；再用 PK-targeted UPSERT。加入“同名同作者、不同 URL”端到端回归，以及历史 orphan 诊断。 |
| **P1** | 现实风险，E1/高 | `list_cache_prefix` 的 `LIKE ? ESCAPE '\\'` 在当前 BINARY 主键和默认大小写不敏感 LIKE 组合下不能使用主键前缀范围；实测 query plan 为 `SCAN caches`。它扫描的是包含全部正文的最大表，并在 `self.stores()` 全局锁内执行；6 个 worker × 每源一次，把扫描成本和锁等待相乘。证据：`book_store.rs:240-254`、`engine/lib.rs:207-212`。 | 立即改为可索引的半开区间（`key >= 'js:' AND key < 'js;'`，通用实现需安全计算 prefix upper bound）或经 query plan 验证的 GLOB；增加大 caches fixture，门禁必须断言 `SEARCH ... USING INDEX`，并测搜索墙钟与锁等待。 |
| **P1** | 现实风险，E2/高 | `save_book` 只写 23 个字段并使用 `REPLACE`。即便解除二级唯一索引，导入后再刷新/保存仍会把 `customTag/customCoverUrl/readConfig/syncTime/...` 重置为默认值。证据：`book_store.rs:48-88`、`db.rs:18-50`。 | 明确字段所有权后改成只更新对应字段的 UPSERT/UPDATE；用每个未写字段的保留测试锁住。 |
| **P1 待验证** | 现实风险，E2/中 | bundled header 为 SQLite 3.46.0；SQLite 官方 WAL-reset 公告称 3.7.0～3.51.2 受低概率竞态影响，条件是 WAL、两个以上连接、跨线程/进程并发写与 checkpoint。Rubato 有三条连接和独立的 store/cookie 写路径，条件具备可达性，但尚未验证各平台发布产物与实际 checkpoint 交错。修复版 rusqlite 0.39 bundled SQLite 3.51.3，而 0.38 已有 breaking changes。 | 先在 Android/macOS/Windows release 产物记录 `sqlite_version()`/compile options，做兼容性 spike（升级 rusqlite、受控 backport/fork、官方 backport SQLite 三选一）和并发压力测试；依据官方公告完成修复，但不让未评估的依赖大升级挡在前面三个确定性修复之前。 |
| **P1** | 能力缺口，E2/高 | 没有正式 schema 版本与完整迁移链；新增/改列只能继续堆表形状特判，无法可靠覆盖多个已发布版本。证据：`db.rs:91-145`。 | 建 v1 fixture、顺序迁移和迁移后数据不变量；旧 29 列 source 库必须保留覆盖。 |
| **P2** | 现实风险，E2/高 | `db::tune` 把 `journal_mode=WAL` 与 `foreign_keys=ON` 放进同一 `execute_batch` 后整体 `.ok()`；任何错误都会被吞。未来添加 FK 后，某连接上 FK 未启用会使 cascade/约束静默失效。证据：`db.rs:193-199`。 | 两条 PRAGMA 分开执行并读回验证；`foreign_keys=1` 是开库硬条件，WAL 可按明确能力策略降级。每条新连接都验证。 |
| **P2** | 现实风险，E2/高 | `delete_source` 是裸 DELETE，`source_of` 查不到即返回通用 `NotFound`。删除书源后，`origin` 指向它的书仍在书架上，但刷新目录和取正文全部失败，用户只看到“打不开、也不知道为什么”。删源是日常操作，不是迁移期边缘情况。证据：`source_store.rs:80-83`、`engine/lib.rs:648-653`。 | **不加 FK**（换源、源暂时不可用、导入时源尚未落库都是合法中间状态，FK 会把它们变成写入失败）。改为显式引用检查：删源前统计并提示受影响书数；`source_of` 失败返回可区分的“书源已删除”错误而非通用 `NotFound`，UI 给换源入口。孤儿盘点与 P0 同批进行。 |
| **P2** | 现实风险，E2/高 | 加书架、刷新目录分别提交 book 和 chapters；第二步失败或进程中断时，`totalChapterNum`、书籍元数据与目录不一致。证据：`engine/lib.rs:575-577`、`:604-606`。 | 收成一个 Repository 事务并做故障注入。 |
| **P2** | 现实风险，E2/中 | Cookie 使用独立连接，写失败只打印日志：当次内存登录有效但重启后丢失，UI 无法告知或重试。证据：`cookie_store.rs:52-64`。 | 返回 typed error 或进入可观测重试队列。 |
| **P2** | 能力缺口，E2/高 | JS 持久状态、正文缓存和设备 ID 共用 `caches`，敏感/不可丢状态与可删除缓存无法分别加密、备份、限额和清理；正文又无容量/访问时间/淘汰。证据：`book_store.rs:193-217`、`engine/lib.rs:99-120`。 | 先修前缀查询；后续按数据性质拆域并建立容量策略。 |
| **P2** | 现实风险，E2/中 | 翻页后立即发起异步进度写，Dart 不等待也不合并；快速翻页会堆积小事务，页面退出时缺少显式 flush/顺序保证。证据：`reader_page.dart:139-151`。 | 基于实测决定 debounce/coalescing，换章、退后台和退出页必须 flush。 |
| **P2** | 能力缺口，E2/高 | 书源导入坏行静默跳过，只返回成功数，用户无法知道失败项、原因和覆盖情况。证据：`source_store.rs:34-40`、`sources_sheet.dart:71-77`。 | 统一 ImportReport，错误与跳过可复算。 |
| **P3 待测** | 改进机会，E2/中 | `Stores { books, sources }` 共用全局 Mutex；但网络与解析在锁外，当前最重的临界区来自上述全表扫。直接上读池/writer actor 的净收益未知。 | 修复范围查询并采集锁等待/数据库耗时；只有剩余争用超过产品预算时才引入池或 actor。 |

严重度描述的是数据与用户影响，不代表实施工期。E1 表示已在等价运行环境/查询计划中复现，E2 表示由仓库源码、schema 与官方语义直接推出。P0 的 SQL 语义由 SQLite 官方 REPLACE 文档直接支持；P1 前缀扫描已用 `EXPLAIN QUERY PLAN` 在本仓库等价 schema 上复现。SQLite 版本项是高影响、低概率且有依赖迁移成本的“待验证高风险”，不再把版本升级本身当作无需评估的第一步。

## 3. 数据分类与保存策略

不要继续按“所有能序列化的东西都塞 SQLite/通用 KV”分类。建议按数据能否重建、敏感度和一致性要求分为五类：

| 类别 | 数据 | 事实源与介质 | 备份 | 清理/加密 |
|---|---|---|---|---|
| A. 核心主数据 | 书架、目录、阅读进度、书源定义、分组、书签/高亮、阅读记录，以及用户自定义/明确持久化的封面与背景 | `rubato.db` + durable media 目录；数据库保存内容 hash 和引用 | 默认包含 | 事务/引用保护；不可按空间策略删除 |
| B. 用户设置 | 阅读样式、主题、书架布局、缓存上限、同步策略 | `rubato.db` 的强类型设置表；Rust 暴露 typed API | 默认包含 | schema 版本化；设备专属设置可排除 |
| C. 敏感持久状态 | Cookie、登录信息、书源运行时变量、WebDAV token/密码、设备密钥 | 主库密文列 + 平台安全存储中的主密钥；云凭据优先只进 Keychain/Keystore/Credential Manager | 默认不包含，用户显式选择并设置备份口令 | AEAD 字段加密；日志永不输出值 |
| D. 可重建缓存 | 章节正文、网络可重新抓取的封面、HTTP 响应、派生排版 | 独立 `rubato-cache.db` + cache 目录 | 默认不包含 | LRU/TTL/总容量；可一键清空；无需迁移保证 |
| E. 临时/任务状态 | 搜索会话、WebView session、导入解包、暂存行、后台任务进度 | 内存、cache 目录、`import_sessions` 暂存表 | 不包含 | 超时/启动清理；任务可恢复或明确失败 |

关键决定：

- 不引入 Android DataStore 作为跨平台主设置源。Rubato 的业务事实源在 Rust，Android 与桌面应共用同一套 typed settings Repository；平台安全存储只负责密钥和凭据。
- 不对整个主库默认使用 SQLCipher。先采用“敏感字段 AEAD + 平台主密钥”，保持 SQLite 调试、迁移、备份和桌面构建简单。若未来威胁模型要求整库加密，再单独评估 SQLCipher 的跨平台、升级和性能成本。
- SecretStore 是可注入能力，不是所有运行环境的隐式前提：Android/macOS/Windows 产品构建使用平台实现；`:memory:`、差分跑子和单测注入确定性的 `TestSecretStore`；只操作内存库/临时数据且无平台能力的命令行探针可使用仅进程生命周期有效的 `EphemeralSecretStore`。持久文件库若没有持久 SecretStore，核心非敏感功能仍可打开，但敏感读写返回 `SecretStoreUnavailable`，绝不能用临时密钥写入下次无法解密的密文，也不静默回落到磁盘明文。
- 正文缓存移出主库。目录属于离线阅读状态，应留在主库；正文与图片可重新抓取，进入可限额缓存域。
- `customCoverUrl/persistedCoverUrl` 指向的用户自定义或明确持久化媒体属于 A 类；只有能从网络重新获取的普通封面缓存属于 D 类。两者使用不同目录/引用类型，清缓存不得触及 A 类文件。
- `androidId` 不再伪装成 cache 项。它是安装身份/兼容标识，应迁入明确的 `installation_state`，必要时由平台安全存储保护。

## 4. 目标数据层

### 4.1 边界

```text
Flutter 页面 / Controller
        │ 只调用粗粒度命令、订阅不可变快照
        ▼
FRB API（DTO、任务句柄、进度流；不暴露 SQL/Connection）
        ▼
Rust Repository / Use Case
  ├─ BookshelfRepository
  ├─ SourceRepository
  ├─ SettingsRepository
  ├─ CredentialRepository
  ├─ CacheRepository
  └─ ImportRepository
        ▼
Storage runtime
  ├─ 主库：短事务 + 可观测的串行访问；读池/writer actor 由基线决定
  ├─ 缓存库：独立连接与独立限额
  ├─ 可注入 SecretStore（平台 / 测试 / 进程临时实现）
  └─ 文件/ZIP 流
```

Repository 的意义是确定每类数据的唯一所有者和冲突策略，不是为了增加空壳类。初期可以继续留在 `store` crate 内按模块组织；只有 Legado 解析器能独立测试、依赖明显膨胀后，再拆 `legado-import` crate。

> **Dart 侧不镜像这一层。** 这张图里的六个 Repository 都在 Rust，`rust.bookshelf()`
> 本身就是仓储调用；Flutter 那边再建一套同名的转发类正是上一句要拒绝的空壳类。
> Dart 侧只有一份**内存副本 + 失效通知**（`app/lib/data/live.dart`），不持有事实源、
> 不做写入编排、不落盘。理由与它换掉的是什么，见 `reader-layout-plan.md` ADR-003。

### 4.2 连接与并发模型

首轮不预设“连接池 + writer actor”一定优于现状。先建立可比较的基线：

1. 把 `list_cache_prefix` 改为索引范围查询，先去掉当前最大、且已被 query plan 证明的临界区成本。
2. 在 `Stores` 锁入口记录等待时间和持锁时间，在 SQLite 操作记录 SQL 类别、耗时、`SQLITE_BUSY` 与 WAL/checkpoint 状态。
3. 保持网络抓取、JSON 解析、ZIP 解压、压缩和图片处理在锁与事务外；事务只提交已校验的结果。
4. 拆开并验证每条连接的 PRAGMA；每个读事务尽快结束，避免长读拖住 checkpoint。
5. 在相同 profile 构建、相同 fixture 和相同机器上比较修复前后的搜索、阅读与导入 workload。

只有范围查询修复后，锁等待仍显著占据端到端延迟或出现可复现的读阻塞，才引入一个独占写连接的 writer actor；只有并行读取被证明受单连接限制，才增加小型只读连接池。池大小由 benchmark 决定，不在设计文档里预先写死。扩大连接并发前必须完成 SQLite WAL-reset 修复路径。

若实测需要 actor/读池，其约束是：所有写命令串行但不持有跨网络/解析工作的锁；Cookie、source runtime、书架不各自制造无治理的 writer/checkpointer；读连接在创建时逐条完成 PRAGMA 验证。

SQLite 基线配置必须通过代码读取并验证结果，不能吞错：

- `journal_mode=WAL`；不支持时明确降级并记录能力状态。
- `foreign_keys=ON`；连接池每条连接都设置并验证。
- `busy_timeout` 保留，但 `SQLITE_BUSY` 是可观测错误，不把十秒等待当正常流控。
- `synchronous` 先保持 SQLite 默认/保守策略，基准后再决定是否用 `NORMAL`；不以牺牲掉电耐久换未经测量的速度。
- checkpoint 先使用默认自动策略；记录 WAL 大小和慢提交后，再决定是否在空闲期执行 `PASSIVE`。不要定时强行 `TRUNCATE` 阻塞读者。
- 应用空闲或完成大批导入后运行 `PRAGMA optimize`；不在每次启动执行全库 `ANALYZE/VACUUM`。

### 4.3 schema 与迁移

引入 `schema_migrations`（或严格使用 `PRAGMA user_version`）并遵守：

1. 每个发布版本只执行已编号、不可跳过的迁移。
2. 迁移在单写者模式运行；开始前检查可用空间并生成可恢复快照。
3. 涉及 FK 的迁移完成后执行 `foreign_key_check`；高风险版本升级后执行 `quick_check`，完整 `integrity_check` 放在诊断/维护入口，避免每次普通启动全库检查。
4. 每个已发布 schema 都保存 fixture，CI 从每个 fixture 迁到当前版本并验证行数、关键字段和约束。
5. 使用 expand/contract：先加新表/列和双读适配，至少跨一个发布窗口后再删旧列；禁止隐式破坏性迁移。
6. 普通实体保存使用字段所有权明确的 `INSERT ... ON CONFLICT DO UPDATE SET ...` 或 `UPDATE`；不再使用会删除旧行的 `REPLACE`。SQLite 3.35+ 可以写多个 `ON CONFLICT` 子句，但语法能力不能代替实体身份决策。
7. 把“保存书 + 替换目录 + 更新章节数”收成一个 Repository 事务。
8. 任何约束收紧前先诊断并修复历史不一致，迁移后再启用约束和执行校验。

首轮建议 schema：

- `books`：第一项迁移先采用本文推荐身份语义——`bookUrl` 是唯一身份；删除 `index_books_name_author` 唯一约束，并以同名普通索引支持潜在重复提示。保留 Legado 兼容字段，新增 `row_version/created_at/updated_at`；明确哪些字段是用户自定义、哪些来自书源刷新。若产品拒绝这一身份语义，必须先定义合并规则，不能开始书架导入。此处与裁判 Room 的 `Book` 实体索引（`Book.kt:37`）**有意不一致**：Rubato 不做同名同作者的隐式合并。差分套覆盖解析与流水线、**不覆盖落库层**，没有任何自动化门禁能拦住后来的人以“对齐裁判”为由把唯一索引加回来 —— 所以这条分歧必须同时写在本文和 `db.rs` 的建表注释里，两处都要写明原因。
- `chapters`：先在迁移快照/报告中保存所有不存在 parent book 的 `bookUrl`、目录数量和可恢复线索，再由明确策略清理或恢复，不能无报告地删；之后通过重建表增加真实 `FOREIGN KEY(bookUrl) REFERENCES books(bookUrl) ON DELETE CASCADE`。开库后验证 `foreign_keys=1`，迁移后执行 `foreign_key_check`；删除书籍时另由 CacheRepository 清理对应 D 类 `content:` 缓存，不能误以为 FK 会跨表/跨库代劳。
- `books.origin` → `book_sources.bookSourceUrl`：**不建 FK**。换源、书源暂时不可用、导入顺序导致源尚未落库，都是合法的中间状态，用 FK 会把它们变成写入失败。改为显式引用检查 —— 删源前统计并提示受影响书数，`source_of` 失败返回可区分的“书源已删除”错误并给换源入口。阶段 0 的孤儿盘点同时覆盖 `chapters` 无 parent book 和这条悬空 `origin` 两类。
- `book_sources`：拆成原始定义与本地覆盖。`source_documents(url, raw_json, source_updated_at, hash)` 保存兼容文档；`source_preferences(url, enabled, custom_order, ...)` 保存本地开关和排序。更新源定义不覆盖本地偏好。迁移与往返必须继续经过 `BookSource::from_value` 的同一语义路径，并以现有 source 差分套和全量差分套作为硬门禁。
- `source_runtime`：按 `(source_url, key)` 存持久运行状态、deadline、敏感标记和版本；不再扫描全局 `js:` KV。
- `cookies`：规范化 domain/key，密文保存，写失败返回业务错误。
- `app_settings(namespace, key, type, value, schema_version, updated_at)`：只由 typed Repository 访问。
- `import_sessions/import_items`：导入预演、恢复、报告和幂等键。
- `change_log`：暂不启用云同步写放大；先保留实体版本字段，真正做同步时再按协议落地 outbox/tombstone。

## 5. Legado 数据导入

### 5.1 兼容边界

Rubato 应兼容 Legado 的“逻辑备份格式”，不直接打开或替换 `legado.db`：

- 上游当前 Room schema 已到 104，包含二十余类实体；Rubato 直接依赖其数据库文件会被表、类型转换器、Room 迁移和版本差异锁死。
- Legado 官方备份本身已经以 JSON/配置文件/媒体目录组成 ZIP，是更稳定、可验证、可选择的数据交换边界。
- 原始数据库导入可在以后作为单独的专家模式工具，通过只读打开 + schema fingerprint + 逻辑转换实现；不能进入首版产品路径。

首版接受三类输入：

1. `bookSource` 单对象/数组 JSON：保留现有入口，但改为统一 ImportPipeline。
2. Legado 备份 ZIP：识别 `bookshelf.json/bookSource.json/...`。
3. 已解包目录：仅用于测试和桌面诊断，不在移动端普通 UI 暴露。

Rubato 自有备份必须使用独立 manifest 和格式版本，不能伪装成 Legado ZIP。另提供“导出为 Legado 兼容数据”的 best-effort 功能。

### 5.2 上游备份内容与首版支持矩阵

仓库内冻结的 Legado 基准会输出以下主要条目：`judge/engine/src/main/java/io/legado/app/help/storage/Backup.kt:52`。

| Legado 条目 | 首版策略 | 说明 |
|---|---|---|
| `bookSource.json` | 完整导入 | 原始 JSON 保真；规则定义更新与本地启用/排序分离 |
| `bookshelf.json` | 导入网络书；本地书预演后默认跳过 | 保留阅读进度、自定义信息；目录不在上游备份中，导入后按需刷新 |
| `bookGroup.json` | 保存并映射 | 即使首版 UI 未完整支持，也进入主库，避免书架分组信息永久丢失 |
| `bookmark.json` | 保存，UI 可后续启用 | 先定义稳定实体与关联策略 |
| `highlight.json/highlightRule.json` | 第二批 | 需要先确定 Rubato 正文位置模型；不可直接假设字符位置始终等价。**2026-09-02：位置模型已定**（reader-layout-plan M4：章内 UTF-16 下标，与 `durChapterPos` 同一把尺，原点含章节标题）。本地 `highlights` 表已按上游 `BookHighlight.kt` 逐列建好并写 `layoutTitleLength`，样式存上游那套 JSON 且保留认不出的字段——导入这一步现在只差读 ZIP 的那一段。`highlightRule` 仍未定 |
| `cookies.json` | 可选、默认关闭 | 需要用户输入 Legado 备份口令；解密后立即转为 Rubato 密文，明文不落盘 |
| `runtimeSourceCache.json` | 可选、默认关闭 | 只允许白名单前缀；映射为 `(source_url,key)`，不直接拼入通用 KV |
| `config.xml`、阅读/主题配置 JSON | allowlist 映射 | 只导入 Rubato 已定义且语义一致的设置；路径、线程数、Android 专属项不照搬 |
| `readRecord.json/searchHistory.json` | 第二批 | 先保留导入报告；实现对应产品能力后再进主库 |
| `replaceRule/txtTocRule/httpTTS/...` | 按 Rubato 功能逐项开放 | 不支持的条目不静默宣称成功 |
| `rss*`、服务器、直链上传 | 暂不导入 | 当前产品没有等价能力 |
| `covers/`、`bg/` | 可选持久媒体导入 | 用户自定义/明确持久化的封面与背景进入 A 类 durable media，并进入 Rubato 自有备份；只有可重新抓取的网络封面副本进入 D 类缓存。校验 MIME、尺寸、总空间；使用内容哈希重命名，不信任 ZIP 文件名 |

对暂不支持的数据，导入报告必须列出“发现但未应用”。用户可选择保留原始备份文件，Rubato 不默认复制一份可能含敏感信息的 ZIP。

Legado 的备份类别由用户勾选，`selectedBackupFileNames(isEnabled)` 返回的是本次选择，不是所有 ZIP 都必须具备的清单。因此 ZIP 中缺少 `bookshelf.json`、`cookies.json` 等条目时，ImportPlan 记录 `not_present`，不报“损坏”也不计入失败；只有条目存在但 JSON/密文/校验无法读取时才是 `present_invalid`。Legado 格式没有 Rubato v1 manifest 那样的声明清单时，Rubato 不能把“用户未选择”和“ZIP 被手工删项”进一步区分；报告应诚实标为“备份中未包含”。

### 5.3 导入流水线

```text
选择文件
  → 指纹识别（source JSON / Legado ZIP / Rubato backup）
  → 安全解包到随机临时目录
  → 流式解析 + schema 校验 + 兼容转换
  → 生成 ImportPlan（新增/更新/冲突/跳过/需密码/not_present/present_invalid）
  → UI 预演与冲突策略选择
  → 写 staging/import_items（可取消、可续跑）
  → 单写者执行短事务 apply
  → 外键/计数/抽样校验
  → ImportReport + 清理临时明文
```

安全约束：

- 防 ZIP Slip：拒绝绝对路径、`..`、符号链接和解包后越界路径。
- 设置条目数、单文件大小、总解压大小和压缩比上限，防 zip bomb。
- JSON 使用流式反序列化；不要把整个 ZIP、几千条书源和书架同时变成 `Value` 常驻内存。
- 密码错误、密文损坏和不支持版本必须在 apply 前失败；Cookie/运行时变量明文只存在于受控内存并尽快清零。
- 所有行都有来源条目、原始索引、转换版本、错误码和稳定幂等键。
- 取消发生在解析批次和提交批次边界；已提交类别有明确状态，重新执行不会重复创建。
- 缺少可选类别不触发 apply 失败；只要条目存在，就必须先完成该条目的格式与完整性校验，不能把坏文件降级成“未选择”。

### 5.4 冲突策略

默认提供“合并（推荐）”，另提供“仅新增”和“用备份替换所选类别”。不能只有一个全局覆盖开关。

- 书源定义：相同 `bookSourceUrl` 为同一实体。定义内容按 `lastUpdateTime`/内容 hash 决定是否更新；本地 `enabled/customOrder` 默认保留。用户选择完全恢复时才采用备份偏好。
- 书籍：阶段 0 解除 `(name, author)` 唯一约束后，相同 `bookUrl` 才直接合并；同名同作者但 URL 不同不自动合并，只提示潜在重复。用户自定义封面、简介、标签和单书设置默认保留本地值。该迁移未完成时禁止开放书架 apply，避免设计契约与数据库约束相互矛盾。
- 阅读进度：先比较 `durChapterTime`，较新的进度胜出；时间相同再比较章节索引和章内位置。记录被覆盖前后的摘要供报告展示。
- 分组：保留 Legado group id；若和 Rubato 本地自建组冲突，则创建稳定映射，不改写已有组含义。
- Cookie/运行时变量：只 REPLACE 相同 key，保留本地独有项，与上游恢复语义一致。
- 不支持实体：绝不计入“成功导入”；报告为 `unsupported/preserved/skipped`。

### 5.5 导入结果 API 与 UI

替换当前只返回 `u32` 的 API：

```text
startImport(input, options) -> taskId
watchImport(taskId) -> ImportProgress(stage, bytes, items, warnings)
previewImport(taskId) -> ImportPlan
applyImport(taskId, decisions) -> ImportReport
cancelImport(taskId)
```

`ImportReport` 至少包含：输入格式与版本、耗时、峰值批次、每类新增/更新/跳过/失败数、冲突决策、错误样例（限量）、校验结果。日志和报告都不包含 Cookie、登录数据、完整规则正文和文件绝对路径。

## 6. Rubato 自有备份、恢复与同步预留

### 6.1 Rubato backup v1

ZIP 根部增加 `manifest.json`：

- `format = "rubato-backup"`
- `formatVersion`
- 创建时间、应用版本、平台、数据类别
- 每个条目的 schema 版本、字节数、SHA-256
- 可选加密信息：KDF、salt、AEAD、nonce/chunk 信息
- 不记录设备密钥和明文凭据

manifest 的 `data categories` 是声明清单：声明存在但条目缺失/校验失败属于损坏；没有声明的类别属于用户未选择。A 类 durable media（包括用户自定义或明确持久化的封面/背景）随对应主数据默认备份，D 类网络缓存不备份。

逻辑数据用流式 JSONL 或分块 JSON 保存，不直接复制 `rubato.db` 作为永久格式。生成备份时可用 SQLite Online Backup API 得到一致快照，再从快照导出；严禁只复制 WAL 模式下的 `.db` 而遗漏 `-wal/-shm`。

敏感备份使用现代口令方案（例如 Argon2id + AEAD，参数写入 manifest）；Legado 的旧 AES 方案只存在于兼容导入器，不作为 Rubato 新备份格式。

### 6.2 后台任务

- 用户主动导入/恢复：前台可见任务，有进度与取消；进入后台后是否继续由用户设置决定。
- 自动备份、云同步、缓存维护：Android 使用唯一命名、可重试、带约束的 WorkManager；桌面使用同一 Rust 任务协议，在进程内调度并在下次启动补跑。
- 每个后台任务必须幂等；数据库记录 job key、输入 revision、尝试次数和最后错误。
- 网络同步只传逻辑实体和 tombstone，不上传正在使用的 SQLite 文件。

### 6.3 同步冲突模型（后续阶段）

不要现在就实现通用 CRDT。先为未来保留稳定 entity id、`updated_at`、`row_version`、tombstone 和 operation id：

- 阅读进度采用领域合并：最新阅读时间优先，同时间取更靠后位置。
- 书签/高亮使用 UUID，可独立新增和删除。
- 书源定义和本地启用偏好分离，避免更新规则时覆盖设备偏好。
- 设置区分 roaming 与 device-local；字号/主题可漫游，缓存大小和下载路径不可漫游。
- outbox 上传成功前不能删除；服务端操作以 operation id 去重。

## 7. 性能基线、门禁与优化顺序

### 7.1 复用现有跑子，不另造一套“代表负载”

性能改造必须先过语义门禁。门禁按“改动是否可能改变交给裁判对照的输入或输出”分两档 —— 判断依据是路径，不是文件所在的 crate：

- **全量档**：改动经过 `BookSource::from_value`、source 落库往返，或 `engine`/`pipeline` 中任何进入差分对照的语义路径时，运行 `tools/all_diff.sh`，结果不得比受版本控制的 `fixtures/diff-baseline.json` 退化；改动 `book_sources` 时，除全量门禁外单独核对 source 套件当前全部样本（本次复核为 1727 例）和相关 fetch/pipeline 套件。
- **局部档**：只改 `books`/`chapters`/`caches` 的落库面（阶段 0 的 `save_book` UPSERT、事务合并、前缀查询、PRAGMA 拆分都属此类）时，这些路径根本不进入差分对照，每次全量跑是纯开销。以 store 单测、`EXPLAIN QUERY PLAN` 门禁和 source 套件冒烟为准，合并前再跑一次全量作为收口。

不要在文档里冻结“十六/十七套、99633/100144 例”之类会随样本增长而过期的总数；以当次受版本控制的 runner、baseline 和零新增 FAIL 为准。

新增轻量 store benchmark/诊断 runner，但复用项目已有 fixture 和工作流入口，输出机器可比较的 JSON：

- 开库、迁移、首屏书架查询；
- JsEnv 初始化与 6 worker 搜索，分别记录 `list_cache_prefix` SQL 时间、扫描计划、锁等待和端到端时间；
- 章节缓存命中/未命中、快速翻页进度写；
- 书源与书架导入的 preview/apply、坏行和冲突；
- 缓存淘汰、backup snapshot、checkpoint 与迁移。

每个 workload 固定 profile 构建、fixture、预热次数、重复次数和机器信息，记录 median/p95、RSS 峰值、处理条目/字节、锁等待、SQL 时间、`SQLITE_BUSY`、WAL 大小与 checkpoint 结果。先把修复前结果保存为 B0；测量噪声和产品设备预算后，再把批准的数值写入 versioned performance baseline。没有 B0 的指标只叫“待测”，不叫 SLO。

### 7.2 阶段 0 的确定性门禁

- `list_cache_prefix('js:')` 在代表性大 `caches` fixture 上必须得到索引范围 `SEARCH`，不能出现全表 `SCAN caches`；同时保留返回值等价测试，包括 `%`、`_`、非 ASCII 和空前缀等通用 API 边界。若 API 实际只允许固定命名空间，应收窄 API，避免维护多余的 LIKE 转义语义。
- “同名同作者、不同 URL”保存后两本书都存在，旧书 chapters/自定义字段不变；删除书时 FK cascade 和 cache 清理遵守各自域规则。
- PRAGMA 初始化分别返回结果：每条连接 `foreign_keys=1`；WAL 成功或进入可测试、可观测的明确降级状态。
- 搜索/翻页/导入 workload 不出现新增 `SQLITE_BUSY`、丢 Cookie、丢 source runtime 或完整性错误。
- 每项优化都提供 B0 与 after 的同机对照；actor、读池、压缩和分页改造必须有它们各自命中的瓶颈证据。

### 7.3 优化顺序与决策条件

1. 先把 JS 前缀读取改成索引半开区间/已验证的 GLOB，并按 `source_url` 精确加载；这是低风险、已证明的全表扫描消除项。
2. 大批导入使用 prepared statement + 短事务批次；批次大小由 benchmark 选，不硬编码“越大越快”。
3. 只有列表真实出现大 offset 和 FFI 大对象问题时，才改 keyset pagination；不能机械重写所有列表。
4. 阅读进度若实测出现事务堆积，再引入 per-book coalescing writer；内存保留每本书最新值，换章、退后台和退出阅读页必须 flush。
5. 范围查询修复后若锁等待仍是显著瓶颈，再比较“缩短临界区”“拆锁”“writer actor + 读池”，以最小改动满足预算。actor 不是现代化勋章。
6. 正文缓存使用 zstd 等压缩前先测文本大小、CPU、电量与读取延迟；收益不稳定就保留 `codec=plain`。
7. 封面/图片使用内容寻址文件，数据库保存 hash、来源、耐久类别和元数据；A 类 durable media 与 D 类 cache 使用不同根目录和清理策略。
8. 删除/迁移大批数据后不立即 `VACUUM`；根据 freelist、空间预算和维护窗口决定，必要时再评估 incremental auto-vacuum。

## 8. 安全、隐私与可靠性

- 主库和缓存库只放应用私有目录；Android 文件选择使用 SAF，不申请广域存储权限。
- Android Keystore、macOS Keychain、Windows Credential Manager 保存每安装主密钥/云凭据。平台层只暴露 `seal/open/delete`，Rust 不持久化密钥。
- 差分跑子、`:memory:` 测试和 CI 注入确定性的 `TestSecretStore`；只处理内存库/临时数据的 CLI 探针可使用不落盘的 `EphemeralSecretStore`。持久库缺少持久 SecretStore 时，仅敏感能力返回 `SecretStoreUnavailable`，核心主数据仍可打开；任何环境都不允许为了“跑通”写入不可恢复的临时密钥密文或退回明文持久化。
- Cookie、登录信息、source runtime 和备份口令标为敏感数据；错误、trace、崩溃报告只记录类别、hash 和错误码。
- 导入前展示将要导入的敏感类别；默认不导入 Cookie/运行时变量。
- 恢复或 schema 迁移失败时，原库保持可打开；临时库/快照有状态文件和自动清理期限。
- 每次备份完成后随机抽样读取并校验 manifest hash；恢复完成后执行计数、外键、关键关联和抽样内容校验。
- 缓存损坏只清理单条/分区并重新获取，不能拖垮主库启动。
- 清缓存和缓存损坏恢复不能删除用户自定义/明确持久化的封面与背景；其引用完整性按 A 类主数据校验。

## 9. 分阶段路线图

### 阶段 0：数据正确性护栏（先做）

工作项：

1. **止损，不等产品决策**：加入 P0 复现测试，把 `(name, author)` 唯一索引迁为普通索引 —— 它是独立索引而非表约束，`DROP INDEX` + `CREATE INDEX` 即可，不必重建表。当前那个唯一索引实际在做的事是“删掉一本书、留下孤儿目录”，这不是任何人想要的合并语义，因此这一步**不依赖 §12.2 的产品拍板**，也不能被它挡住；`save_book` 的写法在下一项一并改掉，两项必须同一个发布窗口落地。
2. 把 `save_book` 从部分列 `REPLACE` 改为字段所有权明确的 UPSERT/UPDATE；为所有非本次写入字段建立不丢失测试。
3. 盘点历史孤儿：`chapters` 无 parent book、`books.origin` 指向已删书源两类，各自在迁移快照/报告中保留可恢复线索，按明确策略处置，不能无报告地删。
4. 合并“书 + 目录 + 章节数”事务，并加入第二步失败/进程中断故障注入。
5. 把 `list_cache_prefix` 改为可索引范围查询，冻结等价性测试与 `EXPLAIN QUERY PLAN` 门禁；记录搜索 B0/after 和锁等待变化。
6. 拆分 `journal_mode` 与 `foreign_keys` 初始化，逐条读回；FK 是硬条件，WAL 使用明确能力状态。让 Cookie 写失败返回 typed error 或进入可观测重试。
7. 给 `source_of` 加可区分的“书源已删除”错误，删源前统计并提示受影响书数（换源 UI 可延后，但错误必须先能分辨）。
8. 为当前 schema 建 fixture、schema dump 和数据 round-trip 测试；引入 `user_version/schema_migrations`，把当前状态定义为正式 v1，迁移修复孤儿后再给 chapters 增加 FK。
9. 按 §7.1 的两档规则设定差分门禁：全量档以 `tools/all_diff.sh` 对 `fixtures/diff-baseline.json` 零新增 FAIL 为准，局部档合并前收口一次。
10. 核验各平台 release 产物的 `sqlite_version()`/compile options，完成 rusqlite 升级、受控 backport/fork 等路径的兼容性 spike；在扩大连接并发前落到 SQLite 官方已修复版本。

“是否额外提供人工合并 UI”属于 §12.2 的产品决策，可以延后；工作项 1 落地后，数据库侧已不再隐式代替产品做这个决定，因此它不再阻塞本阶段。

验收：

- 从空库、当前库和旧 29 列 source 库都能迁到 v1。
- 同名同作者、不同 `bookUrl` 的两本书可共存；保存第二本不会删除第一本、其目录或自定义字段，迁移后的 orphan 处置可解释。
- 注入第二步失败时，书、目录和章节数均不改变；保存阅读进度/刷新书籍不会改变自定义列。
- `list_cache_prefix('js:')` 的 query plan 使用索引 `SEARCH` 而非 `SCAN caches`，并提交同机 B0/after 结果。
- 每条连接读回 `foreign_keys=1`；WAL 模式或降级原因可观测；发布产物记录 SQLite 修复状态。
- 删除书源后，指向它的书仍在架、可被列出，打开时得到可区分的“书源已删除”错误而非通用 `NotFound`；受影响书数在删源前已提示。
- `tools/all_diff.sh` 相对受版本控制 baseline 无新增 FAIL；阶段 0 workload 无 busy、丢状态和完整性错误。压力时长与数据规模写进 runner 配置，不在设计文档拍固定分钟数。

### 阶段 1：统一导入框架 + 书源导入升级

工作项：

1. 建 ImportSession、ImportPlan、ImportReport 和流式进度 API。
2. 现有粘贴 JSON 改走统一流水线，返回逐条错误和冲突预演。
3. 拆分 source document 与 local preferences，消除更新定义覆盖启用/排序。
4. 增加大文件、坏行、重复 URL、未知字段和取消测试。

验收：

- 现有 starter.json、source 差分套与全量差分套相对 baseline 无退化；`BookSource::from_value` 往返语义不变。
- 任意坏行不会导致已存在 source 被部分覆盖。
- 重复导入同一文件结果幂等。
- 代表性 source fixture 达到阶段 0 建立的 versioned performance baseline，报告数与数据库数一致。

### 阶段 2：Legado ZIP MVP

工作项：

1. 安全 ZIP reader、格式指纹、资源上限和预演 UI。
2. 支持 `bookSource/bookshelf/bookGroup/bookmark`。
3. 实现书籍冲突、进度合并、本地书跳过报告和缺失 source 检查；把用户未选择的备份类别报告为 `not_present`，存在但无效的条目报告为 `present_invalid`。
4. 加入真实 Legado 备份 fixture：多个上游版本、空项、损坏 ZIP、zip-slip、zip bomb。

验收：

- 导入中断前后原库一致，重试无重复。
- 导入的网络书可显示；首次打开按需刷新目录并恢复到合理章节位置。
- 不支持条目、潜在重复、本地书和未包含类别均在报告中可见；可选类别缺失不被误报成损坏。
- 不出现解包目录逃逸或超限内存。

### 阶段 3：敏感数据、设置与媒体

工作项：

1. 可注入 SecretStore + AEAD 字段加密 + key rotation 元数据：平台实现、`TestSecretStore`、`EphemeralSecretStore` 与 unavailable 行为一起交付。
2. 兼容 Legado `cookies.json/runtimeSourceCache.json` 解密，并用固定向量验证旧格式。
3. typed settings 与 Legado allowlist 映射。
4. 内容寻址媒体导入、空间预检和失败清理。

验收：

- 错口令不会写入任何敏感数据；正确口令导入后明文不落盘。
- 屏幕锁定/密钥失效时能明确提示并清理不可解密会话，不导致主库不可用。
- 差分/CI/CLI 不依赖系统 Keychain/Keystore；测试实现不向磁盘写明文，unavailable 模式不阻断非敏感主库操作。
- 设置导入不改变设备路径、线程/性能参数和平台专属选项。
- 自定义/持久化封面导入 A 类 durable media，清缓存后仍存在并可被 Rubato 自有备份恢复。

### 阶段 4：主库/缓存分离与按证据扩展并发

工作项：

1. 新建 cache DB，将正文缓存从通用 KV 渐进搬迁；旧缓存只读回退一个版本后删除。新表以 `(book_url, chapter_url)` 两列为键，**不再拼 `content:<bookUrl>:<chapterUrl>` 字符串** —— bookUrl 自身可以含 `:`，前缀删除在 `http://a/b` 与 `http://a/b:x` 这类相邻 URL 上会误删邻居的缓存。只伤可重建数据，所以不进阶段 0；但不能把这个歧义原样搬进新库。
2. 将 A 类 durable media 与 D 类网络缓存迁到不同目录/引用类型，加入引用修复与配额测试。
3. 根据阶段 0～3 的锁等待和 SQL 指标决定是否需要拆锁、单 writer actor 和读池；没有剩余瓶颈就保留简单模型。
4. 仅对实测瓶颈引入 per-book 进度合并写和列表 keyset pagination；source runtime 在阶段 0 已先完成索引精确加载。
5. 完善数据库/缓存 benchmark、WAL/slow query/busy/cache-hit 指标。

验收：

- 清缓存不会丢书架、进度、登录状态或 source runtime。
- 清缓存不会删除自定义/持久化封面与背景，A/D 类引用校验全绿。
- 新旧版本切换期间无双写冲突；旧路径删除条件明确。
- 所有已批准的 versioned performance baseline 达标；若引入 actor/读池，决策记录包含 B0、候选实现和 after 数据。

### 阶段 5：Rubato backup v1 与可靠后台任务

工作项：

1. versioned manifest、逻辑导出、校验、现代口令加密。
2. 恢复预演与选择性恢复，复用 ImportPipeline。
3. Android WorkManager 唯一任务；桌面补跑协议。
4. 增加 Legado best-effort 导出。

验收：

- 跨 Android/macOS/Windows 往返恢复主数据一致。
- A 类 durable media 随主数据恢复；D 类网络缓存不进入备份。
- 任意条目被截断/篡改都在 apply 前检测。
- 应用被杀、设备重启、重复调度不会生成并行备份或重复提交。

### 阶段 6：同步（有产品需求时再做）

先确定服务端、账号、端到端加密和多设备冲突产品规则，再实现 outbox、tombstone 和增量同步。不要为“现代化”提前引入未被产品需要的分布式复杂度。

## 10. 测试与发布验收矩阵

| 范围 | 必测内容 | 发布门禁 |
|---|---|---|
| 现有差分兼容 | 按 §7.1 两档：触及裁判对照路径走全量 `tools/all_diff.sh`，source 改动额外核对 source 与相关 fetch/pipeline 套件；纯落库面改动走局部档并在合并前收口一次全量 | 相对 `fixtures/diff-baseline.json` 无新增 FAIL；禁止用新单测替代现有裁判路径；也禁止用“跑了全量”替代局部档该有的 query plan 与保留字段测试 |
| schema | 每个已发布版本升级、旧 29 列 source 库、孤儿修复、索引变化、空间不足、断电注入 | fixture 全绿；同名同作者不同 URL 可共存；无破坏性 fallback |
| Repository | CRUD、事务回滚、并发读写、字段保留、FK cascade、每连接 PRAGMA 读回 | 单元/集成测试全绿；`foreign_keys=1`；无静默字段/子项删除 |
| 引用完整性 | 删书源后书的可列出性与错误可分辨、删书后 chapters/缓存清理、导入期源尚未落库的中间状态、历史悬空 `origin` 盘点 | 悬空引用不产生静默失败；“书源已删除”与通用 `NotFound` 可区分；合法中间状态不被写入失败阻断 |
| 查询计划 | 大正文缓存下 JS 前缀查询、返回等价性、锁等待 | `SEARCH` 索引范围而非 `SCAN caches`；提交 B0/after |
| Legado import | 可选类别缺失、存在但损坏、版本差异、坏 JSON、未知字段、重复、冲突、本地书、加密项 | `not_present`/`present_invalid` 可区分；计数和报告可复算；重复导入幂等 |
| ZIP 安全 | slip、symlink、bomb、超大条目、损坏 CRC | apply 前拒绝，临时文件清理 |
| 敏感数据 | 错口令、密钥失效、无平台 SecretStore、日志扫描、备份默认项 | 明文不落盘/日志；默认不导入；CI/CLI 可运行且不降级明文 |
| 媒体耐久 | 自定义封面、网络封面、清缓存、备份恢复、引用损坏 | A 类随备份恢复且清缓存不删；D 类可重建且默认不备份 |
| 性能 | 固定 fixture/profile 的开库、搜索、缓存、导入、翻页、维护 workload | 通过 versioned baseline；无基线的候选优化不得宣称达标 |
| 可靠性 | kill -9、恢复重试、磁盘满、只读目录、SQLITE_BUSY 注入 | 原库可开；任务状态可解释 |
| 备份 | 在线写入时快照、hash、跨平台恢复、旧格式读取 | 一致快照；篡改可检测 |

## 11. 不做的事情

- 不直接复制 Legado `legado.db` 覆盖 Rubato 主库。
- 不为追求框架统一，把 Rust 数据事实源迁到 Dart/Android Room。
- 不把不支持的 Legado 数据静默丢掉后仍显示“全部导入成功”。
- 不让新旧实现同时写同一事实；迁移期只能有单一写方向。
- 不先实现云同步再定义实体身份、冲突和删除语义。
- 不在没有发布型 benchmark 的情况下随意调整 SQLite cache/page/mmap/synchronous 参数。

## 12. 待确认的产品决策

这些决策不阻塞阶段 0～1，但进入 Legado ZIP MVP 前要定：

1. 首版是否承诺导入本地 TXT/EPUB；如果承诺，必须同时设计文件授权、跨平台路径和媒体搬迁，不能只导入一条失效路径。
2. 在阶段 0 默认采用“`bookUrl` 为身份、同名同作者不同 URL 保留两本”的前提下，首版是否另提供人工合并 UI。这条**不阻塞阶段 0**：解除唯一索引是纯技术止损（现状是静默删书），已在工作项 1 无条件执行；此处只决定是否额外给用户一个手动合并入口。无论 UI 是否提供，数据库都不再用唯一索引隐式代替产品决策。
3. Cookie/登录变量导入是否只面向高级用户，以及是否接受 Legado 旧备份口令兼容风险。
4. Rubato 备份默认是否包含书源规则；规则可能包含用户 token，需要在导出前做敏感字段提示。
5. 自动备份的目标是本地、WebDAV 还是后续账号云；这决定阶段 5 的平台与认证工作量。

## 13. 依据

- Android 官方数据层建议：Repository 作为数据入口，每类数据定义单一事实源，数据库适合需查询/关联/局部更新的数据，设置适合小型强类型存储：<https://developer.android.com/topic/architecture/data-layer>
- Android 官方持久后台工作：需要离开页面、进程重启后仍可靠执行的工作使用 WorkManager，并使用 unique work、约束和重试：<https://developer.android.com/develop/background-work/background-tasks/persistent>
- SQLite WAL：读写可并发但只有一个 writer；checkpoint、长读和 WAL 大小需要测量治理；复制 WAL 数据库时 WAL 文件属于持久状态：<https://sqlite.org/wal.html>
- SQLite WAL-reset bug 公告：受影响版本、触发条件，以及 3.51.3、3.50.7、3.44.6 修复版本：<https://sqlite.org/wal.html#the_wal_reset_bug>
- rusqlite releases：用于核对各版本的 bundled SQLite 和破坏性变更，不能只改版本号：<https://github.com/rusqlite/rusqlite/releases>
- SQLite `REPLACE`：遇到 UNIQUE/PRIMARY KEY 冲突时会删除既有行再继续插入：<https://sqlite.org/lang_conflict.html>
- SQLite UPSERT：冲突目标和多个 `ON CONFLICT` 子句的语法能力；实体身份与合并规则仍需由产品定义：<https://sqlite.org/lang_upsert.html>
- SQLite 查询优化：LIKE/GLOB 使用索引的 collation、pattern 与 ESCAPE 前提；本项目最终以 `EXPLAIN QUERY PLAN` 门禁为准：<https://sqlite.org/optoverview.html#the_like_optimization>
- SQLite 外键：外键按连接启用，启用后必须读回并在迁移后执行一致性检查：<https://sqlite.org/foreignkeys.html>
- SQLite Online Backup API：在线数据库一致快照：<https://sqlite.org/backup.html>
- SQLite `PRAGMA optimize`：<https://sqlite.org/pragma.html#pragma_optimize>
