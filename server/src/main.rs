use hyper::service::{make_service_fn, service_fn};
use hyper::Server;
use std::convert::Infallible;
use std::net::SocketAddr;

mod assets;
mod router;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[tokio::main]
async fn main() {
    let port = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3000);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    let make_svc = make_service_fn(|_conn| async {
        Ok::<_, Infallible>(service_fn(router::route))
    });

    let server = Server::bind(&addr)
        .serve(make_svc);

    println!("Static server running on http://{}", addr);
    println!("Serving {} routes", router::route_count());

    if let Err(e) = server.await {
        eprintln!("Server error: {}", e);
        std::process::exit(1);
    }
}
