use std::net::UdpSocket;

use std::{
    collections::HashMap,
    env,
    io::Error as IoError,
    net::SocketAddr,
    sync::{Arc, Mutex},
};

use futures_channel::mpsc::{unbounded, UnboundedSender};
use futures_util::{future, pin_mut, stream::TryStreamExt, StreamExt};

use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::protocol::Message;

type Tx = UnboundedSender<Message>;
type PeerMap = Arc<Mutex<HashMap<SocketAddr, Tx>>>;

async fn handle_connection(
    peer_map: PeerMap,
    raw_stream: TcpStream,
    addr: SocketAddr,
    sock_udp: UdpSocket,
) {
    let ws_stream = tokio_tungstenite::accept_async(raw_stream)
        .await
        .expect("Error during the websocket handshake occurred");

    let (tx, rx) = unbounded();
    peer_map.lock().unwrap().insert(addr, tx);

    let (outgoing, incoming) = ws_stream.split();

    let broadcast_incoming = incoming.try_for_each(|msg| {
        println!("{msg:?}");

        let msg_bin = &msg.into_data();
        let msg_bin_u8: &[u8] = msg_bin;
        let _sock_resp = sock_udp.send_to(msg_bin_u8, "127.0.0.1:34125");
        println!("{_sock_resp:?}");
        let peers = peer_map.lock().unwrap();
        let broadcast_recipients = peers.iter().map(|(_, ws_sink)| ws_sink);
        let msg_to_send = Vec::new();
        for recp in broadcast_recipients {
            recp.unbounded_send(Message::Binary(msg_to_send.clone()))
                .unwrap();
        }
        future::ok(())
    });

    let receive_from_others = rx.map(Ok).forward(outgoing);

    pin_mut!(broadcast_incoming, receive_from_others);
    future::select(broadcast_incoming, receive_from_others).await;

    println!("{} disconnected", &addr);
    peer_map.lock().unwrap().remove(&addr);
}

async fn read_udp(sock_udp: UdpSocket) -> Result<(), IoError> {
    let mut buf = [0; 100];
    let (_amt, _src) = sock_udp.recv_from(&mut buf)?;
    println!("{buf:?}");

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), IoError> {
    let addr_ws = env::args()
        .nth(1)
        .unwrap_or_else(|| "0.0.0.0:8081".to_string());

    let addr_udp = env::args()
        .nth(2)
        .unwrap_or_else(|| "127.0.0.1:34123".to_string());

    let addr_to_displ = addr_udp.clone();

    let socket_udp = UdpSocket::bind(addr_udp).expect("Couldn't bind udp socket");
    socket_udp
        .set_nonblocking(true)
        .expect("Failed to enter non-blocking mode");

    let socket_udp_2 = socket_udp.try_clone().unwrap();

    let state = PeerMap::new(Mutex::new(HashMap::new()));
    let state_2 = PeerMap::new(Mutex::new(HashMap::new()));
    let state_2_helper = state_2.clone();

    let try_socket = TcpListener::bind(&addr_ws).await;
    let listener = try_socket.expect("Failed to bind");
    println!(
        "Listening on: {} for websockets and on: {} for UDP connections",
        addr_ws, addr_to_displ
    );
    tokio::spawn(async move {
        loop {
            let _ = read_udp(socket_udp.try_clone().unwrap()).await;
        }
    });

    while let Ok((stream, addr_ws)) = listener.accept().await {
        let socket_udp_to_write = socket_udp_2.try_clone().unwrap();
        tokio::spawn(handle_connection(
            state.clone(),
            stream,
            addr_ws,
            socket_udp_to_write,
        ));
    }

    Ok(())
}
