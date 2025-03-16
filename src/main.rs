// main.rs
use actix_web::{
    App, Either, HttpResponse, HttpServer, Responder, get, http::header::ContentType, web,
};
use rusqlite::Connection;
use serde::Deserialize;
use std::sync::{Arc, Mutex};

// 数据库模型
#[derive(Debug)]
struct Hitokoto {
    id: i32,
    uuid: String,
    text: String,
    r#type: String,
    from: String,
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

// 应用状态（数据库连接池）
struct AppState {
    db: Arc<Mutex<Connection>>,
}

// 主处理函数
#[get("/")]
async fn get_hitokoto(
    data: web::Data<AppState>,
    params: web::Query<QueryParams>,
) -> impl Responder {
    let mut conditions = vec!["1=1".to_string()];
    let mut query_params: Vec<Box<dyn rusqlite::ToSql>> = vec![];

    // 构建查询条件
    if let Some(categories) = &params.c {
        println!("categories: {:?}", categories);
        let categories: Vec<&str> = categories.split(',').collect(); // 将字符串按逗号分隔为字符串向量
        if !categories.is_empty() {
            conditions.push(format!(
                "type IN ({})",
                categories.iter().map(|_| "?").collect::<Vec<_>>().join(",")
            ));
            query_params.extend(categories.iter().map(|c| Box::new(c.to_string()) as _));
        }
    }

    if let Some(min) = params.min_length {
        conditions.push("length >= ?".to_string());
        query_params.push(Box::new(min));
    }

    if let Some(max) = params.max_length {
        conditions.push("length <= ?".to_string());
        query_params.push(Box::new(max));
    }

    // 执行查询
    let query = format!(
        "SELECT * FROM hitokoto WHERE {} ORDER BY RANDOM() LIMIT 1",
        conditions.join(" AND ")
    );

    let hitokoto = {
        let conn = data.db.lock().unwrap();
        let mut stmt = conn.prepare(&query).unwrap();
        stmt.query_row(rusqlite::params_from_iter(query_params), |row| {
            Ok(Hitokoto {
                id: row.get(0)?,
                uuid: row.get(1)?,
                text: row.get(2)?,
                r#type: row.get(3)?,
                from: row.get(4)?,
                from_who: row.get(5)?,
                length: row.get(6)?,
            })
        })
        .ok()
    };

    match hitokoto {
        Some(h) => match params.encode.as_deref() {
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
                "from": h.from,
                "from_who": h.from_who,
                "uuid": h.uuid,
            }))),
        },
        None => Either::Right(HttpResponse::NotFound().body("No hitokoto found")),
    }
}

// 新增路由处理函数，根据uuid查询Hitokoto
#[get("/{uuid}")]
async fn get_hitokoto_by_uuid(
    data: web::Data<AppState>,
    uuid: web::Path<String>,
) -> impl Responder {
    let query = "SELECT * FROM hitokoto WHERE uuid = ? LIMIT 1";

    let hitokoto = {
        let conn = data.db.lock().unwrap();
        let mut stmt = conn.prepare(query).unwrap();
        stmt.query_row(rusqlite::params![uuid.as_str()], |row| {
            Ok(Hitokoto {
                id: row.get(0)?,
                uuid: row.get(1)?,
                text: row.get(2)?,
                r#type: row.get(3)?,
                from: row.get(4)?,
                from_who: row.get(5)?,
                length: row.get(6)?,
            })
        })
        .ok()
    };

    match hitokoto {
        Some(h) => HttpResponse::Ok().json(serde_json::json!({
            "id": h.id,
            "text": h.text,
            "length": h.length,
            "type": h.r#type,
            "from": h.from,
            "from_who": h.from_who,
            "uuid": h.uuid,
        })),
        None => HttpResponse::NotFound().body("No hitokoto found with the given uuid"),
    }
}

use clap::Parser;

#[derive(Parser)]
#[clap(version = env!("CARGO_PKG_VERSION"), about = "A simple hitokoto server in Rust")]
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
        default_value = "hitokoto.db",
        help = "Sets the path to the SQLite database file"
    )]
    database: String,

    #[clap(short = 'w', long = "workers", value_name = "WORKERS", default_value_t = num_cpus::get(), help = "Sets the number of worker threads")]
    workers: usize,
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let cli = Cli::parse();

    let host = cli.host;
    let port = cli.port;
    let database_path = cli.database;
    let num_cpus = cli.workers;

    let bind_addr = format!("{}:{}", host, port);

    println!("Welcome to hitokoto-rust!");
    println!("Version: {}", env!("CARGO_PKG_VERSION"));
    println!("Server running at http://{}", bind_addr);

    // 初始化数据库连接
    let conn = Arc::new(Mutex::new(
        Connection::open(database_path).expect("Failed to open database"),
    ));

    // 启动服务器
    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(AppState {
                db: Arc::clone(&conn),
            }))
            .service(get_hitokoto)
            .service(get_hitokoto_by_uuid) // 添加新的路由服务
    })
    .bind(bind_addr)?
    .workers(num_cpus)
    .run()
    .await
}
