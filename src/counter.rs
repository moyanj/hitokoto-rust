use std::sync::Arc;
use std::time::{Duration, Instant};

use actix_web::{
    Error, HttpResponse, Responder,
    dev::{Service, ServiceRequest, ServiceResponse, Transform},
    http::header::ContentType,
    web,
};
use futures_util::future::{LocalBoxFuture, Ready, ready};
use parking_lot::Mutex;
use std::collections::VecDeque;

// 请求统计数据结构
#[derive(Debug, Clone)]
pub struct RequestStats {
    per_minute: Arc<Mutex<SlidingWindowCounter>>,
    per_hour: Arc<Mutex<SlidingWindowCounter>>,
    per_day: Arc<Mutex<SlidingWindowCounter>>,
}

impl RequestStats {
    pub fn new() -> Self {
        Self {
            per_minute: Arc::new(Mutex::new(SlidingWindowCounter::new(Duration::from_secs(
                60,
            )))),
            per_hour: Arc::new(Mutex::new(SlidingWindowCounter::new(Duration::from_secs(
                3600,
            )))),
            per_day: Arc::new(Mutex::new(SlidingWindowCounter::new(Duration::from_secs(
                86400,
            )))),
        }
    }

    pub fn requests_per_minute(&self) -> u64 {
        self.per_minute.lock().count()
    }

    pub fn requests_per_hour(&self) -> u64 {
        self.per_hour.lock().count()
    }

    pub fn requests_per_day(&self) -> u64 {
        self.per_day.lock().count()
    }

    pub fn increment(&self) {
        let now = Instant::now();
        self.per_minute.lock().increment(now);
        self.per_hour.lock().increment(now);
        self.per_day.lock().increment(now);
    }
}

// 滑动窗口计数器
#[derive(Debug)]
struct SlidingWindowCounter {
    window: Duration,
    requests: VecDeque<Instant>,
}

impl SlidingWindowCounter {
    fn new(window: Duration) -> Self {
        Self {
            window,
            requests: VecDeque::new(),
        }
    }

    fn increment(&mut self, now: Instant) {
        self.cleanup(now);
        self.requests.push_back(now);
    }

    fn count(&mut self) -> u64 {
        let now = Instant::now();
        self.cleanup(now);
        self.requests.len() as u64
    }

    fn cleanup(&mut self, now: Instant) {
        while let Some(oldest) = self.requests.front() {
            if now.duration_since(*oldest) > self.window {
                self.requests.pop_front();
            } else {
                break;
            }
        }
    }
}

// 中间件
#[derive(Clone)]
pub struct RequestCounterMiddleware {
    stats: RequestStats,
}

impl RequestCounterMiddleware {
    pub fn new(stats: RequestStats) -> Self {
        Self { stats }
    }
}

impl<S, B> Transform<S, ServiceRequest> for RequestCounterMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Transform = RequestCounterMiddlewareService<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(RequestCounterMiddlewareService {
            service,
            stats: self.stats.clone(),
        }))
    }
}

pub struct RequestCounterMiddlewareService<S> {
    service: S,
    stats: RequestStats,
}

impl<S, B> Service<ServiceRequest> for RequestCounterMiddlewareService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    actix_web::dev::forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        // 在请求处理前增加计数器
        self.stats.increment();

        let fut = self.service.call(req);

        Box::pin(async move {
            let res = fut.await?;
            Ok(res)
        })
    }
}

// 用于获取统计数据的handler
pub async fn get_stats(stats: web::Data<RequestStats>) -> impl Responder {
    HttpResponse::Ok()
        .content_type(ContentType::json())
        .body(format!(
            r#"{{"requests_per_minute":{},"requests_per_hour":{},"requests_per_day":{}}}"#,
            stats.requests_per_minute(),
            stats.requests_per_hour(),
            stats.requests_per_day()
        ))
}
