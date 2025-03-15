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
    c: Option<Vec<String>>,
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
        conditions.push(format!(
            "type IN ({})",
            categories.iter().map(|_| "?").collect::<Vec<_>>().join(",")
        ));
        query_params.extend(categories.iter().map(|c| Box::new(c) as _));
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
    let query = format!("SELECT * FROM hitokoto WHERE {}", conditions.join(" AND "));

    let hitokotos = {
        let conn = data.db.lock().unwrap();
        let mut stmt = conn.prepare(&query).unwrap();
        let hitokotos_iter = stmt
            .query_map(rusqlite::params_from_iter(query_params), |row| {
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
            .unwrap();

        let mut hitokotos: Vec<Hitokoto> = hitokotos_iter.collect::<Result<Vec<_>, _>>().unwrap();
        use rand::seq::SliceRandom;
        let mut rng = rand::rng();
        hitokotos.shuffle(&mut rng);
        if !hitokotos.is_empty() {
            Some(hitokotos.remove(0))
        } else {
            None
        }
    };

    match hitokotos {
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

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // 初始化数据库连接
    let conn = Arc::new(Mutex::new(
        Connection::open("hitokoto.db").expect("Failed to open database"),
    ));

    // 创建数据表（示例用）
    conn.lock()
        .unwrap()
        .execute(
            "CREATE TABLE IF NOT EXISTS hitokoto (
            id INTEGER PRIMARY KEY,
            text TEXT NOT NULL,
            length INTEGER,
            type TEXT
        )",
            [],
        )
        .unwrap();

    // 启动服务器
    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(AppState {
                db: Arc::clone(&conn),
            }))
            .service(get_hitokoto)
    })
    .bind("0.0.0.0:8000")?
    .run()
    .await
}
