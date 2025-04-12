# hitokoto-rust 🦀

基于 Actix-web 和 SQLx 的高性能 Rust「一言」API 服务实现（原项目：[hitokoto-osc/hitokoto-api](https://github.com/hitokoto-osc/hitokoto-api)）。

## 🚀 功能特性

- **多数据库支持**：原生支持 MySQL/PostgreSQL/SQLite（通过编译特性切换）
- **极致性能**：基于 Actix-web 异步框架 + SQLx 零成本抽象
- **动态配置**：支持线程数、连接池、监听地址等运行时参数
- **智能过滤**：分类组合/长度范围/返回格式控制
- **生产就绪**：内置连接池管理、异步 I/O、内存安全保证
- **跨平台**：支持 Linux/macOS/Windows 部署

## 📦 环境要求

- Rust 1.65+ 工具链
- 数据库（任选其一）：
  - MySQL 5.7+ / MariaDB 10.3+
  - PostgreSQL 12+
  - SQLite 3.35+

## 🛠️ 安装与运行

### 编译选项
```bash
cargo build --release 

# 带mimalloc的编译
cargo build --release --features mimalloc
```

### 快速启动
```bash
# 使用默认配置（MySQL）
HITOKOTO_DB="mysql://user:pass@localhost/hitokoto" \
./target/release/hitokoto-rust

# 自定义配置示例
./target/release/hitokoto-rust \
    --host 0.0.0.0 \
    --port 8080 \
    --database "postgres://user:pass@localhost/hitokoto" \
    --workers 8 \
    --max-connections 20
```

## ⚙️ 配置项

| 参数             | 环境变量              | 默认值                                   | 说明                                     |
| ---------------- | --------------------- | ---------------------------------------- | ---------------------------------------- |
| `--host/-h`      | HITOKOTO_HOST         | 0.0.0.0                                  | 监听地址                                 |
| `--port/-p`      | HITOKOTO_PORT         | 8080                                     | 监听端口                                 |
| `--database/-d`  | HITOKOTO_DB           | mysql://root:password@localhost/hitokoto | 数据库连接字符串                         |
| `--workers/-w`   | HITOKOTO_WORKERS      | CPU 核心数                               | 工作线程数                               |
| `--memory/-M`    | HITOKOTO_MEMORY       | False                                    | 是否将数据全部加载至内存（极大提升性能） |
| `--limiter`      | HITOKOTO_LIMITER      | False                                    | 是否使用限流器                           |
| `--limiter_rate` | HITOKOTO_LIMITER_RATE | 10                                       | 限流器速率（每秒请求数）                 |

## 📡 API 文档

### 随机获取语句
```
GET /
```

**请求参数**：
| 参数         | 类型    | 示例值    | 说明                       |
| ------------ | ------- | --------- | -------------------------- |
| `c`          | string  | a,b,c     | 分类过滤（逗号分隔多个值） |
| `encode`     | string  | text/json | 响应格式（默认JSON）       |
| `min_length` | integer | 50        | 最小字符长度               |
| `max_length` | integer | 100       | 最大字符长度               |

**分类标识符**：
- `a`: 动画
- `b`: 漫画
- `c`: 游戏
- `d`: 文学
- `e`: 原创

### 按 UUID 查询
```
GET /{uuid}
```

## 🗄️ 数据库结构
```sql
-- 通用表结构（适配不同数据库语法）
CREATE TABLE hitokoto (
    id          INT PRIMARY KEY,
    uuid        VARCHAR(36) UNIQUE NOT NULL,
    text        TEXT NOT NULL,
    type        VARCHAR(1) NOT NULL,  -- 分类标识
    from_source VARCHAR(255) NOT NULL,
    from_who    VARCHAR(255),
    length      INT NOT NULL
);
```

## 🧩 高级配置

### 全部加载至内存

### 连接池调优
通过 `--max-connections` 设置连接池大小，推荐公式：  
`max_connections = (workers * 2) + 1`

### 内存分配器
可以使用 mimalloc 作为全局分配器（非 MSVC 环境）

### 线程数调优
通过 `--workers` 设置工作线程数，默认为 CPU 核心数

## 技术栈
- Actix-web: 异步 Web 框架
- SQLx : 数据库抽象层
- Mimalloc : 内存分配器
- simd-json : JSON 解析器
- clap: 命令行参数解析

## 📜 开源协议
MIT License © 2025 MoYan
