#[macro_use]
extern crate futures;

use bytes::BytesMut;
use futures::future::FutureResult;
use futures::future::JoinAll;
use futures::{Async, Future, IntoFuture, Poll};
use std::io::{self, repeat, Read, Write};
use std::iter::repeat_with;
use std::marker::PhantomData;
use std::net::SocketAddr;
use tokio::codec::BytesCodec;
use tokio::net::{TcpListener, TcpStream};
use tokio::prelude::AsyncRead;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tower::{ServiceBuilder, ServiceExt};
use tower_layer::Layer;
use tower_service::Service;

struct Message<TFuture> {
    resp_tx: oneshot::Sender<TFuture>,
}

struct Worker<S, TRequest>
where
    S: Service<TRequest>,
{
    service: S,
    rx: mpsc::Receiver<Message<S::Future>>,
}

struct RepeatService<S> {
    n: usize,
    service: S,
}

impl<S, TReq> Service<TReq> for RepeatService<S>
where
    S: Service<TReq>,
    TReq: Clone,
{
    type Response = Vec<S::Response>;
    type Error = S::Error;
    type Future = JoinAll<Vec<S::Future>>;

    fn poll_ready(&mut self) -> Poll<(), Self::Error> {
        self.service.poll_ready()
    }

    fn call(&mut self, req: TReq) -> Self::Future {
        let n = self.n;
        futures::future::join_all(
            repeat_with(|| self.service.call(req.clone()))
                .take(n)
                .collect::<Vec<_>>(),
        )
    }
}

struct RepeatLayer<S> {
    n: usize,
    _s: PhantomData<S>,
}

impl<S> RepeatLayer<S> {
    fn new(n: usize) -> Self {
        Self { n, _s: PhantomData }
    }
}

impl<S> Layer<S> for RepeatLayer<S> {
    type Service = RepeatService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        RepeatService {
            n: self.n,
            service: inner,
        }
    }
}

fn main() {
    let mut connect_svc = tower_util::service_fn(|&addr| {
        TcpStream::connect(&addr).and_then(|mut conn| {
            conn.write(b"HELLO");
            futures::future::ok(())
        })
    });

    // Flood with 4000 connections
    let mut flood_svc = ServiceBuilder::new()
        .layer(RepeatLayer::new(4000))
        .service(connect_svc);

    // Call the flood service
    let addr = "127.0.0.1:9999".parse().unwrap();
    let flooder = flood_svc
        .call(&addr)
        .and_then(|res| {
            println!("Made {} connections", res.len());
            Ok(())
        })
        .map_err(|err| ());

    tokio::run(flooder);
}
