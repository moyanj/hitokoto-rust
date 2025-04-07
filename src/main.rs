// main.rs
use actix_web::{
    App, Either, HttpResponse, HttpServer, Responder, get, http::header::ContentType, web,
};
use clap::Parser;
use rand::prelude::*;
use serde::Deserialize;
use sqlx::FromRow;
use sqlx::any::AnyKind;
use std::env;
use std::sync::atomic::Ordering;

mod db;
use db::{DbState, get_pool};

#[cfg(all(feature = "mimalloc", not(target_env = "msvc")))]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use simd_json::json;

#[derive(FromRow)]
struct Hitokoto {
    id: i32,
    uuid: String,
    text: String,
    r#type: String,
    from_source: String,
    from_who: Option<String>,
    length: i32,
}

impl Hitokoto {
    fn to_json(&self) -> simd_json::OwnedValue {
        json!({
            "id": self.id,
            "text": self.text,
            "length": self.length,
            "type": self.r#type,
            "from": self.from_source,
            "from_who": self.from_who,
            "uuid": self.uuid,
        })
    }
}

// 查询参数结构
#[derive(Deserialize)]
struct QueryParams {
    c: Option<String>,
    encode: Option<String>,
    min_length: Option<i32>,
    max_length: Option<i32>,
}

#[derive(Parser)]
#[clap(name = "hitokoto-rust", version = env!("CARGO_PKG_VERSION"), about = "A hitokoto server in Rust", long_about = None)]
struct Cli {
    /// Server host address
    #[arg(
        short = 'H',
        long,
        value_name = "HOST",
        default_value = "0.0.0.0",
        help = "Sets the server host address",
        env = "HITOKOTO_HOST"
    )]
    host: String,

    /// Server port
    #[arg(
        short,
        long,
        value_name = "PORT",
        default_value_t = 8080,
        help = "Sets the server port",
        env = "HITOKOTO_PORT"
    )]
    port: u16,

    /// Database connection URL
    #[arg(
        short,
        long,
        value_name = "DATABASE_URL",
        default_value = "mysql://root:yo12345678@localhost/hitokoto",
        help = "Sets the database connection URL",
        env = "HITOKOTO_DB"
    )]
    database: String,

    /// Number of worker threads
    #[arg(
        short,
        long,
        value_name = "WORKERS",
        default_value_t = num_cpus::get(),
        help = "Sets the number of worker threads",
        env = "HITOKOTO_WORKERS"
    )]
    workers: usize,

    /// Maximum number of connections in the database pool
    #[arg(
        short,
        long,
        value_name = "MAX_CONNECTIONS",
        default_value_t = 10,
        help = "Sets the maximum number of connections in the database pool",
        env = "HITOKOTO_MAX_CONNECTIONS"
    )]
    max_connections: u32,
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let cli = Cli::parse();

    let host = cli.host;
    let port = cli.port;
    let database_url = cli.database;
    let num_cpus = cli.workers;
    let max_connections = cli.max_connections; // 获取最大连接数

    let bind_addr = format!("{}:{}", host, port);

    println!("Welcome to hitokoto-rust!");
    println!("Version: {}", env!("CARGO_PKG_VERSION"));

    // 初始化数据库连接池，并设置最大连接数
    let pool: DbState = get_pool(&database_url, max_connections, 10, 60)
        .await
        .unwrap();

    println!("Server running at http://{}", bind_addr);
    // 启动服务器
    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(pool.clone()))
            .service(get_hitokoto)
            .service(update_count)
            .service(get_hitokoto_by_uuid)
    })
    .bind(bind_addr)?
    .workers(num_cpus)
    .run()
    .await
}

fn build_query_conditions(params: &QueryParams, state: &DbState) -> (String, Vec<String>) {
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
    let rand = match state.db_kind {
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

// 新增通用查询执行函数
async fn execute_query_with_params(
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

async fn rand_hitokoto_without_params(state: &DbState) -> Result<Option<Hitokoto>, sqlx::Error> {
    // 生成随机索引
    let rand_index = rand::rng().random_range(0..state.count.load(Ordering::Relaxed));

    // 构造带 OFFSET 的查询
    let query = format!("SELECT * FROM hitokoto LIMIT 1 OFFSET {}", rand_index);

    // 执行查询
    execute_query_with_params(state, &query, &[]).await
}

fn make_response(
    encode: Option<String>,
    hitokoto: Result<Option<Hitokoto>, sqlx::Error>,
) -> impl Responder {
    match hitokoto {
        Ok(Some(h)) => {
            if encode == Some("text".to_string()) {
                Either::Left(
                    HttpResponse::Ok()
                        .content_type(ContentType::plaintext())
                        .body(h.text),
                )
            } else {
                Either::Right(HttpResponse::Ok().json(h.to_json()))
            }
        }
        Ok(None) => Either::Right(HttpResponse::NotFound().body("No hitokoto found")),
        Err(_) => Either::Right(HttpResponse::InternalServerError().body("Internal Server Error")),
    }
}

#[get("/")]
async fn get_hitokoto(data: web::Data<DbState>, params: web::Query<QueryParams>) -> impl Responder {
    let encode = params.encode.clone();
    if params.c.is_none() && params.min_length.is_none() && params.max_length.is_none() {
        let hitokoto = rand_hitokoto_without_params(&data).await;
        return make_response(encode, hitokoto);
    }

    let (query, query_params) = build_query_conditions(&params, data.get_ref());
    let params_slice: Vec<&str> = query_params.iter().map(|s| s.as_str()).collect();
    let hitokoto = execute_query_with_params(&data, &query, &params_slice).await;

    make_response(encode, hitokoto)
}

// 新增路由处理函数修改
#[get("/{uuid}")]
async fn get_hitokoto_by_uuid(data: web::Data<DbState>, uuid: web::Path<String>) -> impl Responder {
    let query = "SELECT * FROM hitokoto WHERE uuid = ? LIMIT 1";

    let hitokoto = execute_query_with_params(&data, query, &[uuid.as_str()])
        .await
        .map_err(|e| {
            eprintln!("Database query error: {}", e);
            HttpResponse::InternalServerError().body("Internal Server Error")
        });

    match hitokoto {
        Ok(Some(h)) => HttpResponse::Ok().json(h.to_json()),
        Ok(None) => HttpResponse::NotFound().body("No hitokoto found with the given uuid"),
        Err(_) => HttpResponse::InternalServerError().body("Internal Server Error"),
    }
}

#[get("/update_count")]
async fn update_count(data: web::Data<DbState>) -> impl Responder {
    let query = "SELECT COUNT(*) FROM hitokoto";
    let count = sqlx::query_scalar::<_, i64>(query)
        .fetch_one(&data.pool)
        .await
        .unwrap();

    data.count.store(count, Ordering::Relaxed);

    HttpResponse::Ok().body("Count updated")
}
