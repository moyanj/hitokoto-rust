# hitokoto-rust 🦀

[![Ask DeepWiki](https://deepwiki.com/badge.svg)](https://deepwiki.com/moyanj/hitokoto-rust)

基于 Actix-web 和 SQLx 的高性能 Rust 版【一言服务】实现，提供轻量、安全、高性能的 API。支持数据库切换、限流、内存优化等参数配置。

---

## 🚀 功能特性

- 🔌 多数据库支持：MySQL / SQLite
- 🚀 高性能：异步架构 + 编译时 SQL 优化
- 💡 内存优化模式：`--memory` 可将数据加载至内存数据库，进一步加速 3–10 倍(0.7.1+ 版本因未知问题可能导致轻微性能下降)
- 🧠 动态配置：线程数、连接池、监听地址等可灵活调整
- 📏 智能查询：按类型、长度与格式返回数据
- 🛡️ 生产就绪：连接池、异态 I/O、Litmus-tested 异步安全
- 📊 内置请求限流器：可防止 DDoS 或突增请求穿透系统

---

## 📦 环境依赖

- ✅ **Rust 工具链** v1.85+
- 🗄 **数据库支持**（任选其一）：
  - **MySQL 5.7+ / MariaDB 10.3**
  - **SQLite 3.35+**（推荐用于调试场景）

---

## 🛠 安装部署说明

### 网络预构建包（从 GitHub Actions 获取）

我们为以下平台提供 Release：
- `x86_64-unknown-linux-gnu`
- `x86_64-unknown-linux-musl`
- `x86_64-pc-windows-msvc`
- `aarch64-unknown-linux-gnu`
- `aarch64-unknown-linux-musl`

[🔗 Actions - build.yml 构建页面](https://github.com/moyanj/hitokoto-rust/actions/workflows/build.yml)

### 本地构建方式

```bash
# 标准构建
cargo build --release
```

---

## 🚪 使用方式与启动命令

终端启动示例（带内存模式 + 数据库 & worker 配置）：

```bash
# 使用默认 MySQL 配置
HITOKOTO_DB="mysql://root:password@localhost/hitokoto" \
./target/release/hitokoto-rust

# 启动 SQLite，内存优化，8 工作线程
./target/release/hitokoto-rust \
    --database "sqlite://./1.db" \
    --memory \
    --workers 8

# 启用限流（每秒最多 100 个请求）
./target/release/hitokoto-rust --limiter --limiter_rate 100
```

---

## ⚙️ 运行时参数配置

| 参数                | 环境变量                   | 默认值                                     | 说明                           |
| ------------------- | -------------------------- | ------------------------------------------ | ------------------------------ |
| `-h` / `--host`     | `HITOKOTO_HOST`            | `0.0.0.0`                                  | 服务绑定地址                   |
| `-p` / `--port`     | `HITOKOTO_PORT`            | `8080`                                     | 服务开放端口                   |
| `-d` / `--database` | `HITOKOTO_DB`              | `mysql://root:password@localhost/hitokoto` | 数据库连接详情（DSN）          |
| `-w` / `--workers`  | `HITOKOTO_WORKERS`         | CPU 核心数                                 | 指定适量线程关系以优化多核调度 |
| `-m` / `--memory`   | `HITOKOTO_MEMORY`          | `false`                                    | 预加载数据至内存 SQLite        |
| `--limiter`         | `HITOKOTO_LIMITER`         | `false`                                    | 启用限流器                     |
| `--limiter_rate`    | `HITOKOTO_LIMITER_RATE`    | `10`                                       | 限流速率：RPS（每秒请求）      |
| `--init`            | -                          | `false`                                    | 初始化数据库                   |
| `--max-connections` | `HITOKOTO_MAX_CONNECTIONS` | `10`                                       | 最大数据库连接池大小           |

---

## ⚙️ 性能测试结果

### 🧪 启动命令

启用内存 SQLite，8 worker

```bash
target/release/hitokoto-rust -d sqlite://1.db -m -w 8
```

### 📊 压测输出

```bash
$ wrk -t8 -c1000 -d10s --latency http://127.0.0.1:8080
Running 10s test @ http://127.0.0.1:8080
  8 threads and 1000 connections
  Thread Stats   Avg      Stdev     Max   +/- Stdev
    Latency    82.78ms    4.48ms 101.06ms   95.82%
    Req/Sec     1.48k   275.01     2.21k    71.00%
  Latency Distribution
     50%   82.57ms
     75%   84.71ms
     90%   86.36ms
     99%   90.07ms
  117891 requests in 10.04s, 53.65MB read
Requests/sec:  11743.96
Transfer/sec:      5.34MB

```

---

### 🖥️ 机器环境信息
```bash
$ neofetch

██████████████████  ████████   moyan@moyan-pc
██████████████████  ████████   --------------
██████████████████  ████████   OS: Manjaro Linux x86_64
██████████████████  ████████   Kernel: 6.15.6-zenm1+
████████            ████████   Uptime: 21 hours, 25 mins
████████  ████████  ████████   Packages: 1844 (pacman)
████████  ████████  ████████   Shell: zsh 5.9
████████  ████████  ████████   CPU: Intel Xeon E5-2673 v3 (24) @ 3.100GHz
████████  ████████  ████████   GPU: NVIDIA GeForce GT 740
████████  ████████  ████████   Memory: 4635MiB / 31991MiB
████████  ████████  ████████
████████  ████████  ████████
```

---

## 📡 API 接口文档

### 🎲 随机返回一言语录

```
GET /
```

| 参数         | 类型  | 示例值      | 说明                           |
| ------------ | ----- | ----------- | ------------------------------ |
| `c`          | `str` | `a,b,c`     | 类型过滤（逗号分隔）           |
| `encode`     | `str` | `text/json` | 响应格式设定                   |
| `min_length` | `int` | `50`        | 最小字符数                     |
| `max_length` | `int` | `100`       | 最大字符数（谢绝超长语句穿透） |

#### 分类表：

| 标记 | 类型 |
| ---- | ---- |
| `a`  | 动画 |
| `b`  | 漫画 |
| `c`  | 游戏 |
| `d`  | 文学 |
| `e`  | 原创 |

#### 响应示例（`?encode=json` 默认）：

```json
{
    "uuid": "bb596739-d5ac-433c-9c4c-406387287576",
    "text": "哪怕前方荆棘密布，也曾由我亲手斩尽。",
    "type": "e",
    "from": "MoYan",
    "from_who": null,
    "length": 30,
    "created_at": "2025-08-04T19:32:26Z"
}
```

### 🔍 查某句语录（通过 UUID）

```
GET /{uuid}
```

---

## 🗄️ 数据库结构（SQL）

本地 SQLite / MySQL：

```sql
CREATE TABLE IF NOT EXISTS hitokoto (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    uuid TEXT UNIQUE NOT NULL,
    text TEXT NOT NULL,
    type TEXT NOT NULL,
    from_source TEXT NOT NULL,
    from_who TEXT,
    length INTEGER NOT NULL
)
```

---

## 🧩 高级调优建议

### 📈 内存模式：高性能场景取胜关键

0.7.1+ 版本的内存模式因未知问题，可能相较非内存模式存在 5-7% 的性能下降

```bash
# 指定启用内存加速模式
target/release/hitokoto-rust -d sqlite://1.db -m
```

将语录一次性载入内存数据库，性能可提升 3–10 倍。

### 🔧 连接池配置公式

```bash
HITOKOTO_MAX_CONNECTIONS = (HITOKOTO_WORKERS * 2) + 1
```

---

## 🛠️ 核心技术栈说明

- **Actix-web**：纯异步 Web 框架，提供零损耗 HTTP 服务核心
- **SQLx**：在 Rust 中无需运行时 ORM，SQL 编译期通过特性自动切换 MySQL / SQLite
- **tokio**：Rust 高性能异步运行时支持
- **mimalloc**：兼容性强、延迟低的内存分配器

---

## 📜 协议与版权

MIT License © 2025 MoYan