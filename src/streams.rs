use std::future::Future;
use std::ops::{Deref, DerefMut};
use std::pin::Pin;
use std::sync::mpsc::RecvError;
use std::task::{Context, Poll};
use bytes::{Buf, BufMut, Bytes, BytesMut};
use futures_util::FutureExt;
use quinn::{Connection, ReadError, SendStream, WriteError};
use crate::connection::QPConnection;
use crate::{Chunk, DeliveryGuarantee, OrderingGuarantee};

pub struct QPWriteChunk<'a>(quinn::WriteChunk<'a>);

impl<'a> Future for QPWriteChunk<'a> {
    type Output = Result<(), WriteError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.0).poll(cx)
    }
}

pub struct QPReadChunk<'a>(quinn::ReadChunk<'a>);

impl<'a> Future for QPReadChunk<'a> {
    type Output = Result<Option<Packet>, ReadError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match Pin::new(&mut self.0).poll(cx) {
            Poll::Ready(Ok(Some(mut chunk))) => {
                let packet = Packet::deserialize(&mut chunk.bytes);
                return Poll::Ready(Ok(Some(packet)))
            }
            Poll::Ready(Ok(None)) => Poll::Ready(Ok(None)),
            Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
            Poll::Pending => Poll::Pending,
        }
    }
}

pub struct QPSendStream(pub(crate) quinn::SendStream);

impl QPSendStream {
    pub fn write_chunk<'a>(&mut self, ordering: OrderingGuarantee, delivery: DeliveryGuarantee, buf: &'a [u8]) -> QPWriteChunk<'_> {
        let bytes =  Packet::serialize(buf,
           Header {
               ordering_guarantee: ordering,
               delivery_guarantee: delivery
           }
        );

        QPWriteChunk(self.0.write_chunk(bytes))
    }
}

pub struct QPRecvStream(pub(crate) quinn::RecvStream, pub(crate) bool);

impl QPRecvStream {
    pub fn read_chunk<'a>(&mut self) -> QPReadChunk<'_> {
        return QPReadChunk(self.0.read_chunk(1300, self.1))
    }
}

impl Deref for QPSendStream {
    type Target = quinn::SendStream;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for QPSendStream {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Deref for QPRecvStream {
    type Target = quinn::RecvStream;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for QPRecvStream {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

pub struct Reliable {
    pub send_stream: QPSendStream
}

impl Reliable {
    pub async fn write_chunk<'a>(&'a mut self, buf: &'a [u8]) {
        let a = self.send_stream.write_chunk(OrderingGuarantee::Ordered(None), DeliveryGuarantee::Reliable, buf).await;
        println!("Client: write result: {:?}", a);
    }
}

impl Deref for Reliable {
    type Target = QPSendStream;

    fn deref(&self) -> &Self::Target {
        &self.send_stream
    }
}

impl DerefMut for Reliable {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.send_stream
    }
}

pub struct ReliableUnordered {
    pub connection: QPConnection,
    pub streams: Vec<QPSendStream>
}

impl ReliableUnordered {
    pub async fn write_chunk<'a>(&'a mut self, buf: &'a [u8]) {
        let mut stream = self.connection.open_uni().await.unwrap();
        let a = stream.write_chunk(OrderingGuarantee::None, DeliveryGuarantee::Reliable, buf).await;
        self.streams.push(stream);
        println!("Client: write result: {:?}", a);
    }

    pub async fn refresh(&mut self) {
        for stream in &mut self.streams {
            stream.0.finish().await;
        }

        self.streams.clear();
    }
}

pub struct ReliableSequenced {
    pub connection: QPConnection,
    pub streams: Vec<QPSendStream>
}

impl ReliableSequenced {
    pub async fn write_chunk<'a>(&'a mut self, buf: &'a [u8]) {
        let mut stream = self.connection.open_uni().await.unwrap();

        let a = stream.write_chunk(OrderingGuarantee::Sequenced(None), DeliveryGuarantee::Reliable, buf).await;
        self.streams.push(stream);
        println!("Client: write result: {:?}", a);
    }

    pub async fn refresh(&mut self) {
        for stream in &mut self.streams {
            stream.0.finish().await;
        }

        self.streams.clear();
    }
}

pub struct Packet{pub header: Header, pub payload: Bytes}

impl Packet {
    pub fn serialize(buf: &[u8], header: Header) -> Bytes {
        let mut bytes = BytesMut::new();
        header.serialize(&mut bytes);
        bytes.put_slice(buf);

        bytes.freeze()
    }

    pub fn deserialize(buffer: &mut Bytes) -> Packet {
        let header = Header::deserialize(buffer);
        let payload = Bytes::copy_from_slice(&buffer[2..]);

        Packet {
            header,
            payload
        }
    }
}

#[derive(Copy, Clone, Debug, PartialOrd, PartialEq, Eq)]
pub struct Header {
    pub delivery_guarantee: DeliveryGuarantee,
    pub ordering_guarantee: OrderingGuarantee
}

impl Header {
    pub fn new(delivery_guarantee: DeliveryGuarantee, ordering_guarantee: OrderingGuarantee) -> Header {
        Header {
            delivery_guarantee,
            ordering_guarantee
        }
    }

    pub fn deserialize(buffer: &mut Bytes) -> Header {

        let delivery_guarantee = buffer.get_u8();
        let ordering_guarantee = buffer.get_u8();


        let delivery_guarantee = match delivery_guarantee {
            0 => DeliveryGuarantee::Unreliable,
            1 => DeliveryGuarantee::Reliable,
            _ => panic!("not found")
        };

        let ordering_guarantee = match ordering_guarantee {
            0 => OrderingGuarantee::None,
            1 => OrderingGuarantee::Sequenced(None),
            2 => OrderingGuarantee::Ordered(None),
            _ => panic!("not found")
        };

        Header {
            delivery_guarantee,
            ordering_guarantee
        }
    }

    pub fn serialize(&self, bytes: &mut BytesMut)  {
        match &self.delivery_guarantee {
            DeliveryGuarantee::Unreliable => {
                bytes.put_u8(0);
            }
            DeliveryGuarantee::Reliable => {
                bytes.put_u8(1);
            }
        }

        match &self.ordering_guarantee {
            OrderingGuarantee::None => {
                bytes.put_u8(0);
            }
            OrderingGuarantee::Sequenced(_) => {
                bytes.put_u8(1);
            }
            OrderingGuarantee::Ordered(_) => {
                bytes.put_u8(2);
            }
        }
    }
}
