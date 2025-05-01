use crate::{Hitokoto, QueryParams};
use rand::prelude::*;
use sqlx::any::{AnyKind, AnyPool, AnyPoolOptions};
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Duration;

pub struct DbState {
    pub pool: AnyPool,    // 数据库连接池
    pub count: AtomicI64, // 总条数
}

impl Clone for DbState {
    fn clone(&self) -> Self {
        Self {
            pool: self.pool.clone(),
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

    Ok(DbState { pool, count })
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

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM hitokoto")
        .fetch_one(&memory_pool)
        .await?;

    let count: AtomicI64 = AtomicI64::new(count);
    pool.close().await;
    Ok(DbState {
        pool: memory_pool,
        count,
    })
}

pub fn build_query_conditions(params: &QueryParams, state: &DbState) -> (String, Vec<String>) {
    let mut conditions = vec!["1=1".to_string()];
    let mut query_params = vec![];

    if let Some(categories) = &params.c {
        let categories: Vec<&str> = categories.split(',').collect();
        if !categories.is_empty() {
            conditions.push(format!(
                "type IN ({})",
                categories.iter().map(|_| "?").collect::<Vec<_>>().join(",")
            ));
            query_params.extend(categories.iter().map(|c| c.to_string()));
        }
    }

    if let Some(min) = params.min_length {
        conditions.push("length >= ?".to_string());
        query_params.push(min.to_string());
    }

    if let Some(max) = params.max_length {
        conditions.push("length <= ?".to_string());
        query_params.push(max.to_string());
    }

    // 用运行时判断替换编译时条件
    let rand = match state.pool.any_kind() {
        AnyKind::MySql => "RAND()",
        _ => "RANDOM()",
    };
    (
        format!(
            "SELECT * FROM hitokoto WHERE {} ORDER BY {} LIMIT 1",
            conditions.join(" AND "),
            rand,
        ),
        query_params,
    )
}

// 通用查询执行函数
pub async fn execute_query_with_params(
    state: &DbState,
    query: &str,
    params: &[&str],
) -> Result<Option<Hitokoto>, sqlx::Error> {
    let mut q = sqlx::query_as::<_, Hitokoto>(query);
    for param in params {
        q = q.bind(param);
    }
    q.fetch_optional(&state.pool).await
}

pub async fn rand_hitokoto_without_params(
    state: &DbState,
) -> Result<Option<Hitokoto>, sqlx::Error> {
    // 生成随机索引
    let rand_index = rand::rng().random_range(0..state.count.load(Ordering::Relaxed));

    // 构造带 OFFSET 的查询
    let query = format!("SELECT * FROM hitokoto LIMIT 1 OFFSET {}", rand_index);

    // 执行查询
    execute_query_with_params(state, &query, &[]).await
}

pub async fn table_exists(pool: &AnyPool, table_name: &str) -> Result<bool, sqlx::Error> {
    let query = match pool.any_kind() {
        sqlx::any::AnyKind::Postgres => {
            "SELECT EXISTS (SELECT 1 FROM information_schema.tables WHERE table_name = $1)"
        }
        sqlx::any::AnyKind::MySql => {
            "SELECT COUNT(*) > 0 FROM information_schema.tables WHERE table_name = ?"
        }
        sqlx::any::AnyKind::Sqlite => {
            "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type = 'table' AND name = ?"
        }
        _ => return Err(sqlx::Error::Configuration("Unsupported database".into())),
    };

    let exists: bool = sqlx::query_scalar(query)
        .bind(table_name)
        .fetch_one(pool)
        .await?;

    Ok(exists)
}