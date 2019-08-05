#[macro_use]
extern crate futures;

use bytes::BytesMut;
use futures::Future;
use futures::{Async, Poll, Stream};
use std::io;
use tokio::codec::BytesCodec;
use tokio::net::{TcpListener, TcpStream};
use tokio::prelude::AsyncRead;

struct Handler {
    socket: TcpStream,
    read_buf: BytesMut,
}

impl Handler {
    pub fn new(socket: TcpStream) -> Self {
        Self {
            socket,
            read_buf: BytesMut::new(),
        }
    }

    fn fill_read_buf(&mut self) -> Poll<(), io::Error> {
        loop {
            self.read_buf.reserve(1024);
            let n = try_ready!(self.socket.read_buf(&mut self.read_buf));

            if n == 0 {
                return Ok(Async::Ready(()));
            }
        }
    }
}

impl Stream for Handler {
    type Item = BytesMut;
    type Error = std::io::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        let is_closed = self.fill_read_buf()?.is_ready();
        let bytes = self.read_buf.take();

        if bytes.len() > 0 {
            return Ok(Async::Ready(Some(bytes)));
        }

        if is_closed {
            println!("DISCONNECTED");
            Ok(Async::Ready(None))
        } else {
            Ok(Async::NotReady)
        }
    }
}

fn handle(socket: TcpStream) {
    let mut handler = Handler::new(socket);

    let fut = handler
        .for_each(|bytes| {
            println!("GET BYTES? {:x?}", bytes);
            Ok(())
        })
        .map_err(|err| {
            println!("GET ERROR {:?}", err);
            ()
        });

    tokio::spawn(fut);
}

fn main() {
    let addr = "127.0.0.1:9999".parse().unwrap();
    let server = TcpListener::bind(&addr)
        .unwrap()
        .incoming()
        .for_each(move |socket| {
            handle(socket);
            Ok(())
        })
        .map_err(|err| {
            println!("{:?}", err);
        });

    println!("Server listening in {}", addr);
    tokio::run(server);
}
