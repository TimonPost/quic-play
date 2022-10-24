use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::ops::{Deref, DerefMut};
use std::pin::Pin;
use std::sync::mpsc::{Receiver, RecvError, Sender, TryRecvError};
use std::task::{Context, Poll};
use futures_util::TryStreamExt;
use quinn::{RecvStream, StreamId};
use crate::{ConnectionError, DeliveryGuarantee, QPRecvStream, OrderingGuarantee};
use crate::streams::{QPSendStream, Packet, Reliable, ReliableSequenced, ReliableUnordered};

#[derive(Clone)]
pub struct QPConnection(pub(crate) quinn::Connection);

impl QPConnection {
    pub fn open_uni(&self) -> GameOpenUni {
        GameOpenUni(self.0.open_uni())
    }
}

pub struct QPNewConnection {
    pub(crate) new_connection: quinn::NewConnection,
    pub(crate) game_connection: QPConnection,
    events_rx: Receiver<GameEvent>,
    events_tx: Sender<GameEvent>,
    reliable_streams: HashMap<StreamId, QPRecvStream>,
}

impl QPNewConnection {
    pub (crate) fn new(new_connection: quinn::NewConnection, game_connection: QPConnection) -> Self {
        let (events_tx, events_rx) = std::sync::mpsc::channel();

        Self {
            new_connection,
            game_connection,
            events_rx,
            events_tx,
            reliable_streams: HashMap::new(),
        }
    }


    pub fn reliable_unordered(&self) -> ReliableUnordered {
        unsafe { ReliableUnordered { connection: self.game_connection.clone(),streams: Vec::new()} }
    }

    pub fn reliable_sequenced(&self) -> ReliableSequenced {
        unsafe { ReliableSequenced { connection: self.game_connection.clone(),streams: Vec::new()} }
    }

    pub async fn reliable(&self) -> Reliable {
        let stream = self.game_connection.open_uni().await.unwrap();
        Reliable { send_stream: stream}
    }

    pub async fn  poll(&mut self) {
        while let Ok(Some(stream)) = self.new_connection.uni_streams.try_next().await  {            
            let mut stream = QPRecvStream(stream, true);

            if let Ok(Some(packet)) = stream.read_chunk().await {
                match (packet.header.delivery_guarantee, packet.header.ordering_guarantee) {
                    (DeliveryGuarantee::Reliable, OrderingGuarantee::Ordered(None)) => {
                        self.events_tx.send(GameEvent::Packet(packet));
                        self.reliable_streams.insert(stream.id(), stream);
                    },
                    (DeliveryGuarantee::Reliable, OrderingGuarantee::None) => {
                        self.events_tx.send(GameEvent::Packet(packet));
                    },
                    (DeliveryGuarantee::Reliable, OrderingGuarantee::Sequenced(None)) => {
                        self.events_tx.send(GameEvent::Packet(packet));
                    }
                    _ => {},
                }
            }
        }

        for (id, mut stream) in &mut self.reliable_streams {
            if let Ok(Some(packet)) = stream.read_chunk().await {
                self.events_tx.send(GameEvent::Packet(packet));
            }
        }
    }

    pub async fn next_event(&self) -> Result<GameEvent, TryRecvError> {
        self.events_rx.try_recv()
    }
}

impl Deref for QPNewConnection {
    type Target = quinn::NewConnection;

    fn deref(&self) -> &Self::Target {
        &self.new_connection
    }
}

impl DerefMut for QPNewConnection {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.new_connection
    }
}

pub enum GameEvent {
    Packet(Packet),
}

pub enum QPStream {
    Reliable(QPRecvStream),
    ReliableUnordered(QPRecvStream),
    ReliableSequenced(QPRecvStream),
    Unreliable(QPRecvStream),
    UnreliableSequenced(QPRecvStream),
}

pub struct GameOpenUni(quinn::OpenUni);

impl Future for GameOpenUni {
    type Output = Result<QPSendStream, quinn::ConnectionError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {

        return match Pin::new(&mut self.0).poll(cx) {
            Poll::Ready(Ok(value)) => {
                Poll::Ready(Ok(QPSendStream(value)))
            }
            Poll::Ready(Err(value)) => {
                Poll::Ready(Err(value))
            }
            _ => { Poll::Pending }
        }
    }
}