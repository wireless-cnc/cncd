use actix_web::{middleware, web, App, HttpServer};
use libmdns::Responder;
use tokio::runtime::Handle;
use std::process;


mod comm_async;
mod websocket;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));
    let tty = std::env::args().nth(1).expect("no tty path given");
    let app_data = web::Data::new(tty.clone());
    let port: u16 = std::env::args()
        .nth(2)
        .expect("no port given")
        .parse()
        .expect("failed to parse port number");
    log::info!("starting HTTP server at 0.0.0.0:{}", port);
    let handle = Handle::current();
    let responder = Responder::spawn(&handle)?;
    let service_name = format!("CNCD {}", process::id());
    let _cncd_service = responder.register(
        "_http._tcp".into(),
        service_name,
        port,
        &["path=/", "type=cnc", "name=Punky"],
    );
    HttpServer::new(move || {
        // keep scope
        App::new()
            .app_data(app_data.clone())
            .service(web::resource("/ws").route(web::get().to(websocket::handler)))
            .wrap(middleware::Logger::new("%a %r %s %b %T"))
    })
    .bind(("0.0.0.0", port))?
    .run()
    .await
}
