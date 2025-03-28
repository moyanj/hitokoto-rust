```markdown
# hitokoto-rust 🦀

一个基于 Actix-web 和 sqlx 的高性能 Rust「一言」API 服务实现（原项目：[hitokoto-osc/hitokoto-api](https://github.com/hitokoto-osc/hitokoto-api)）。

## 🚀 功能特性

- **多数据库支持**：原生支持 MySQL/PostgreSQL/SQLite（通过特性切换）
- **高性能**：基于 Actix-web 异步框架和 SQLx 编译时校验的查询
- **动态配置**：支持线程数、连接池、监听地址等参数自定义
- **灵活过滤**：支持分类过滤、长度范围、返回格式控制
- **零成本抽象**：无额外抽象层，直接操作数据库
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
# 默认编译（启用 MySQL 特性）
cargo build --release 

# 编译 PostgreSQL 版本
cargo build --release --features "postgres"

# 编译 SQLite 版本
cargo build --release --features "sqlite"

# 全部编译
cargo build --release --all-features
```

### 快速启动
```bash
# 使用默认配置（MySQL）
HITOKOTO_DB="mysql://user:pass@localhost/dbname" \
./target/release/hitokoto-rust

# 自定义参数示例
./target/release/hitokoto-rust \
    --host 0.0.0.0 \
    --port 8080 \
    --database "postgres://user:pass@localhost/dbname" \
    --workers 8 \
    --max-connections 20
```

## ⚙️ 配置项

| 参数                | 环境变量                 | 默认值                          | 说明                   |
| ------------------- | ------------------------ | ------------------------------- | ---------------------- |
| `--host`            | HITOKOTO_HOST            | 0.0.0.0                         | 监听地址               |
| `--port`            | HITOKOTO_PORT            | 8080                            | 监听端口               |
| `--database`        | HITOKOTO_DB              | mysql://root@localhost/hitokoto | 数据库连接字符串       |
| `--workers`         | HITOKOTO_WORKERS         | CPU 核心数                      | 工作线程数             |
| `--max-connections` | HITOKOTO_MAX_CONNECTIONS | 10                              | 数据库连接池最大连接数 |

## 📡 API 文档

### 随机获取语句
```
GET /
```

**参数说明**：
| 参数         | 类型    | 示例值    | 说明                       |
| ------------ | ------- | --------- | -------------------------- |
| `c`          | string  | a,b,c     | 分类过滤（多个用逗号分隔） |
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
-- 通用表结构（具体语法需适配数据库）
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

## 🧩 高级功能

### 连接池配置
通过 `--max-connections` 控制连接池大小，建议设置为 `(workers * 2) + 1`W

## 📜 开源协议
MIT License © 2024 MoYan
