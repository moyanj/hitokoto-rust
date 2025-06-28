// main.rs
use actix_web::{
    App, Either, HttpResponse, HttpServer, Responder, get, http::header::ContentType, web,
};
use clap::Parser;
use sqlx::FromRow;
use std::env;
use std::sync::atomic::Ordering;

mod counter;
mod db;
mod init;
use db::*;

use actix_governor::{Governor, GovernorConfigBuilder};
use actix_web::middleware::Compress;

#[cfg(all(feature = "mimalloc", not(target_env = "msvc")))]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

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
    pub fn to_json(&self) -> String {
        let from_who = match &self.from_who {
            Some(who) => format!("\"{}\"", who),
            None => "null".to_string(),
        };

        format!(
            r#"{{"id":{},"uuid":"{}","text":"{}","type":"{}","from":"{}","from_who":{},"length":{}}}"#,
            self.id,
            self.uuid,
            self.text.replace('"', "\\\""), // 转义双引号
            self.r#type,
            self.from_source.replace('"', "\\\""),
            from_who,
            self.length
        )
    }
}

// 查询参数结构
#[derive(serde::Deserialize)]
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
        long,
        value_name = "MAX_CONNECTIONS",
        default_value_t = 10,
        help = "Sets the maximum number of connections in the database pool",
        env = "HITOKOTO_MAX_CONNECTIONS"
    )]
    max_connections: u32,

    /// Load data into memory SQLite database
    #[arg(
        short,
        long,
        help = "Load data into memory SQLite database",
        env = "HITOKOTO_MEMORY"
    )]
    memory: bool,

    // Use Limiter
    #[arg(long, help = "Use Limiter", env = "HITOKOTO_LIMITER")]
    limiter: bool,

    // Limiter Rate
    #[arg(
        long,
        default_value_t = 100,
        help = "Limiter Rate",
        env = "HITOKOTO_LIMITER_RATE"
    )]
    limiter_rate: u64,

    #[cfg(feature = "init")]
    #[arg(long, help = "Initialize database")]
    init: bool,
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let cli = Cli::parse();

    let host = cli.host;
    let port = cli.port;
    let database_url = cli.database;
    let num_cpus = cli.workers;
    let max_connections = cli.max_connections;
    let memory = cli.memory;
    let use_limiter = cli.limiter;
    let limiter_rate = cli.limiter_rate;

    let bind_addr = format!("{}:{}", host, port);

    println!("Welcome to hitokoto-rust!");
    println!("Version: {}", env!("CARGO_PKG_VERSION"));

    if database_url.starts_with("postgres") {
        println!("Don't use PostgreSQL with hitokoto-rust!");
        return Ok(());
    }

    #[cfg(feature = "init")]
    if cli.init {
        println!("Initializing database...");
        init::init_db(&database_url).await.unwrap();
        println!("Database initialized.");
    }

    // Initialize database connection pool with max connections
    let pool: DbState = get_pool(&database_url, max_connections, 10, 60)
        .await
        .unwrap();

    let pool = if memory {
        println!("Loading data into memory SQLite database...");
        load_data_to_memory(&pool.pool).await.unwrap()
    } else {
        pool
    };

    if use_limiter {
        println!("Using Limiter with rate {} per second", limiter_rate);
    } else {
        println!("Not using Limiter");
    }

    println!("Server running at http://{}", bind_addr);

    let app_factory = move || {
        let app = App::new().app_data(web::Data::new(pool.clone()));

        // Apply limiter if enabled
        let app = if use_limiter {
            let governor_conf = GovernorConfigBuilder::default()
                .requests_per_second(limiter_rate)
                .burst_size((limiter_rate as f32 * 1.8).ceil() as u32)
                .finish()
                .unwrap();
            app.wrap(Governor::new(&governor_conf))
        } else {
            app.wrap(Governor::new(
                &GovernorConfigBuilder::default()
                    .permissive(true)
                    .finish()
                    .unwrap(),
            ))
        };
        let req_stats = counter::RequestStats::new();
        app.wrap(Compress::default())
            .app_data(web::Data::new(req_stats.clone()))
            .wrap(counter::RequestCounterMiddleware::new(req_stats.clone()))
            .wrap(actix_cors::Cors::permissive())
            .route("/stats", web::get().to(counter::get_stats))
            .service(get_hitokoto)
            .service(update_count)
            .service(get_hitokoto_by_uuid)
    };

    // Start server
    HttpServer::new(app_factory)
        .bind(bind_addr)?
        .workers(num_cpus)
        .run()
        .await
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
                Either::Right(
                    HttpResponse::Ok()
                        .content_type(ContentType::json())
                        .body(h.to_json()),
                )
            }
        }
        Ok(None) => Either::Right(HttpResponse::NotFound().body("No hitokoto found")),
        Err(_) => Either::Right(HttpResponse::InternalServerError().body("Internal Server Error")),
    }
}

#[get("/")]
async fn get_hitokoto(data: web::Data<DbState>, params: web::Query<QueryParams>) -> impl Responder {
    let encode = params.encode.clone();
    // 如果没有提供任何参数
    if params.c.is_none() && params.min_length.is_none() && params.max_length.is_none() {
        let hitokoto = rand_hitokoto_without_params(&data).await;
        return Either::Right(make_response(encode, hitokoto));
    }

    // 验证 min_length 和 max_length 的合理性
    if let (Some(min), Some(max)) = (params.min_length, params.max_length) {
        if min < 0 || max < 0 {
            return Either::Left(
                HttpResponse::BadRequest().body("The length parameter cannot be negative."),
            );
        }
        if min > max {
            return Either::Left(
                HttpResponse::BadRequest()
                    .body("The min_length cannot be greater than the max_length."),
            );
        }

        // 检查是否超出数据库中的实际范围
        let db_min = data.min_length.load(Ordering::Relaxed);
        let db_max = data.max_length.load(Ordering::Relaxed);

        if min > db_max || max < db_min {
            return Either::Left(
                HttpResponse::BadRequest()
                    .body("The requested length range exceeds the range of database records"),
            );
        }
    }

    let (query, query_params) = build_query_conditions(&params, data.get_ref());
    let params_slice: Vec<&str> = query_params.iter().map(|s| s.as_str()).collect();
    let hitokoto = execute_query_with_params(&data, &query, &params_slice).await;

    Either::Right(make_response(encode, hitokoto))
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
        Ok(Some(h)) => HttpResponse::Ok()
            .content_type(ContentType::json())
            .body(h.to_json()),
        Ok(None) => HttpResponse::NotFound().body("No hitokoto found with the given uuid"),
        Err(_) => HttpResponse::InternalServerError().body("Internal Server Error"),
    }
}

#[get("/update_count")]
async fn update_count(data: web::Data<DbState>) -> impl Responder {
    data.update().await.unwrap();
    HttpResponse::Ok().body("Count updated")
}
