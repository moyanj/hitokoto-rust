use crate::{Hitokoto, QueryParams};
use arc_swap::ArcSwap;
use rand::prelude::*;
use sqlx::any::{AnyKind, AnyPool, AnyPoolOptions};
use std::sync::Arc;
use std::sync::atomic::{AtomicI32, Ordering};
use std::time::Duration;

pub struct DbState {
    pub pool: AnyPool,               // 数据库连接池
    pub count: AtomicI32,            // 总条数
    pub max_length: AtomicI32,       // 最大长度
    pub min_length: AtomicI32,       // 最大长度
    pub uuids: ArcSwap<Vec<String>>, // UUID列表
}

impl Clone for DbState {
    fn clone(&self) -> Self {
        Self {
            pool: self.pool.clone(),
            count: AtomicI32::new(self.count.load(Ordering::SeqCst)),
            max_length: AtomicI32::new(self.max_length.load(Ordering::SeqCst)),
            min_length: AtomicI32::new(self.min_length.load(Ordering::SeqCst)),
            uuids: ArcSwap::new(self.uuids.load().clone()),
        }
    }
}

impl DbState {
    pub async fn update(&self) -> Result<(), sqlx::Error> {
        let query = "SELECT COUNT(*) FROM hitokoto";
        let count = sqlx::query_scalar::<_, i32>(query)
            .fetch_one(&self.pool)
            .await
            .unwrap();

        self.count.store(count, Ordering::Relaxed);

        let (max_l, min_l) = get_length_stats(&self.pool).await.unwrap();
        self.max_length.store(max_l, Ordering::Relaxed);
        self.min_length.store(min_l, Ordering::Relaxed);

        let uuids = sqlx::query_scalar::<_, String>("SELECT uuid FROM hitokoto")
            .fetch_all(&self.pool)
            .await?;
        self.uuids.store(Arc::new(uuids));

        Ok(())
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

    let count: i32 = sqlx::query_scalar("SELECT COUNT(*) FROM hitokoto")
        .fetch_one(&pool)
        .await?;

    let count: AtomicI32 = AtomicI32::new(count);

    if count.load(Ordering::Relaxed) == 0 {
        panic!("No data found.");
    } else {
        println!("Database type: {:?}", db_kind);
        println!("Total records: {}", count.load(Ordering::Relaxed));
    }

    let (max_l, min_l) = get_length_stats(&pool).await.unwrap();
    let max_l = AtomicI32::new(max_l);
    let min_l = AtomicI32::new(min_l);

    // 加载UUID列表
    let uuids = sqlx::query_scalar::<_, String>("SELECT uuid FROM hitokoto")
        .fetch_all(&pool)
        .await?;
    let uuids = Arc::new(uuids);

    Ok(DbState {
        pool,
        count,
        max_length: max_l,
        min_length: min_l,
        uuids: ArcSwap::new(uuids),
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

    let uuid_list: Vec<String> = hitokotos.iter().map(|h| h.uuid.clone()).collect(); // 创建UUID列表

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

    // 获取数据库统计信息
    let count: i32 = sqlx::query_scalar("SELECT COUNT(*) FROM hitokoto")
        .fetch_one(&memory_pool)
        .await?;

    let count: AtomicI32 = AtomicI32::new(count);

    let (max_l, min_l) = get_length_stats(pool).await.unwrap();
    let max_l = AtomicI32::new(max_l);
    let min_l = AtomicI32::new(min_l);

    pool.close().await; // 关闭原始数据库连接
    Ok(DbState {
        pool: memory_pool,
        count,
        max_length: max_l,
        min_length: min_l,
        uuids: ArcSwap::new(Arc::new(uuid_list)),
    })
}

pub fn build_query_conditions(params: &QueryParams, state: &DbState) -> (String, Vec<String>) {
    let mut conditions = Vec::new();
    let mut query_params: Vec<String> = Vec::new();

    // 构建过滤条件（与之前相同）
    if let Some(categories) = &params.c {
        let categories: Vec<&str> = categories.split(',').collect();
        if !categories.is_empty() {
            let placeholders = vec!["?"; categories.len()].join(",");
            conditions.push(format!("type IN ({})", placeholders));
            for c in categories {
                query_params.push(c.to_string());
            }
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

    let where_clause = if !conditions.is_empty() {
        format!("WHERE {}", conditions.join(" AND "))
    } else {
        "".to_string()
    };

    let rand_function = match state.pool.any_kind() {
        AnyKind::MySql => "RAND()",
        _ => "RANDOM()",
    };

    let sql = format!(
        "SELECT * FROM (
            SELECT * FROM hitokoto
            {where_clause}
        ) AS sampled
        ORDER BY {rand_function}
        LIMIT 1"
    );

    (sql, query_params)
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
    let uuids = state.uuids.load();
    if uuids.len() > 0 {
        // 获取随机索引
        let rand_index = rand::rng().random_range(0..uuids.len());

        // 构造带 WHERE 条件的查询
        let query = "SELECT * FROM hitokoto WHERE uuid = ?";

        // 执行查询
        return execute_query_with_params(state, query, &[&uuids[rand_index]]).await;
    }

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
            // PostgreSQL不支持，但留着不删
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

pub async fn get_length_stats(pool: &AnyPool) -> Result<(i32, i32), sqlx::Error> {
    let (max, min): (i32, i32) = sqlx::query_as(
        r#"
        SELECT MAX(length), MIN(length)
        FROM hitokoto
        "#,
    )
    .fetch_one(pool)
    .await?;

    Ok((max, min))
}
