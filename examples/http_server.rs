#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

use core::future::{pending, Future};

use embedded_svc::io::asynch::Write;
use embedded_svc::{
    executor::asynch::Blocker,
    http::server::{
        asynch::{Connection, Handler, Request},
        HandlerResult,
    },
    mutex::{RawMutex, StdRawCondvar, StdRawMutex},
    utils::{
        asynch::executor::embedded::{CondvarWait, EmbeddedBlocker},
        http::server::registration::ChainRoot,
    },
};
use embedded_svc_impl::asynch::{
    http::server::Server,
    stdnal::StdTcpServerSocket,
    tcp::{TcpAcceptor, TcpServerSocket},
};

fn main() {
    simple_logger::SimpleLogger::new().env().init().unwrap();

    let wait = CondvarWait::<StdRawCondvar>::new();

    let blocker = EmbeddedBlocker::new(wait.notify_factory(), wait);

    let mut socket = StdTcpServerSocket::new();

    blocker.block_on(async move {
        let acceptor = socket.bind("0.0.0.0:8080".parse().unwrap()).await.unwrap();

        run::<StdRawMutex, _>(acceptor).await;
    });
}

pub async fn run<R, A>(acceptor: A)
where
    R: RawMutex,
    A: TcpAcceptor,
{
    let handler = ChainRoot
        .get("/", Simple)
        .post("/", Simple2)
        .get("/foo", Simple2);

    let mut server = Server::<1, 1, _, _>::new(acceptor, handler);

    server.process::<1, 1, R, _>(pending()).await.unwrap();
}

pub struct Simple;

impl<C> Handler<C> for Simple
where
    C: Connection,
{
    type HandleFuture<'a>
    = impl Future<Output = HandlerResult>
    where
    Self: 'a,
    C: 'a;

    fn handle<'a>(&'a self, connection: &'a mut C) -> Self::HandleFuture<'a> {
        async move {
            let request = Request::wrap(connection)?;

            request
                .into_ok_response()
                .await?
                .write_all("Hello!".as_bytes())
                .await?;

            Ok(())
        }
    }
}

pub struct Simple2;

impl<C> Handler<C> for Simple2
where
    C: Connection,
{
    type HandleFuture<'a>
    = impl Future<Output = HandlerResult>
    where
    Self: 'a,
    C: 'a;

    fn handle<'a>(&'a self, connection: &'a mut C) -> Self::HandleFuture<'a> {
        async move {
            connection.into_response(200, Some("OK"), &[]).await?;

            Ok(())
        }
    }
}
