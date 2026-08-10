use actix_web::{get, App, HttpRequest, HttpResponse, HttpServer};

#[get("/{tail:.*}")]
async fn echo(req: HttpRequest) -> HttpResponse {
    let body = format!(
        "OK {} {} {}\n",
        req.method(),
        req.path(),
        req.headers()
            .get("host")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("-"),
    );
    HttpResponse::Ok()
        .content_type("text/plain")
        .body(body)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(8080);

    HttpServer::new(|| App::new().service(echo))
        .bind(("0.0.0.0", port))?
        .run()
        .await
}