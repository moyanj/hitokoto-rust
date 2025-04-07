use crate::Hitokoto;
use sqlx::any::{AnyKind, AnyPool, AnyPoolOptions};
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Duration;

pub struct DbState {
    pub pool: AnyPool,
    pub db_kind: AnyKind,
    pub count: AtomicI64,
}

impl Clone for DbState {
    fn clone(&self) -> Self {
        Self {
            pool: self.pool.clone(),
            db_kind: self.db_kind.clone(),
            count: AtomicI64::new(self.count.load(Ordering::SeqCst)),
        }
    }
}

/// 获取数据库连接池
///
/// # 参数
/// - `database_url`: 数据库连接字符串
/// - `max_connections`: 最大连接数
/// - `connect_timeout`: 连接超时时间(秒)
/// - `idle_timeout`: 空闲连接超时时间(秒)
pub async fn get_pool(
    database_url: &str,
    max_connections: u32,
    connect_timeout: u64,
    idle_timeout: u64,
) -> Result<DbState, sqlx::Error> {
    let pool = AnyPoolOptions::new()
        .max_connections(max_connections)
        .acquire_timeout(Duration::from_secs(connect_timeout))
        .idle_timeout(Duration::from_secs(idle_timeout))
        .connect(database_url)
        .await?;

    // 获取数据库类型
    let db_kind = pool.any_kind();

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM hitokoto")
        .fetch_one(&pool)
        .await?;

    let count: AtomicI64 = AtomicI64::new(count);

    if count.load(Ordering::Relaxed) == 0 {
        panic!("No data found.");
    } else {
        println!("Database type: {:?}", db_kind);
        println!("Total records: {}", count.load(Ordering::Relaxed));
    }

    Ok(DbState {
        pool,
        db_kind,
        count,
    })
}

/// 将数据加载到内存中的SQLite数据库
pub async fn load_data_to_memory(pool: &AnyPool) -> Result<DbState, sqlx::Error> {
    // 创建内存中的SQLite数据库连接池
    let memory_pool = AnyPoolOptions::new()
        .max_connections(1) // 内存数据库通常只需要一个连接
        .connect("sqlite::memory:?cache=shared")
        .await?;

    // 创建表结构
    sqlx::query(
        r#"
        CREATE TABLE hitokoto (
            id INTEGER PRIMARY KEY,
            uuid TEXT NOT NULL,
            text TEXT NOT NULL,
            type TEXT NOT NULL,
            from_source TEXT NOT NULL,
            from_who TEXT,
            length INTEGER NOT NULL
        )
        "#,
    )
    .execute(&memory_pool)
    .await?;

    // 从原始数据库复制数据
    let hitokotos = sqlx::query_as::<_, Hitokoto>("SELECT * FROM hitokoto")
        .fetch_all(pool)
        .await?;

    for hitokoto in hitokotos {
        sqlx::query(
            r#"
            INSERT INTO hitokoto (id, uuid, text, type, from_source, from_who, length)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(hitokoto.id)
        .bind(hitokoto.uuid)
        .bind(hitokoto.text)
        .bind(hitokoto.r#type)
        .bind(hitokoto.from_source)
        .bind(hitokoto.from_who)
        .bind(hitokoto.length)
        .execute(&memory_pool)
        .await?;
    }
    // 创建UUID索引
    sqlx::query("CREATE INDEX idx_uuid ON hitokoto (uuid)")
        .execute(&memory_pool)
        .await?;
    // 创建类型,长度联合索引
    sqlx::query("CREATE INDEX idx_type_length ON hitokoto (type, length)")
        .execute(&memory_pool)
        .await?;

    // 设置PRAGMA
    sqlx::query(
        "PRAGMA journal_mode = MEMORY;  -- 内存日志模式
PRAGMA synchronous = OFF;      -- 禁用同步写入
PRAGMA locking_mode = EXCLUSIVE; -- 独占锁（只读模式下无害）
",
    )
    .execute(&memory_pool)
    .await?;

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM hitokoto")
        .fetch_one(&memory_pool)
        .await?;

    let count: AtomicI64 = AtomicI64::new(count);
    pool.close().await;
    Ok(DbState {
        pool: memory_pool,
        db_kind: AnyKind::Sqlite,
        count,
    })
}
