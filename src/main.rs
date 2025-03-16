// main.rs
use actix_web::{
    App, Either, HttpResponse, HttpServer, Responder, get, http::header::ContentType, web,
};
use clap::Parser;
use serde::Deserialize;
use sqlx::FromRow;
use sqlx::mysql::MySqlPool; // 添加错误类型导入

#[derive(Debug, FromRow)]
struct Hitokoto {
    id: i32,
    uuid: String,
    text: String,
    r#type: String,
    from_source: String,
    from_who: Option<String>,
    length: i32,
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
#[clap(version = env!("CARGO_PKG_VERSION"), about = "A hitokoto server in Rust")]
struct Cli {
    #[clap(
        short = 'h',
        long = "host",
        value_name = "HOST",
        default_value = "0.0.0.0",
        help = "Sets the server host address"
    )]
    host: String,

    #[clap(
        short = 'p',
        long = "port",
        value_name = "PORT",
        default_value_t = 8080,
        help = "Sets the server port"
    )]
    port: u16,

    #[clap(
        short = 'd',
        long = "database",
        value_name = "DATABASE",
        default_value = "mysql://root:yo12345678@localhost/hitokoto",
        help = "Sets the MySQL database connection URL"
    )]
    database: String,

    #[clap(short = 'w', long = "workers", value_name = "WORKERS", default_value_t = num_cpus::get(), help = "Sets the number of worker threads")]
    workers: usize,

    #[clap(
        short = 'm',
        long = "max-connections",
        value_name = "MAX_CONNECTIONS",
        default_value_t = 10,
        help = "Sets the maximum number of connections in the database pool"
    )]
    max_connections: u32, // 添加最大连接数参数
}

// 应用状态（数据库连接池）
struct AppState {
    db: MySqlPool,
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
    println!("Server running at http://{}", bind_addr);

    // 初始化数据库连接池，并设置最大连接数
    let pool = sqlx::mysql::MySqlPoolOptions::new()
        .max_connections(max_connections)
        .connect(&database_url)
        .await
        .expect("Failed to connect to database");

    // 启动服务器
    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(AppState { db: pool.clone() }))
            .service(get_hitokoto)
            .service(get_hitokoto_by_uuid)
    })
    .bind(bind_addr)?
    .workers(num_cpus)
    .run()
    .await
}

// 主处理函数
#[get("/")]
async fn get_hitokoto(
    data: web::Data<AppState>,
    params: web::Query<QueryParams>,
) -> impl Responder {
    let mut conditions = vec!["1=1".to_string()];
    let mut query_params: Vec<String> = vec![];

    // 构建查询条件
    if let Some(categories) = &params.c {
        println!("categories: {:?}", categories);
        let categories: Vec<&str> = categories.split(',').collect(); // 将字符串按逗号分隔为字符串向量
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

    // 执行查询
    let query = format!(
        "SELECT * FROM hitokoto WHERE {} ORDER BY RAND() LIMIT 1",
        conditions.join(" AND ")
    );

    let mut query_builder = sqlx::query_as::<_, Hitokoto>(&query);
    for param in query_params {
        query_builder = query_builder.bind(param);
    }

    let hitokoto = query_builder.fetch_optional(&data.db).await.map_err(|e| {
        eprintln!("Database query error: {}", e);
        HttpResponse::InternalServerError().body("Internal Server Error")
    });

    match hitokoto {
        Ok(Some(h)) => match params.encode.as_deref() {
            Some("text") => Either::Left(
                HttpResponse::Ok()
                    .content_type(ContentType::plaintext())
                    .body(h.text),
            ),
            _ => Either::Right(HttpResponse::Ok().json(serde_json::json!({
                "id": h.id,
                "text": h.text,
                "length": h.length,
                "type": h.r#type,
                "from": h.from_source,
                "from_who": h.from_who,
                "uuid": h.uuid,
            }))),
        },
        Ok(None) => Either::Right(HttpResponse::NotFound().body("No hitokoto found")),
        Err(_) => Either::Right(HttpResponse::InternalServerError().body("Internal Server Error")),
    }
}

// 新增路由处理函数，根据uuid查询Hitokoto
#[get("/{uuid}")]
async fn get_hitokoto_by_uuid(
    data: web::Data<AppState>,
    uuid: web::Path<String>,
) -> impl Responder {
    let query = "SELECT * FROM hitokoto WHERE uuid = ? LIMIT 1";

    let hitokoto = sqlx::query_as::<_, Hitokoto>(query)
        .bind(uuid.as_str())
        .fetch_optional(&data.db)
        .await
        .map_err(|e| {
            eprintln!("Database query error: {}", e);
            HttpResponse::InternalServerError().body("Internal Server Error")
        });

    match hitokoto {
        Ok(Some(h)) => HttpResponse::Ok().json(serde_json::json!({
            "id": h.id,
            "text": h.text,
            "length": h.length,
            "type": h.r#type,
            "from": h.from_source,
            "from_who": h.from_who,
            "uuid": h.uuid,
        })),
        Ok(None) => HttpResponse::NotFound().body("No hitokoto found with the given uuid"),
        Err(_) => HttpResponse::InternalServerError().body("Internal Server Error"),
    }
}
