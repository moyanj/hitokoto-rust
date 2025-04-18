use actix_web::Error;
use serde::{Deserialize, Serialize};
use sqlx::{AnyPool, any::AnyPoolOptions};
use std::{fs, io::Write};

const VERSION_URL: &str =
    "https://github.com/hitokoto-osc/sentences-bundle/raw/refs/heads/master/version.json";
const CACHE_DIR: &str = "./cache";

#[derive(Debug, Serialize, Deserialize)]
struct VersionData {
    updated_at: i32,
    sentences: Vec<CategoryMeta>,
}

#[derive(Debug, Serialize, Deserialize)]
struct CategoryMeta {
    key: String,
    name: String,
    timestamp: i32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Sentence {
    uuid: String,
    hitokoto: String,
    #[serde(rename = "type")]
    sentence_type: String,
    from: String,
    from_who: Option<String>,
    length: i32,
}

#[derive(Debug, Serialize, Deserialize)]
struct CategoryData {
    timestamp: i32,
    sentences: Vec<Sentence>,
}

pub async fn init_db(db_url: &str) -> Result<(), Error> {
    let pool = get_pool(&db_url.to_string()).await.unwrap();

    // 创建缓存目录（处理错误）
    fs::create_dir_all(CACHE_DIR)?;

    let version_data = get_version().await.unwrap();

    let mut total_inserted = 0;

    for category in version_data.sentences {
        println!("\n正在处理分类: {}", category.name);

        let sentences =
            fetch_category_data(&category.key, &category.name, category.timestamp).await?;

        // 批量插入
        let inserted = batch_insert_sentences(&pool, &sentences).await.unwrap();

        total_inserted += inserted;
    }

    // 创建索引
    create_indexes(&pool).await.unwrap();

    println!("\n操作完成，总计处理 {} 条记录", total_inserted);

    Ok(())
}

async fn fetch_category_data(
    key: &String,
    name: &String,
    timestamp: i32,
) -> Result<Vec<Sentence>, Error> {
    let cache_path = std::path::Path::new(CACHE_DIR).join(format!("{}.json", key));

    // 先尝试从缓存加载
    if let Ok(cached_content) = fs::read_to_string(&cache_path) {
        let cached_data: CategoryData = serde_json::from_str(&cached_content)?;

        // 检查缓存是否需要更新
        if timestamp > cached_data.timestamp {
            println!("缓存的 {} 数据是最新的，无需更新", name);
            return Ok(cached_data.sentences);
        }
    }

    let url = format!(
        "https://github.com/hitokoto-osc/sentences-bundle/raw/refs/heads/master/sentences/{}.json",
        key
    );
    let client = reqwest::Client::new();
    let response = client.get(&url).send().await.unwrap();

    let sentences: Vec<Sentence> = response.json().await.unwrap();
    println!("成功下载 {} 数据", name);

    // 保存到缓存
    let cache_data = CategoryData {
        timestamp: timestamp,
        sentences: sentences.clone(),
    };

    let mut file = std::fs::File::create(&cache_path).unwrap();
    let json = serde_json::to_string_pretty(&cache_data)?;
    file.write_all(json.as_bytes()).unwrap();

    Ok(sentences)
}

async fn batch_insert_sentences(
    pool: &AnyPool,
    sentences: &[Sentence],
) -> Result<usize, sqlx::Error> {
    let mut tx = pool.begin().await?;

    for sentence in sentences {
        sqlx::query(
            r#"
            INSERT INTO hitokoto (uuid, text, type, from_source, from_who, length)
            VALUES (?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&sentence.uuid)
        .bind(&sentence.hitokoto)
        .bind(&sentence.sentence_type)
        .bind(&sentence.from)
        .bind(&sentence.from_who)
        .bind(sentence.length)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    println!("成功插入 {} 条记录", sentences.len());

    Ok(sentences.len())
}

async fn create_indexes(pool: &AnyPool) -> Result<(), sqlx::Error> {
    let mut conn = pool.acquire().await?;

    // 创建常用查询字段索引
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_type ON hitokoto (type)")
        .execute(&mut *conn)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_length ON hitokoto (length)")
        .execute(&mut *conn)
        .await?;

    Ok(())
}

async fn get_version() -> Result<VersionData, reqwest::Error> {
    let client = reqwest::Client::new();
    let response = client.get(VERSION_URL).send().await?;

    let version_data = response.json::<VersionData>().await?;
    Ok(version_data)
}

async fn get_pool(db_url: &String) -> Result<AnyPool, sqlx::Error> {
    // 创建数据库连接池
    let pool = AnyPoolOptions::new()
        .max_connections(5)
        .connect(db_url)
        .await?;

    // 创建表结构
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS hitokoto (
            id INT PRIMARY KEY AUTO_INCREMENT,
            uuid VARCHAR(36) UNIQUE NOT NULL,
            text TEXT NOT NULL,
            type VARCHAR(1) NOT NULL,
            from_source TEXT NOT NULL,
            from_who TEXT,
            length INT NOT NULL
        )
        "#,
    )
    .execute(&pool)
    .await?;

    // 清除旧数据
    sqlx::query("TRUNCATE TABLE hitokoto")
        .execute(&pool)
        .await?;

    Ok(pool)
}
