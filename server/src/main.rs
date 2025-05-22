use futures_util::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::protocol::Message;
use serde::{Deserialize, Serialize};

type Users = Arc<Mutex<HashMap<SocketAddr, String>>>;
type PeerMap = Arc<Mutex<HashMap<SocketAddr, tokio::sync::mpsc::UnboundedSender<Message>>>>;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum MsgType {
    Users,
    Register,
    Message,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WebSocketMessage {
    message_type: MsgType,
    data_array: Option<Vec<String>>,
    data: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct ChatMessage {
    from: String,
    message: String,
}

async fn handle_connection(
    peer_map: PeerMap,
    users: Users,
    raw_stream: TcpStream,
    addr: SocketAddr,
) {
    println!("Incoming TCP connection from: {}", addr);

    let ws_stream = tokio_tungstenite::accept_async(raw_stream)
        .await
        .expect("Error during WebSocket handshake");
    println!("WebSocket connection established with: {}", addr);

    let (mut ws_sender, mut ws_receiver) = ws_stream.split();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

    // Insert the new client
    peer_map.lock().unwrap().insert(addr, tx);

    // Broadcast the connected users list to all connections
    let broadcast_users = || {
        let users = users.lock().unwrap();
        let user_list: Vec<String> = users.values().cloned().collect();
        
        let msg = WebSocketMessage {
            message_type: MsgType::Users,
            data_array: Some(user_list),
            data: None,
        };
        
        let json = serde_json::to_string(&msg).unwrap();
        
        let peer_map = peer_map.lock().unwrap();
        for (_peer_addr, tx) in peer_map.iter() {
            let _ = tx.send(Message::Text(json.clone()));
        }
    };

    // Forward messages from internal channel to WebSocket
    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if let Err(e) = ws_sender.send(msg).await {
                println!("Error sending message to {}: {}", addr, e);
                break;
            }
        }
    });

    // Process incoming messages
    while let Some(result) = ws_receiver.next().await {
        match result {
            Ok(msg) => {
                if let Message::Text(text) = msg {
                    println!("Received message from {}: {}", addr, text);
                    
                    // Try to parse the message
                    if let Ok(ws_msg) = serde_json::from_str::<WebSocketMessage>(&text) {
                        match ws_msg.message_type {
                            MsgType::Register => {
                                if let Some(username) = ws_msg.data {
                                    users.lock().unwrap().insert(addr, username);
                                    broadcast_users();
                                }
                            },
                            MsgType::Message => {
                                if let Some(content) = ws_msg.data {
                                    let username = users.lock().unwrap().get(&addr).cloned().unwrap_or_else(|| "Anonymous".to_string());
                                    
                                    let chat_msg = ChatMessage {
                                        from: username,
                                        message: content,
                                    };
                                    
                                    let msg_json = serde_json::to_string(&chat_msg).unwrap();
                                    
                                    let response = WebSocketMessage {
                                        message_type: MsgType::Message,
                                        data: Some(msg_json),
                                        data_array: None,
                                    };
                                    
                                    let response_json = serde_json::to_string(&response).unwrap();
                                    
                                    let peer_map = peer_map.lock().unwrap();
                                    for (_peer_addr, tx) in peer_map.iter() {
                                        let _ = tx.send(Message::Text(response_json.clone()));
                                    }
                                }
                            },
                            MsgType::Users => {
                                broadcast_users();
                            },
                        }
                    }
                }
            },
            Err(e) => {
                println!("Error receiving message from {}: {}", addr, e);
                break;
            }
        }
    }

    // Connection closed
    println!("WebSocket connection closed: {}", addr);
    users.lock().unwrap().remove(&addr);
    peer_map.lock().unwrap().remove(&addr);
    broadcast_users();
}

#[tokio::main]
async fn main() {
    // Create a simple logger
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));

    let addr = "127.0.0.1:8080".to_string();
    let listener = TcpListener::bind(&addr).await.expect("Failed to bind");
    println!("WebSocket server listening on: {}", addr);

    let peer_map = PeerMap::new(Mutex::new(HashMap::new()));
    let users = Users::new(Mutex::new(HashMap::new()));

    while let Ok((stream, addr)) = listener.accept().await {
        let peer_map_clone = peer_map.clone();
        let users_clone = users.clone();
        
        tokio::spawn(async move {
            handle_connection(peer_map_clone, users_clone, stream, addr).await;
        });
    }
}