use std::net::UdpSocket;

use std::thread::sleep;
use std::time::Duration;
use std::{env, io::Error as IoError};

use futures_util::{future, stream::TryStreamExt, StreamExt};
use futures_util::{pin_mut, SinkExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::protocol::Message;

// Static data to read the UDP arrays into.
// Tried using unbounded_channel for comm between the UDP thread and WS threads but couldn't get it to work when only 1 WS client was present
static mut UDP_DATA: Vec<u8> = Vec::<u8>::new();

// Handels WebSocket connections
// raw_stream : Raw stream of WebSocket stream
// sock_udp : A udp socket to WRITE the data to the PLC, the reading is handled by another udp socket conn.
async fn handle_connection(raw_stream: TcpStream, sock_udp: UdpSocket) {
    let ws_stream = tokio_tungstenite::accept_async(raw_stream)
        .await
        .expect("Error during the websocket handshake occurred");

    let (mut outgoing, incoming) = ws_stream.split();

    let sock_udp_2 = sock_udp.try_clone().unwrap();

    // Unbounded channel to send data between the WebSocket reader, and WebSocket writer futures.
    let (tx, mut rx) = mpsc::unbounded_channel::<Message>();

    // Handles the incoming data from WebSocket clients.
    let msg_inc = incoming.try_for_each(|msg| {
        // Check if the data from the WebSocket is binary (Vec[u8]) or text (String)
        let is_bin_msg = msg.is_binary();
        let is_text_msg = msg.is_text();

        // If it's a text that means its a "request" from the WebSocket client to read out the UDP data
        if is_text_msg {
            match msg.clone().into_text() {
                Ok(msg_ok) => {
                    if msg_ok == "get_vals".to_string() {
                        let _ = tx.send(Message::from("write_vals_to_ws"));
                    } else {
                    };
                }
                Err(_err) => {
                    println!("Error: {_err:?}");
                    let _ = tx.send(Message::from("write_udp_done"));
                }
            };
        }
        // If the data is binary, than the WebSocket clients sends the data, that should be forwarded to the PLC on UDP.
        else if is_bin_msg {
            match msg.clone().into_data() {
                msg_ok => {
                    let msg_bin = &msg_ok;

                    let msg_bin_u8: &[u8] = msg_bin;

                    let _sock_resp = sock_udp_2.send_to(msg_bin_u8, "127.0.0.1:34125").unwrap();

                    let _ = tx.send(Message::from("write_udp_done"));
                }
            };
        }

        future::ok(())
    });

    // Future to handle the inner comm on the unbounded channel
    // It writes the outgoing stream for the WebSocket, so basically this handles the data writes to the WebSocket.
    let send_data = async {
        while let Some(msg) = rx.recv().await {
            if msg.clone().into_text().unwrap() == "write_vals_to_ws".to_string() {
                unsafe {
                    let _resp = outgoing.send(Message::from(UDP_DATA.clone())).await;
                };
            } else if msg.clone().into_text().unwrap() == "write_udp_done".to_string() {
                let _resp = outgoing.send(Message::from("write_udp_done")).await;
            }
        }
    };

    pin_mut!(msg_inc, send_data);
    future::select(msg_inc, send_data).await;
}

// Reads the udp socket, this function gets the values from the PLC.
// Stores it in a "GLOBAL" variable.
async fn read_udp(sock_udp: UdpSocket) -> Result<(), IoError> {
    let mut buf = [0; 100];
    let (_amt, _src) = sock_udp.recv_from(&mut buf)?;

    let msg_to_send = Vec::from(buf);
    unsafe {
        UDP_DATA = msg_to_send;
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), IoError> {
    let addr_ws = env::args()
        .nth(1)
        .unwrap_or_else(|| "0.0.0.0:8081".to_string());

    let addr_udp_read = env::args()
        .nth(2)
        .unwrap_or_else(|| "127.0.0.1:34123".to_string());

    let addr_udp_write = env::args()
        .nth(3)
        .unwrap_or_else(|| "127.0.0.1:34126".to_string());

    let addr_to_displ = addr_udp_read.clone();

    let socket_udp_read = UdpSocket::bind(addr_udp_read).expect("Couldn't bind udp socket");
    socket_udp_read
        .set_nonblocking(true)
        .expect("Failed to enter non-blocking mode");

    let socket_udp_write = UdpSocket::bind(addr_udp_write).expect("Couldn't bind udp write socket");
    socket_udp_write
        .set_nonblocking(true)
        .expect("Faild to enter non-blocking mode on udp write");

    let socket_udp_write_2 = socket_udp_write.try_clone().unwrap();

    let try_socket = TcpListener::bind(&addr_ws).await;
    let listener = try_socket.expect("Failed to bind");

    println!(
        "Listening on: {} for websockets and on: {} for UDP connections",
        addr_ws, addr_to_displ
    );

    // Spawns a new thread to handle the UDP read.
    tokio::spawn(async move {
        loop {
            let _ = read_udp(socket_udp_read.try_clone().unwrap()).await;

            // Time off between reads, not to hug 100% of the CPU thread.
            let twm = Duration::from_millis(10);
            sleep(twm);
        }
    });

    // WebSocket listener
    while let Ok((stream, _addr_ws)) = listener.accept().await {
        let socket_udp_to_write = socket_udp_write_2.try_clone().unwrap();
        // Spawns a new thread for each websocket connection.
        tokio::spawn(handle_connection(stream, socket_udp_to_write));
    }
    Ok(())
}
