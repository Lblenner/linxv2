use async_std::{
    channel::{unbounded, Receiver, Sender},
    net::{SocketAddr, TcpListener, TcpStream},
    task,
};
use async_tungstenite::{accept_async, tungstenite::Message, WebSocketStream};
use futures::{future::join_all, prelude::*, stream::SplitSink};
use std::collections::HashMap;

fn main() {
    let (s, r) = unbounded::<Event>();

    task::spawn(event_handler(r));
    task::block_on(socket_acceptor(s));
}

pub async fn socket_acceptor(s: Sender<Event>) {
    let mut id: i128 = 0; //TODO

    let server = TcpListener::bind("127.0.0.1:8020").await.unwrap();

    println!("WebSocket server listening on port {}", "8020");

    loop {
        let accept_result = server.accept().await;

        match accept_result {
            Err(err) => println!("Error while accepting client: {}", err.to_string()),
            Ok((stream, addr)) => {
                task::spawn(accept_connection(stream, addr, id, s.clone()));
                id = id + 1;
            }
        }
    }
}

async fn accept_connection(stream: TcpStream, addr: SocketAddr, id: i128, s: Sender<Event>) {
    let accept_resut = accept_async(stream).await.unwrap();

    println!("New WebSocket: {}", addr);

    let (outgoing, incoming) = accept_resut.split();

    s.send(Event::NewUser(NewUser::new(outgoing, id)))
        .await
        .unwrap();

    //Create a future that will execute incoming message for each message received
    let incoming_msg = incoming.try_for_each(move |msg| {
        task::spawn(message_handler(addr, msg, s.clone(), id));
        future::ok(())
    });

    task::spawn(incoming_msg);
}

pub async fn message_handler(_addr: SocketAddr, msg: Message, s: Sender<Event>, id: i128) {
    let message = msg.to_text().unwrap();

    s.send(Event::NewMessage(NewMessage::new(message.to_string(), id)))
        .await
        .unwrap();
}

pub enum Event {
    NewUser(NewUser),
    NewMessage(NewMessage),
}

pub type OutStream = SplitSink<WebSocketStream<TcpStream>, Message>;
pub struct NewUser {
    pub stream: OutStream,
    pub id: i128,
}

impl NewUser {
    pub fn new(stream: OutStream, id: i128) -> Self {
        NewUser {
            stream: stream,
            id: id,
        }
    }
}

pub struct NewMessage {
    pub id: i128,
    pub message: String,
}

impl NewMessage {
    pub fn new(message: String, id: i128) -> Self {
        NewMessage {
            message: message,
            id: id,
        }
    }
}

pub async fn event_handler(r: Receiver<Event>) {
    let mut current_users: HashMap<i128, OutStream> = HashMap::new();

    loop {
        let event = r.recv().await.unwrap();

        match event {
            Event::NewMessage(new_message) => {
                println!("Event: NewMessage");
                let mut v = Vec::new();
                for (&id, stream) in current_users.iter_mut() {
                    if id != new_message.id {
                        let ft = stream.send(Message::from(new_message.message.to_string()));
                        v.push(ft);
                    }
                }
                join_all(v).await;
            }
            Event::NewUser(new_socket) => {
                println!("Event: NewUser");
                current_users.insert(new_socket.id, new_socket.stream);
            }
        }
    }
}
