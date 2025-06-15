use core::net::SocketAddr;

use std::convert::Infallible;

use std::io;

use hyper::{server::conn::http1, service::service_fn};
use hyper_util::rt::TokioIo;

use tokio::net::TcpListener;

use sqlx::postgres::PgPool;

use dav_server::DavHandler;

use rs_pgdbsize2webdav::fake_lock_system;

use rs_pgdbsize2webdav::pool2dbsize_fs;

#[tokio::main]
async fn main() -> Result<(), io::Error> {
    let conn: String = std::env::var("ENV_PG_CONN_STR")
        .map_err(|_| io::Error::other("connection string ENV_PG_CONN_STR missing"))?;

    let port: u16 = std::env::var("ENV_WD_LISTEN_PORT")
        .map_err(|_| io::Error::other("listen port ENV_WD_LISTEN_PORT missing"))
        .and_then(|s| str::parse(&s).map_err(io::Error::other))?;

    let poo: PgPool = PgPool::connect(&conn).await.map_err(io::Error::other)?;

    let dfs = pool2dbsize_fs(poo);

    let lsys = fake_lock_system();

    let dav_server = DavHandler::builder()
        .filesystem(Box::new(dfs))
        .locksystem(Box::new(lsys))
        .build_handler();

    let addr: SocketAddr = ([127, 0, 0, 1], port).into();

    let listener = TcpListener::bind(addr).await?;

    loop {
        let (stream, _) = listener.accept().await?;
        let ds = dav_server.clone();
        let tio = TokioIo::new(stream);

        tokio::task::spawn(async move {
            let r: Result<_, _> = http1::Builder::new()
                .serve_connection(
                    tio,
                    service_fn({
                        move |req| {
                            let d = ds.clone();
                            async move { Ok::<_, Infallible>(d.handle(req).await) }
                        }
                    }),
                )
                .await;
            match r {
                Ok(_) => {}
                Err(e) => eprintln!("{e}"),
            }
        });
    }
}
