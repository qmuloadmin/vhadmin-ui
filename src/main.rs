use anyhow::Error;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, iter::FromIterator};
use web_sys::console;
use yew::format::{Json, Nothing};
use yew::services::fetch::{FetchService, FetchTask, Request, Response};

use std::array::IntoIter;

use yew::prelude::*;

#[derive(PartialEq, Hash, Deserialize, Serialize, Clone, Copy)]
enum Server {
    Default,
    Rotis,
}

impl Eq for Server {
    fn assert_receiver_is_total_eq(&self) {}
}

enum StatusMsg {
    StartServer(Server),
    StopServer(Server),
    UpdateServer(Server),
    StatusUpdate(ServerStatuses),
    Error(Error),
}

#[derive(Serialize)]
enum ServerAction {
    Start,
    Stop,
    Update,
}

#[derive(Deserialize, PartialEq)]
enum Status {
    Stopped,
    Starting,
    Running,
    ShuttingDown,
    Updating,
    Unknown,
}

#[derive(Deserialize)]
struct StatusResponse {
    servers: HashMap<Server, Status>,
}

#[derive(Serialize)]
struct ServerRequest<'a> {
    server: &'a Server,
    action: &'a ServerAction,
}

impl std::fmt::Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use Status::*;
        match &self {
            Stopped => write!(f, "not running"),
            Starting => write!(f, "starting"),
            Running => write!(f, "running"),
            ShuttingDown => write!(f, "shutting down"),
            Updating => write!(f, "updating"),
            Unknown => write!(f, "unknown"),
        }
    }
}

impl std::fmt::Display for Server {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Server::Default => write!(f, "Default"),
            Server::Rotis => write!(f, "Rotis"),
        }
    }
}

type ServerStatuses = HashMap<Server, Status>;

struct StatusPage {
    link: ComponentLink<Self>,
    // StatusBar shows the current running server
    body: String,
    server_statuses: ServerStatuses,
    task: Option<FetchTask>,
}

impl StatusPage {
    fn server_rows(&self) -> Html {
        let disabled = self.task.is_some();
        let server_row = |row: (&Server, &Status)| {
            let server = row.0.clone();
            let color = match row.1 {
                &Status::Running => "#2ecc40",
                &Status::Starting | &Status::Updating => "#ff851b",
                _ => "#ff4136",
            };
            html! {
                <article class="card">
                    <header>
                      <h2>
                        <span style={format!("color: {}", color)}>{"⬤"}</span>
                           {Self::server_name(&row.0)}{": "}{row.1}
                      </h2>
                    </header>
                { match row.1 {
                    &Status::Stopped => html!{
                        <>
                            <button class="success" disabled={disabled}
                                onclick=self.link.callback(move |_| StatusMsg::StartServer(server))>{"Start"}</button>
                            <button class="warning" disabled={disabled}
                                onclick=self.link.callback(move |_| StatusMsg::UpdateServer(server))>{"Update"}</button>
                        </>

                   },
                    &Status::Running => html!{
                        <button class="error" disabled={disabled}
                            onclick=self.link.callback(move |_| StatusMsg::StopServer(server))>{"Stop"}</button>
                    },
                    _ => html!{<></>}
                }}
                </article>
            }
        };
        html! {
            <>
            {for self.server_statuses.iter().map(server_row)}
            </>
        }
    }

    fn fetch_server_status(&self) -> FetchTask {
        let request = Request::get("/vhadminapi/status")
            .body(Nothing)
            .expect("Could not build get status request");
        let callback = self.link.callback(
            |response: Response<Json<Result<StatusResponse, anyhow::Error>>>| {
                let Json(data) = response.into_body();
                match data {
                    Ok(servers) => StatusMsg::StatusUpdate(servers.servers),
                    Err(err) => StatusMsg::Error(err),
                }
            },
        );
        return FetchService::fetch(request, callback)
            .expect("Failed to start request to server statuses");
    }

    fn request_server_action(&self, server: &Server, action: &ServerAction) -> FetchTask {
        let body = ServerRequest { server, action };
        let request = Request::post("/vhadminapi/action")
            .body(Json(&body))
            .expect("could not build action request");
        let callback = self.link.callback(
            |response: Response<Json<Result<StatusResponse, anyhow::Error>>>| {
                let Json(data) = response.into_body();
                match data {
                    Ok(servers) => StatusMsg::StatusUpdate(servers.servers),
                    Err(err) => StatusMsg::Error(err),
                }
            },
        );
        return FetchService::fetch(request, callback)
            .expect("Failed to start request to server action");
    }

    fn server_name(server: &Server) -> String {
        match server {
            Server::Default => "Default".to_string(),
            Server::Rotis => "Rotis".to_string(),
        }
    }
}

impl Component for StatusPage {
    type Message = StatusMsg;

    type Properties = ();

    fn create(props: Self::Properties, link: ComponentLink<Self>) -> Self {
        let mut thing = Self {
            link,
            body: "Getting server status...".to_owned(),
            server_statuses: HashMap::from_iter(IntoIter::new([
                (Server::Default, Status::Unknown),
                (Server::Rotis, Status::Running),
            ])),
            task: None,
        };
        thing.task = Some(thing.fetch_server_status());
        thing
    }

    fn update(&mut self, msg: Self::Message) -> ShouldRender {
        match msg {
            StatusMsg::StartServer(ref server) => match &self.server_statuses.get(&server).unwrap()
            {
                Status::Running => false,
                Status::Starting => {
                    self.body = format!("server {} is already starting", server);
                    true
                }
                Status::ShuttingDown => {
                    self.body = format!("server {} is stopping. Please try again later", server);
                    true
                }
                Status::Updating => {
                    self.body = format!("server {} is updating. Please try again later", server);
                    true
                }
                _ => {
                    for (k, v) in &self.server_statuses {
                        if server == k {
                            continue;
                        }
                        if v != &Status::Stopped {
                            self.body = format!("server {} is not stopped. Please stop before trying to start another server", k);
                            return true;
                        }
                    }
                    self.task = Some(self.request_server_action(&server, &ServerAction::Start));
                    self.body = format!("starting server {}", server);
                    true
                }
            },
            StatusMsg::StopServer(ref server) => {
                match &self.server_statuses.get(&server).unwrap() {
                    Status::Stopped => false,
                    Status::Starting => {
                        self.body = format!(
                            "server {} is currently starting. Please try again later.",
                            server
                        );
                        true
                    }
                    Status::ShuttingDown => {
                        self.body = format!("server {} is already shutting down.", server);
                        true
                    }
                    _ => {
                        self.body = format!("stopping server {}", server);
                        self.task = Some(self.request_server_action(server, &ServerAction::Stop));
                        true
                    }
                }
            }
            StatusMsg::UpdateServer(ref server) => {
                match &self.server_statuses.get(&server).unwrap() {
                    Status::Updating => false,
                    _ => {
                        self.body = format!("updating server {}", server);
                        self.task = Some(self.request_server_action(server, &ServerAction::Update));
                        true
                    }
                }
            }
            StatusMsg::StatusUpdate(statuses) => {
                self.task = None;
                self.server_statuses = statuses;
                self.body = "Accepting commands".to_owned();
                true
            }
            StatusMsg::Error(err) => {
                self.task = None;
                self.body =
                    "An error occurred. Refresh the page and try again or ask Zach for help"
                        .to_owned();
                console::log_1(&format!("error: {:?}", err).into());
                true
            }
        }
    }

    fn change(&mut self, _props: Self::Properties) -> ShouldRender {
        false
    }

    fn view(&self) -> Html {
        html! {
            <article class="card">
                <header><h1>{"Valheim Server Control"}</h1></header>
                <footer>{"Status: "}{ &self.body }</footer>
                <br/>
            {"Servers:"}
            {self.server_rows()}
            </article>
        }
    }
}

fn main() {
    yew::start_app::<StatusPage>();
}
