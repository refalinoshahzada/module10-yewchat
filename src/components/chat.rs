use serde::{Deserialize, Serialize};
use web_sys::HtmlInputElement;
use yew::prelude::*;
use yew_agent::{Bridge, Bridged};

use crate::services::event_bus::EventBus;
use crate::{services::websocket::WebsocketService, User};

pub enum Msg {
    HandleMsg(String),
    SubmitMessage,
    RegisterUser,
    FetchUsers,
}

#[derive(Deserialize)]
struct MessageData {
    from: String,
    message: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum MsgTypes {
    Users,
    Register,
    Message,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WebSocketMessage {
    message_type: MsgTypes,
    data_array: Option<Vec<String>>,
    data: Option<String>,
}

#[derive(Clone)]
struct UserProfile {
    name: String,
    avatar: String,
}

pub struct Chat {
    users: Vec<UserProfile>,
    chat_input: NodeRef,
    _producer: Box<dyn Bridge<EventBus>>,
    wss: WebsocketService,
    messages: Vec<MessageData>,
    username: String,
}

impl Component for Chat {
    type Message = Msg;
    type Properties = ();

    fn create(ctx: &Context<Self>) -> Self {
        let (user, _) = ctx
            .link()
            .context::<User>(Callback::noop())
            .expect("context to be set");
        let username = user.username.borrow().clone();
        let wss = WebsocketService::new();

        // Set the connection callback
        let link = ctx.link().clone();
        wss.set_on_connection_ready(Box::new(move || {
            link.send_message(Msg::RegisterUser);
        }));

        Self {
            users: vec![],
            messages: vec![],
            chat_input: NodeRef::default(),
            wss,
            _producer: EventBus::bridge(ctx.link().callback(Msg::HandleMsg)),
            username,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::HandleMsg(s) => {
                log::debug!("Received message: {}", s);
                let msg: WebSocketMessage = match serde_json::from_str(&s) {
                    Ok(m) => m,
                    Err(e) => {
                        log::error!("Error parsing message: {:?}", e);
                        return false;
                    }
                };

                match msg.message_type {
                    MsgTypes::Users => {
                        let users_from_message = msg.data_array.unwrap_or_default();
                        log::debug!("Got users: {:?}", users_from_message);
                        self.users = users_from_message
                            .iter()
                            .map(|u| UserProfile {
                                name: u.into(),
                                avatar: format!(
                                    "https://avatars.dicebear.com/api/adventurer-neutral/{}.svg",
                                    u
                                )
                                .into(),
                            })
                            .collect();
                        return true;
                    }
                    MsgTypes::Message => {
                        let message_data: MessageData =
                            serde_json::from_str(&msg.data.unwrap()).unwrap();
                        self.messages.push(message_data);
                        return true;
                    }
                    _ => {
                        return false;
                    }
                }
            }
            Msg::RegisterUser => {
                log::debug!("Registering user: {}", self.username);
                let message = WebSocketMessage {
                    message_type: MsgTypes::Register,
                    data: Some(self.username.clone()),
                    data_array: None,
                };

                match serde_json::to_string(&message) {
                    Ok(json) => {
                        if let Err(e) = self.wss.tx.clone().try_send(json) {
                            log::error!("Error sending registration: {:?}", e);
                        } else {
                            log::debug!("Registration sent successfully");
                            // After registration, request users list
                            ctx.link().send_message(Msg::FetchUsers);
                        }
                    }
                    Err(e) => log::error!("Error serializing registration: {:?}", e),
                }
                false
            }
            Msg::FetchUsers => {
                // Optional: Explicitly request users list from server if needed
                // This might not be necessary if your server automatically broadcasts user lists
                let message = WebSocketMessage {
                    message_type: MsgTypes::Users,
                    data: None,
                    data_array: None,
                };

                if let Ok(json) = serde_json::to_string(&message) {
                    if let Err(e) = self.wss.tx.clone().try_send(json) {
                        log::error!("Error requesting users: {:?}", e);
                    }
                }
                false
            }
            Msg::SubmitMessage => {
                let input = self.chat_input.cast::<HtmlInputElement>();
                if let Some(input) = input {
                    if !input.value().is_empty() {
                        let message = WebSocketMessage {
                            message_type: MsgTypes::Message,
                            data: Some(input.value()),
                            data_array: None,
                        };

                        if let Ok(json) = serde_json::to_string(&message) {
                            if let Err(e) = self.wss.tx.clone().try_send(json) {
                                log::error!("error sending to channel: {:?}", e);
                            }
                        }
                        input.set_value("");
                    }
                };
                false
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let submit = ctx.link().callback(|_| Msg::SubmitMessage);

        html! {
            <div class="flex w-screen">
                <div class="flex-none w-56 h-screen bg-gray-100">
                    <div class="text-xl p-3">{"Users"}</div>
                    {
                        self.users.clone().iter().map(|u| {
                            html!{
                                <div class="flex m-3 bg-white rounded-lg p-2">
                                    <div>
                                        <img class="w-12 h-12 rounded-full" src={u.avatar.clone()} alt="avatar"/>
                                    </div>
                                    <div class="flex-grow p-3">
                                        <div class="flex text-xs justify-between">
                                            <div>{u.name.clone()}</div>
                                        </div>
                                        <div class="text-xs text-gray-400">
                                            {"Hi there!"}
                                        </div>
                                    </div>
                                </div>
                            }
                        }).collect::<Html>()
                    }
                </div>
                <div class="grow h-screen flex flex-col">
                    <div class="w-full h-14 border-b-2 border-gray-300"><div class="text-xl p-3">{"💬 Chat!"}</div></div>
                    <div class="w-full grow overflow-auto border-b-2 border-gray-300">
                        {
                            self.messages.iter().map(|m| {
                                let user = self.users.iter().find(|u| u.name == m.from).unwrap();
                                html!{
                                    <div class="flex items-end w-3/6 bg-gray-100 m-8 rounded-tl-lg rounded-tr-lg rounded-br-lg ">
                                        <img class="w-8 h-8 rounded-full m-3" src={user.avatar.clone()} alt="avatar"/>
                                        <div class="p-3">
                                            <div class="text-sm">
                                                {m.from.clone()}
                                            </div>
                                            <div class="text-xs text-gray-500">
                                                if m.message.ends_with(".gif") {
                                                    <img class="mt-3" src={m.message.clone()}/>
                                                } else {
                                                    {m.message.clone()}
                                                }
                                            </div>
                                        </div>
                                    </div>
                                }
                            }).collect::<Html>()
                        }

                    </div>
                    <div class="w-full h-14 flex px-3 items-center">
                        <input ref={self.chat_input.clone()} type="text" placeholder="Message" class="block w-full py-2 pl-4 mx-3 bg-gray-100 rounded-full outline-none focus:text-gray-700" name="message" required=true />
                        <button onclick={submit} class="p-3 shadow-sm bg-blue-600 w-10 h-10 rounded-full flex justify-center items-center color-white">
                            <svg viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg" class="fill-white">
                                <path d="M0 0h24v24H0z" fill="none"></path><path d="M2.01 21L23 12 2.01 3 2 10l15 2-15 2z"></path>
                            </svg>
                        </button>
                    </div>
                </div>
            </div>
        }
    }
}