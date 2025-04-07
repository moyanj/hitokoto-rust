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
