use std::error::Error;
use std::net::SocketAddr;
use std::time::Duration;
use futures_util::future::{join};
use quinn::{Chunk, ConnectionError,  ServerConfig};
use crate::common::{configure_client, configure_server};
use futures_util::{ TryStreamExt};
use tokio::{task};
use crate::connection::{QPNewConnection};
use crate::delivery_guarantees::{DeliveryGuarantee, OrderingGuarantee};
use crate::endpoint::{QPEndpoint};
use crate::streams::QPRecvStream;

mod delivery_guarantees;
mod common;
mod endpoint;
mod streams;
mod connection;


static SERVER_NAME: &str = "localhost";

fn client_addr() -> SocketAddr {
    "127.0.0.1:5000".parse::<SocketAddr>().unwrap()
}

fn server_addr() -> SocketAddr {
    "127.0.0.1:5001".parse::<SocketAddr>().unwrap()
}

async fn server() -> Result<(), Box<dyn Error>> {
    let (server, cert) = configure_server().unwrap();

    // Bind this endpoint to a UDP socket on the given server address.
    let (endpoint, mut incoming) = QPEndpoint::server(
        server,
        server_addr()
    )?;

    // Start iterating over incoming connections.
    while let Some(conn) = incoming.next().await {
        println!("Server: Connection Incoming");
        let mut connection: QPNewConnection = conn.await?;
        println!("Server: Connection Accepted");

        //let mut uni = connection.reliable().await;
        let mut uni = connection.reliable_sequenced();
        //let mut uni = connection.reliable_unordered();

        //let mut uni = connection.connection.open_uni().await.unwrap();

        for x in 0..100 {
            let result = uni.write_chunk(&format!("Test {}", x).into_bytes()).await;
            tokio::time::sleep(Duration::from_millis(10)).await;
        }

       // uni.refresh();

    }

    Ok(())
}


async fn client() -> Result<(), Box<dyn Error>> {
    // Bind this endpoint to a UDP socket on the given client address.
    let mut endpoint = QPEndpoint::client(client_addr()).unwrap();
    endpoint.set_default_client_config(configure_client());

    println!("Client: Connecting");

    // Connect to the server passing in the server name which is supposed to be in the server certificate.
    let mut connection = endpoint.connect(server_addr(), SERVER_NAME)?.await?;

    println!("Client: Connected");

    loop {
        connection.poll().await;

        if let Ok(packet) = connection.next_event() {
           println!()
        }

        // if let Ok(Some(bi_stream)) = connection.uni_streams.try_next().await {
        //     task::spawn(handle_uni_stream(GameRecvStream(bi_stream)));
        // }

        // if let Ok(Some(uni_stream)) = connection.uni_streams.try_next().await {
        //     println!("bi_stream");
        //     task::spawn(handle_uni_stream(GameRecvStream(uni_stream)));
        // }

    }
}

async fn handle_uni_stream(mut stream: QPRecvStream) {
    loop {
        match  stream.read_chunk().await {
            Ok(Some(chunk)) => { println!("Client: Received: {:?} {}", chunk.header, String::from_utf8(chunk.payload.to_vec()).unwrap())}
            Ok(None) => {println!("Stream finished")}
            Err(e) => {
                //println!("{:?}", e)
            }
        }
    }
}

#[tokio::main]
async fn main() {
    let server = tokio::spawn(async move {
        let mut result = server().await.unwrap();
    });

    let client = tokio::spawn(async move {
        let mut result = client().await.unwrap();
    });

    join(server, client).await;
}
