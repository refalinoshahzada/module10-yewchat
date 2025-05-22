use futures::{channel::mpsc::{Sender, channel}, SinkExt, StreamExt};
use reqwasm::websocket::{futures::WebSocket, Message};
use std::cell::RefCell;
use std::rc::Rc;

use wasm_bindgen_futures::spawn_local;
use yew_agent::Dispatched;

use crate::services::event_bus::{EventBus, Request};

pub type ConnectionReadyCallback = Rc<RefCell<Option<Box<dyn FnOnce()>>>>;

pub struct WebsocketService {
    pub tx: Sender<String>,
    pub connection_ready: ConnectionReadyCallback,
}

impl WebsocketService {
    pub fn new() -> Self {
        let ws = match WebSocket::open("ws://127.0.0.1:8080") {
            Ok(socket) => socket,
            Err(e) => {
                log::error!("Error opening WebSocket: {:?}", e);
                panic!("Failed to open WebSocket");
            }
        };

        let (mut write, mut read) = ws.split();
        let (in_tx, mut in_rx) = channel::<String>(1000);
        let mut event_bus = EventBus::dispatcher();
        let connection_ready: ConnectionReadyCallback = Rc::new(RefCell::new(None));
        let connection_ready_clone = connection_ready.clone();

        spawn_local(async move {
            // Signal that connection is ready before entering the loop
            if let Some(callback) = connection_ready_clone.borrow_mut().take() {
                callback();
            }
            
            while let Some(s) = in_rx.next().await {
                log::debug!("Sending message: {}", s);
                match write.send(Message::Text(s)).await {
                    Ok(_) => log::debug!("Message sent successfully"),
                    Err(e) => log::error!("Error sending WebSocket message: {:?}", e),
                }
            }
        });

        spawn_local(async move {
            while let Some(msg) = read.next().await {
                match msg {
                    Ok(Message::Text(data)) => {
                        log::debug!("from websocket: {}", data);
                        event_bus.send(Request::EventBusMsg(data));
                    }
                    Ok(Message::Bytes(b)) => {
                        let decoded = std::str::from_utf8(&b);
                        if let Ok(val) = decoded {
                            log::debug!("from websocket: {}", val);
                            event_bus.send(Request::EventBusMsg(val.into()));
                        }
                    }
                    Err(e) => {
                        log::error!("WebSocket error: {:?}", e)
                    }
                }
            }
            log::debug!("WebSocket Closed");
        });

        Self { 
            tx: in_tx,
            connection_ready, 
        }
    }

    pub fn set_on_connection_ready(&self, callback: Box<dyn FnOnce()>) {
        *self.connection_ready.borrow_mut() = Some(callback);
    }
}