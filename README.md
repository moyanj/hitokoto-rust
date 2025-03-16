# hitokoto-rust 🦀

一个基于 Actix-web 和 sqlx 的高性能 Rust「一言」API 服务实现（原项目：[https://github.com/hitokoto-osc/hitokoto-api](https://github.com/hitokoto-osc/hitokoto-api)）。

## 功能特性

- 🌟 纯 Rust 实现，高性能低资源占用
- 📦 开箱即用，单一可执行文件部署
- 📚 支持多种返回格式（JSON/纯文本）
- 🎯 智能分类过滤（参数`c`指定类型）
- 📏 支持长度范围过滤（min_length/max_length）
- 🔒 线程安全数据库访问（Arc+Mutex）
- ⚙️ 可配置工作线程数（自动检测CPU核心数）

## 快速开始

### 环境要求
- Rust 1.65+ 工具链
- SQLite 3.35+

### 安装运行
```bash
# 克隆仓库
git clone https://github.com/moyanj/hitokoto-rust.git
cd hitokoto-rust

# 编译发布版本
cargo build --release

# 运行服务（默认参数）
./target/release/hitokoto-rust

# 自定义参数运行
./target/release/hitokoto-rust \
    --host 0.0.0.0:8080 \
    --workers 4
```

## API 使用说明

### 基础请求
```
GET /
```

### 请求参数
| 参数       | 类型    | 说明                                                                                      |
| ---------- | ------- | ----------------------------------------------------------------------------------------- |
| c          | string  | 分类过滤，用逗号（`,`）分割（可选值：a-anime, b-comic, c-game, d-literature, e-original） |
| encode     | string  | 返回格式（可选值：json/text，默认json）                                                   |
| min_length | integer | 最小字符长度限制                                                                          |
| max_length | integer | 最大字符长度限制                                                                          |

### 示例请求
- 获取随机句子：`http://localhost:8000/`
- 指定动漫类型：`http://localhost:8000/?c=a`
- 纯文本格式：`http://localhost:8000/?encode=text`
- 长度限制：`http://localhost:8000/?min_length=50&max_length=100`

## 数据库结构
```sql
CREATE TABLE hitokoto (
    id INTEGER PRIMARY KEY,
    uuid TEXT NOT NULL UNIQUE,
    text TEXT NOT NULL,
    type TEXT NOT NULL,
    from TEXT NOT NULL,
    from_who TEXT,
    length INTEGER NOT NULL
);
```

## 性能特点
- 🚀 基于 Actix-web 的高性能异步框架
- 💾 使用 sqlx 进行高效数据库操作
- 🔄 多线程安全数据库访问
- ⚡ 自动检测 CPU 核心数分配工作线程

## 贡献指南
1. 安装 Rust 工具链
2. 克隆仓库：`git clone https://github.com/moyanj/hitokoto-rust.git`
3. 代码格式化：`cargo fmt`
4. 提交 Pull Request

## 许可证
MIT License © 2025 MoYan