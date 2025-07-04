use serde::{Deserialize, Serialize};
use wasm_bindgen_futures::spawn_local;
use web_sys::window;
use yew::prelude::*;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConnectedNode {
    pub node_id: String,
    pub domain: String,
    pub connected_at: String,
    pub uptime_seconds: u64,
    pub latency_ms: Option<u64>,
    pub last_ping: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListNodesResponse {
    pub nodes: Vec<ConnectedNode>,
    pub total: usize,
}

pub enum Msg {
    FetchNodes,
    NodesReceived(Result<ListNodesResponse, String>),
}

pub struct NodesList {
    nodes: Vec<ConnectedNode>,
    loading: bool,
    error: Option<String>,
    _interval: Option<gloo_timers::callback::Interval>,
}

impl Component for NodesList {
    type Message = Msg;
    type Properties = ();

    fn create(ctx: &Context<Self>) -> Self {
        // Fetch nodes immediately
        ctx.link().send_message(Msg::FetchNodes);

        // Set up auto-refresh every 5 seconds
        let link = ctx.link().clone();
        let interval = gloo_timers::callback::Interval::new(5000, move || {
            link.send_message(Msg::FetchNodes);
        });

        Self {
            nodes: vec![],
            loading: true,
            error: None,
            _interval: Some(interval),
        }
    }

    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::FetchNodes => {
                self.loading = true;
                let link = _ctx.link().clone();

                spawn_local(async move {
                    let result = fetch_nodes().await;
                    link.send_message(Msg::NodesReceived(result));
                });

                false
            }
            Msg::NodesReceived(result) => {
                self.loading = false;
                match result {
                    Ok(response) => {
                        self.nodes = response.nodes;
                        self.error = None;
                    }
                    Err(e) => {
                        self.error = Some(e);
                    }
                }
                true
            }
        }
    }

    fn view(&self, _ctx: &Context<Self>) -> Html {
        html! {
            <div class="bg-white dark:bg-gray-800 rounded-lg shadow">
                <div class="px-6 py-4 border-b border-gray-200 dark:border-gray-700">
                    <h3 class="text-lg font-semibold text-gray-900 dark:text-white">
                        {"Connected Nodes"}
                        if !self.nodes.is_empty() {
                            <span class="ml-2 text-sm font-normal text-gray-500 dark:text-gray-400">
                                {format!("({} total)", self.nodes.len())}
                            </span>
                        }
                    </h3>
                </div>

                <div class="p-6">
                    {if self.loading && self.nodes.is_empty() {
                        html! {
                            <div class="text-center py-8">
                                <div class="inline-flex items-center">
                                    <svg class="animate-spin h-5 w-5 mr-2 text-blue-600" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
                                        <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                                        <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                                    </svg>
                                    <span class="text-gray-600 dark:text-gray-400">{"Loading nodes..."}</span>
                                </div>
                            </div>
                        }
                    } else if let Some(error) = &self.error {
                        html! {
                            <div class="text-center py-8">
                                <p class="text-red-600 dark:text-red-400">{format!("Error: {}", error)}</p>
                            </div>
                        }
                    } else if self.nodes.is_empty() {
                        html! {
                            <div class="text-center py-8">
                                <p class="text-gray-500 dark:text-gray-400">{"No nodes connected"}</p>
                            </div>
                        }
                    } else {
                        html! {
                            <div class="overflow-x-auto">
                                <table class="min-w-full divide-y divide-gray-200 dark:divide-gray-700">
                                    <thead>
                                        <tr>
                                            <th class="px-4 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider">
                                                {"Node ID"}
                                            </th>
                                            <th class="px-4 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider">
                                                {"Domain"}
                                            </th>
                                            <th class="px-4 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider">
                                                {"Uptime"}
                                            </th>
                                            <th class="px-4 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider">
                                                {"Latency"}
                                            </th>
                                            <th class="px-4 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider">
                                                {"Status"}
                                            </th>
                                        </tr>
                                    </thead>
                                    <tbody class="divide-y divide-gray-200 dark:divide-gray-700">
                                        {for self.nodes.iter().map(|node| self.view_node(node))}
                                    </tbody>
                                </table>
                            </div>
                        }
                    }}
                </div>
            </div>
        }
    }
}

impl NodesList {
    fn view_node(&self, node: &ConnectedNode) -> Html {
        let short_id = &node.node_id[..10.min(node.node_id.len())];
        let uptime = format_duration(node.uptime_seconds);
        let latency = node
            .latency_ms
            .map(|ms| format!("{ms} ms"))
            .unwrap_or_else(|| "N/A".to_string());

        html! {
            <tr class="hover:bg-gray-50 dark:hover:bg-gray-750">
                <td class="px-4 py-3 whitespace-nowrap">
                    <div class="flex items-center">
                        <div class="text-sm">
                            <div class="font-medium text-gray-900 dark:text-white font-mono">
                                {short_id}{"..."}
                            </div>
                            <div class="text-xs text-gray-500 dark:text-gray-400 font-mono">
                                {&node.node_id}
                            </div>
                        </div>
                    </div>
                </td>
                <td class="px-4 py-3 whitespace-nowrap">
                    <a href={format!("https://{}", node.domain)}
                       target="_blank"
                       class="text-blue-600 dark:text-blue-400 hover:underline text-sm">
                        {&node.domain}
                    </a>
                </td>
                <td class="px-4 py-3 whitespace-nowrap text-sm text-gray-900 dark:text-white">
                    {uptime}
                </td>
                <td class="px-4 py-3 whitespace-nowrap text-sm text-gray-900 dark:text-white">
                    {latency}
                </td>
                <td class="px-4 py-3 whitespace-nowrap">
                    <span class="inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium bg-green-100 text-green-800 dark:bg-green-900 dark:text-green-200">
                        {"Connected"}
                    </span>
                </td>
            </tr>
        }
    }
}

fn format_duration(seconds: u64) -> String {
    let days = seconds / 86400;
    let hours = (seconds % 86400) / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;

    if days > 0 {
        format!("{days}d {hours}h")
    } else if hours > 0 {
        format!("{hours}h {minutes}m")
    } else if minutes > 0 {
        format!("{minutes}m {secs}s")
    } else {
        format!("{secs}s")
    }
}

async fn fetch_nodes() -> Result<ListNodesResponse, String> {
    let window = window().ok_or("No window object")?;
    let location = window.location();

    // Get the base URL
    let protocol = location.protocol().map_err(|_| "Failed to get protocol")?;
    let host = location.host().map_err(|_| "Failed to get host")?;
    let base_url = format!("{protocol}//{host}");

    // Make request to /nodes endpoint
    let url = format!("{base_url}/nodes");

    let response = gloo_net::http::Request::get(&url)
        .send()
        .await
        .map_err(|e| format!("Network error: {e}"))?;

    if !response.ok() {
        return Err(format!("Server error: {}", response.status()));
    }

    response
        .json::<ListNodesResponse>()
        .await
        .map_err(|e| format!("Failed to parse response: {e}"))
}
