# Gate - Implementation Plan

## Development Philosophy

**Correctness over Speed**: "Fast as we can correctly" with AI assistance - prioritize getting it right over getting it fast.
**Continuous Integration**: Every task includes testing requirements. No feature is "complete" without tests.
**Security First**: Cryptographic foundations and security considerations are built early, not bolted on later.
**Web-First UI**: Web interface before native applications for maximum compatibility and rapid iteration.

## Phase 1: Foundations

### 1.1 Project Infrastructure
**Objective**: Establish workspace, dependencies, and development standards

**Tasks:**
1. **Set up Nix development environment** ✅ **COMPLETED**
   - [x] Create `flake.nix` with Rust toolchain, development tools, and dependencies
   - [x] Configure development shell with proper environment variables
   - [x] Document Nix installation and usage in development workflow
   - [x] **Testing**: Verify `nix develop` provides complete development environment
   - [x] **Documentation**: Add Nix usage instructions to relevant crate READMEs

2. **Create Cargo workspace structure** ✅ **COMPLETED**
   - [x] Set up `Cargo.toml` with workspace members and shared configuration
   - [x] Create core crate (hellas-gate-core) with workspace inheritance
   - [x] Set up `rust-toolchain.toml` for consistent Rust version across Nix and non-Nix environments
   - [x] **Testing**: Verify all crates compile with `cargo check --workspace`

3. **Configure development tooling**
   - Set up `.cargo/config.toml` with common flags and target configurations
   - Configure `clippy.toml` with project-specific lints
   - Set up `rustfmt.toml` for code formatting standards
   - Add pre-commit hooks for formatting and linting
   - **Testing**: Run `cargo fmt --check` and `cargo clippy` in CI

4. **Establish logging infrastructure**
   - Integrate `tracing` and `tracing-subscriber` across all crates
   - Create `crates/core/src/logging.rs` with structured logging setup
   - Configure different log outputs: console (development), file (production), JSON (monitoring)
   - **Testing**: Unit tests for log formatting and filtering

5. **Set up CI/CD pipeline** ✅ **COMPLETED**
   - [x] GitHub Actions workflow for testing on macOS, Linux, Windows
   - [x] Automated testing, linting, and formatting checks
   - [x] Security scanning with `cargo-audit`
   - [x] **Testing**: Verify CI passes on clean repository

### 1.2 Core Types and Cryptography
**Objective**: Define fundamental data structures and security primitives

**Tasks:**
1. **Implement identity and key management**
   ```rust
   // crates/core/src/identity.rs
   pub struct Identity {
       pub private_key: Ed25519PrivateKey,
       pub public_key: Ed25519PublicKey,
   }

   impl Identity {
       pub fn generate() -> Self { /* ... */ }
       pub fn from_private_key(key: &[u8]) -> Result<Self> { /* ... */ }
       pub fn sign(&self, message: &[u8]) -> Signature { /* ... */ }
       pub fn verify(public_key: &PublicKey, message: &[u8], signature: &Signature) -> bool { /* ... */ }
   }
   ```
   - Use `ed25519-dalek` for cryptographic operations
   - Implement secure key generation with OS entropy
   - Add key serialization/deserialization (PEM format)
   - **Testing**: Key generation, signing, verification, round-trip serialization

2. **Define node identity and addressing**
   ```rust
   // crates/core/src/node.rs
   #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
   pub struct NodeId(pub [u8; 32]); // Ed25519 public key bytes

   impl NodeId {
       pub fn from_public_key(key: &Ed25519PublicKey) -> Self { /* ... */ }
       pub fn to_hex(&self) -> String { /* ... */ }
       pub fn from_hex(hex: &str) -> Result<Self> { /* ... */ }
   }
   ```
   - Implement Display trait for human-readable node IDs
   - Add validation for node ID format
   - **Testing**: Conversion functions, validation, display formatting

3. **Create protocol message types**
   ```rust
   // crates/core/src/protocol.rs
   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct Message {
       pub version: u8,
       pub id: MessageId,
       pub timestamp: u64,
       pub payload: MessagePayload,
       pub signature: Option<Signature>, // None for unsigned control messages
   }

   pub type MessageId = [u8; 16]; // UUID v4 bytes

   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub enum MessagePayload {
       Handshake {
           node_id: NodeId,
           protocol_version: u8,
           capabilities: Capabilities,
       },
       HandshakeResponse {
           accepted: bool,
           reason: Option<String>,
           capabilities: Option<Capabilities>,
       },
       CapabilityRequest,
       CapabilityResponse { capabilities: Capabilities },
       Ping { nonce: u64 },
       Pong { nonce: u64 },
       HttpProxyStart { target_path: String },
       Error { code: u32, message: String },
   }
   ```
   - Implement message signing and verification
   - Add message correlation for request/response matching
   - **Testing**: Message serialization, signing verification, correlation logic

4. **Define configuration schema**
   ```rust
   // crates/core/src/config.rs
   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct Configuration {
       pub node: NodeConfig,
       pub network: NetworkConfig,
       pub providers: Vec<ProviderConfig>,
       pub security: SecurityConfig,
       pub web: WebConfig,
   }

   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct NodeConfig {
       pub data_dir: PathBuf,
       pub log_level: String, // "trace", "debug", "info", "warn", "error"
       pub identity_file: PathBuf, // Path to private key file
   }

   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct NetworkConfig {
       pub p2p_port: u16,           // Default: 41145
       pub http_port: u16,          // Default: 31145
       pub control_port: u16,       // Default: 8145
       pub bind_address: IpAddr,    // Default: 127.0.0.1
       pub trusted_peers: Vec<NodeId>,
       pub max_connections: usize,  // Default: 100
       pub connection_timeout: Duration, // Default: 30s
   }
   ```
   - Implement configuration validation with detailed error messages
   - Add configuration migration for version upgrades
   - Support environment variable overrides
   - **Testing**: Validation logic, migration paths, environment overrides

## Phase 2: P2P Networking

### 2.1 P2P Protocol Foundation ✅ **API COMPLETE**
**Objective**: Multi-stream P2P architecture with high-level API

**Completed:**
- Multi-stream protocol design (control, inference, SNI proxy)
- P2PNode with clean external API (`send_chat_completion`, `open_sni_proxy`, etc.)
- Protocol message types and serialization
- Connection management with per-peer state

**Still TODO:**
- [ ] **Actual stream implementation**: Control stream handshake, message sending/receiving
- [ ] **Request correlation**: Match requests to responses across streams
- [ ] **Error handling**: Stream failures, timeouts, retries
- [ ] **Stream lifecycle**: Proper opening/closing of typed streams

**Impact on later phases**:
- HTTP server integrates with `P2PNode` high-level API instead of raw streams
- Request routing simplified: `p2p_node.send_chat_completion(peer_id, request).await?`
- Provider integration needs P2P capability updates
       }
   }
   ```
   - Configurable ping intervals and timeouts
   - Automatic peer disconnection on missed pongs
   - Connection quality metrics collection
   - **Testing**: Ping/pong exchange, timeout handling, disconnection logic

## Phase 3: HTTP API and Provider Integration

### 3.1 HTTP Server Implementation **UPDATED FOR P2P API**
**Objective**: OpenAI-compatible API server integrated with P2PNode

**Updated approach**: HTTP server uses P2PNode high-level API for routing
```rust
// crates/daemon/src/http_server.rs
pub struct HttpServer {
    router: Router,
    provider_manager: Arc<ProviderManager>,
    p2p_node: Arc<P2PNode>, // ✅ Uses new high-level API
    config: HttpConfig,
}

// Updated request routing
pub async fn chat_completions(
    State(app_state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ChatCompletionRequest>,
) -> Result<Response, AppError> {
    // Check for target peer header
    if let Some(target_peer) = headers.get("X-Target-Node") {
        let peer_id = parse_node_id(target_peer)?;
        // ✅ Simple P2P call - complexity hidden
        let response = app_state.p2p_node
            .send_chat_completion(peer_id, request)
            .await?;
        return Ok(Json(response).into_response());
    }

    // Handle locally via provider manager
    app_state.provider_manager.handle_chat_completion(request).await
}
```

**Simplified by P2P refactor**: No manual stream management, just high-level calls
   - **Testing**: HTTP endpoint behavior, OpenAI compatibility, error responses

2. **Implement chat completions endpoint**
   ```rust
   // crates/daemon/src/handlers/chat.rs
   #[derive(Debug, Deserialize)]
   pub struct ChatCompletionRequest {
       pub model: String,
       pub messages: Vec<ChatMessage>,
       pub temperature: Option<f32>,
       pub max_tokens: Option<u32>,
       pub stream: Option<bool>,
       pub top_p: Option<f32>,
       // ... other OpenAI parameters
   }

   pub async fn chat_completions(
       State(app_state): State<AppState>,
       Json(request): Json<ChatCompletionRequest>,
   ) -> Result<Response, AppError> {
       // 1. Validate request parameters
       // 2. Determine target provider (local vs remote)
       // 3. Route request and handle response
       // 4. Return OpenAI-compatible response or stream
   }
   ```
   - Full OpenAI parameter support
   - Streaming via Server-Sent Events
   - Request validation with helpful error messages
   - **Testing**: Parameter validation, streaming behavior, OpenAI compatibility

3. **Add models endpoint and health checking**
   ```rust
   // crates/daemon/src/handlers/models.rs
   #[derive(Debug, Serialize)]
   pub struct ModelInfo {
       pub id: String,
       pub object: String, // "model"
       pub created: u64,   // Unix timestamp
       pub owned_by: String,
       pub permission: Vec<ModelPermission>,
       pub provider: String, // Extension: "ollama", "lmstudio", "remote"
       pub node_id: Option<NodeId>, // Extension: for remote models
   }

   pub async fn list_models(State(app_state): State<AppState>) -> Result<Json<ModelsResponse>, AppError> {
       // 1. Collect models from local providers
       // 2. Collect models from connected peers
       // 3. Return consolidated list
   }
   ```
   - Aggregate models from local and remote providers
   - Include provider and location information
   - Cache model lists with TTL
   - **Testing**: Model aggregation, caching, provider failures

### 3.2 Provider Integration
**Objective**: Connect to local inference engines (Ollama, LM Studio)

**Tasks:**
1. **Create provider abstraction layer**
   ```rust
   // crates/daemon/src/providers/mod.rs
   #[async_trait]
   pub trait InferenceProvider: Send + Sync {
       async fn health_check(&self) -> Result<ProviderHealth>;
       async fn list_models(&self) -> Result<Vec<ModelInfo>>;
       async fn chat_completion(&self, request: ChatCompletionRequest) -> Result<ChatCompletionResponse>;
       async fn chat_completion_stream(&self, request: ChatCompletionRequest) -> Result<Pin<Box<dyn Stream<Item = Result<ChatCompletionChunk>>>>>;
   }

   #[derive(Debug)]
   pub struct ProviderHealth {
       pub status: HealthStatus,
       pub response_time: Duration,
       pub available_models: usize,
       pub error: Option<String>,
   }

   pub enum HealthStatus {
       Healthy,
       Degraded,
       Unhealthy,
   }
   ```
   - Common interface for all provider types
   - Health monitoring and error tracking
   - Streaming response support
   - **Testing**: Provider interface compliance, health monitoring, error handling

2. **Implement Ollama provider integration**
   ```rust
   // crates/daemon/src/providers/ollama.rs
   pub struct OllamaProvider {
       client: reqwest::Client,
       base_url: Url,
       timeout: Duration,
       model_cache: Arc<RwLock<Vec<ModelInfo>>>,
       last_health_check: Arc<RwLock<Option<SystemTime>>>,
   }

   impl OllamaProvider {
       pub async fn new(base_url: Url) -> Result<Self> {
           // Initialize client, verify connection
       }

       async fn discover_models(&self) -> Result<Vec<ModelInfo>> {
           // GET /api/tags to list available models
       }

       async fn transform_request(&self, request: ChatCompletionRequest) -> Result<OllamaRequest> {
           // Convert OpenAI format to Ollama format
       }
   }
   ```
   - Automatic Ollama discovery on standard ports (11434)
   - Request/response format transformation
   - Model caching with periodic refresh
   - **Testing**: Ollama API compatibility, format transformation, discovery

3. **Add LM Studio provider support**
   ```rust
   // crates/daemon/src/providers/lmstudio.rs
   pub struct LMStudioProvider {
       client: reqwest::Client,
       base_url: Url,
       // Similar structure to OllamaProvider
   }

   impl InferenceProvider for LMStudioProvider {
       // LM Studio uses OpenAI-compatible API, so minimal transformation needed
   }
   ```
   - LM Studio discovery on standard ports (1234)
   - OpenAI-compatible API (minimal transformation)
   - Model availability detection
   - **Testing**: LM Studio integration, API compatibility

4. **Create provider manager and auto-discovery**
   ```rust
   // crates/daemon/src/providers/manager.rs
   pub struct ProviderManager {
       providers: Arc<RwLock<HashMap<String, Arc<dyn InferenceProvider>>>>,
       discovery_interval: Duration,
       health_check_interval: Duration,
   }

   impl ProviderManager {
       pub async fn start_discovery(&self) -> Result<()> {
           // Periodically scan for new providers on common ports
           // Add/remove providers based on availability
       }

       pub async fn route_request(&self, model: &str, request: ChatCompletionRequest) -> Result<ChatCompletionResponse> {
           // Find provider that supports the requested model
           // Route request with load balancing
       }
   }
   ```
   - Automatic provider discovery and health monitoring
   - Load balancing across multiple providers
   - Failover and retry logic
   - **Testing**: Discovery mechanisms, load balancing, failover behavior

## Phase 4: Web Frontend and Control Interface

### 4.1 RPC Interface Implementation
**Objective**: JSON-RPC control interface for daemon management

**Tasks:**
1. **Define RPC method signatures**
   ```rust
   // crates/rpc/src/methods.rs
   pub struct RpcMethods {
       daemon_handle: Arc<DaemonHandle>,
       p2p_node: Arc<P2PNode>,
       provider_manager: Arc<ProviderManager>,
   }

   impl RpcMethods {
       // Node management
       pub async fn node_status(&self) -> Result<NodeStatus>;
       pub async fn node_shutdown(&self) -> Result<()>;
       pub async fn node_restart(&self) -> Result<()>;

       // Peer management
       pub async fn add_peer(&self, node_id: NodeId, address: Option<SocketAddr>) -> Result<()>;
       pub async fn remove_peer(&self, node_id: NodeId) -> Result<()>;
       pub async fn list_peers(&self) -> Result<Vec<PeerInfo>>;
       pub async fn peer_info(&self, node_id: NodeId) -> Result<PeerInfo>;

       // Configuration
       pub async fn get_config(&self) -> Result<Configuration>;
       pub async fn update_config(&self, config: Configuration) -> Result<()>;
       pub async fn validate_config(&self, config: Configuration) -> Result<Vec<ValidationError>>;

       // Providers
       pub async fn list_providers(&self) -> Result<Vec<ProviderInfo>>;
       pub async fn add_provider(&self, provider_config: ProviderConfig) -> Result<()>;
       pub async fn test_provider(&self, url: Url) -> Result<ProviderHealth>;

       // Monitoring
       pub async fn get_metrics(&self) -> Result<MetricsSnapshot>;
       pub async fn request_history(&self, limit: Option<usize>) -> Result<Vec<RequestInfo>>;
       pub async fn get_logs(&self, level: Option<String>, limit: Option<usize>) -> Result<Vec<LogEntry>>;

       // Identity management
       pub async fn generate_keypair(&self) -> Result<Identity>;
       pub async fn export_identity(&self) -> Result<String>; // PEM format
       pub async fn import_identity(&self, pem_data: String) -> Result<()>;
   }
   ```
   - Comprehensive daemon control and monitoring
   - Configuration management with validation
   - Real-time status and metrics access
   - **Testing**: All RPC methods, parameter validation, error handling

2. **Implement JSON-RPC server with WebSocket support**
   ```rust
   // crates/rpc/src/server.rs
   pub struct RpcServer {
       methods: Arc<RpcMethods>,
       websocket_connections: Arc<RwLock<HashMap<ConnectionId, WebSocketSender>>>,
   }

   impl RpcServer {
       pub async fn handle_http_request(&self, request: JsonRpcRequest) -> JsonRpcResponse {
           // Handle single request/response
       }

       pub async fn handle_websocket(&self, socket: WebSocket) -> Result<()> {
           // Handle persistent connection with real-time updates
           // Send events: peer_connected, peer_disconnected, request_completed, etc.
       }

       pub async fn broadcast_event(&self, event: RpcEvent) -> Result<()> {
           // Send real-time updates to all connected WebSocket clients
       }
   }

   #[derive(Debug, Serialize)]
   pub enum RpcEvent {
       PeerConnected { node_id: NodeId, capabilities: Capabilities },
       PeerDisconnected { node_id: NodeId, reason: String },
       RequestCompleted { id: String, duration: Duration, success: bool },
       ProviderStatusChanged { name: String, status: HealthStatus },
       ConfigurationChanged,
   }
   ```
   - JSON-RPC 2.0 compliance with batch support
   - WebSocket for real-time event streaming
   - Connection management and cleanup
   - **Testing**: RPC protocol compliance, WebSocket behavior, event delivery

3. **Add authentication and rate limiting**
   ```rust
   // crates/rpc/src/auth.rs
   pub struct RpcAuth {
       api_tokens: HashSet<String>,
       rate_limiter: RateLimiter,
       require_auth: bool,
   }

   impl RpcAuth {
       pub fn validate_request(&self, headers: &HeaderMap) -> Result<()> {
           // Check API token if authentication required
           // Apply rate limiting per client
       }

       pub fn generate_token(&self) -> String {
           // Generate secure random API token
       }
   }
   ```
   - Optional authentication for security-sensitive deployments
   - Rate limiting to prevent abuse
   - Token generation and management
   - **Testing**: Authentication flows, rate limiting behavior, token validation

### 4.2 Yew Web Frontend
**Objective**: Browser-based interface for daemon management

**Tasks:**
1. **Set up Yew project structure and build system**
   ```rust
   // crates/web/src/main.rs
   use yew::prelude::*;
   use yew_router::prelude::*;

   #[derive(Clone, Routable, PartialEq)]
   enum Route {
       #[at("/")]
       Dashboard,
       #[at("/peers")]
       Peers,
       #[at("/config")]
       Configuration,
       #[at("/providers")]
       Providers,
       #[at("/logs")]
       Logs,
       #[at("/setup")]
       Setup,
   }

   #[function_component(App)]
   fn app() -> Html {
       html! {
           <BrowserRouter>
               <Switch<Route> render={switch} />
           </BrowserRouter>
       }
   }
   ```
   - Yew Router for single-page application navigation
   - WebAssembly build configuration
   - Asset bundling and optimization
   - **Testing**: Build process, routing behavior, WebAssembly functionality

2. **Create dashboard and status components**
   ```rust
   // crates/web/src/components/dashboard.rs
   #[derive(Properties, PartialEq)]
   pub struct DashboardProps {}

   #[function_component(Dashboard)]
   pub fn dashboard(_props: &DashboardProps) -> Html {
       let node_status = use_state(|| None::<NodeStatus>);
       let peer_count = use_state(|| 0usize);
       let recent_requests = use_state(|| Vec::<RequestInfo>::new());

       // WebSocket connection for real-time updates
       let websocket = use_websocket("ws://localhost:8145/rpc/ws");

       html! {
           <div class="dashboard">
               <StatusCard node_status={(*node_status).clone()} />
               <PeerSummary count={*peer_count} />
               <RecentRequests requests={(*recent_requests).clone()} />
               <MetricsChart />
           </div>
       }
   }
   ```
   - Real-time status display with WebSocket updates
   - Key metrics visualization (requests/sec, latency, etc.)
   - Recent activity and error tracking
   - **Testing**: Component rendering, WebSocket integration, real-time updates

3. **Implement peer management interface**
   ```rust
   // crates/web/src/components/peers.rs
   #[function_component(PeerManager)]
   pub fn peer_manager() -> Html {
       let peers = use_state(|| Vec::<PeerInfo>::new());
       let add_peer_modal = use_state(|| false);

       let add_peer = {
           let peers = peers.clone();
           Callback::from(move |peer_info: PeerInfo| {
               // Call RPC method to add peer
               // Update local state
           })
       };

       html! {
           <div class="peer-manager">
               <PeerList peers={(*peers).clone()} />
               <AddPeerButton onclick={move |_| add_peer_modal.set(true)} />
               if *add_peer_modal {
                   <AddPeerModal on_add={add_peer} on_cancel={/* ... */} />
               }
           </div>
       }
   }
   ```
   - Add/remove peers with address validation
   - Connection status monitoring
   - Peer capability display
   - **Testing**: Peer operations, modal behavior, validation

4. **Create configuration management interface**
   ```rust
   // crates/web/src/components/config.rs
   #[function_component(ConfigManager)]
   pub fn config_manager() -> Html {
       let config = use_state(|| None::<Configuration>);
       let validation_errors = use_state(|| Vec::<ValidationError>::new());
       let is_dirty = use_state(|| false);

       let save_config = {
           let config = config.clone();
           Callback::from(move |_| {
               // Validate configuration
               // Save via RPC if valid
               // Show success/error feedback
           })
       };

       html! {
           <div class="config-manager">
               <ConfigForm
                   config={(*config).clone()}
                   errors={(*validation_errors).clone()}
                   on_change={/* update config state */}
               />
               <SaveButton onclick={save_config} disabled={!*is_dirty} />
           </div>
       }
   }
   ```
   - Live configuration editing with validation
   - Real-time validation feedback
   - Unsaved changes warning
   - **Testing**: Form validation, save/load operations, change detection

5. **Add setup wizard for first-time users**
   ```rust
   // crates/web/src/components/setup.rs
   #[derive(Debug, Clone)]
   pub struct SetupState {
       pub step: SetupStep,
       pub identity_choice: IdentityChoice,
       pub generated_identity: Option<Identity>,
       pub discovered_providers: Vec<ProviderInfo>,
       pub selected_providers: HashSet<String>,
       pub trust_settings: TrustSettings,
       pub public_endpoint: Option<PublicEndpointConfig>,
   }

   #[derive(Debug, Clone, PartialEq)]
   pub enum SetupStep {
       Welcome,           // Introduction and overview
       Identity,          // Generate new vs import existing identity
       Providers,         // Auto-discovered providers, selection
       TrustSettings,     // Who can access this node (empty = private)
       PublicEndpoint,    // Optional HTTPS endpoint setup
       Review,           // Review all settings before save
       Complete,         // Setup finished, start daemon
   }

   pub enum IdentityChoice {
       Generate,          // Generate new keypair
       Import(String),    // Import from PEM data
       LoadExisting,      // Use existing identity file
   }

   pub struct TrustSettings {
       pub mode: TrustMode,
       pub trusted_keys: Vec<NodeId>,
   }

   pub enum TrustMode {
       Private,           // No external access
       Friends,           // Manually added trusted keys
       Public,            // Accept requests from anyone (not recommended)
   }
   ```
   **Setup wizard questions and flow:**
   - **Step 1 - Welcome**: Explain Private Gate and setup process
   - **Step 2 - Identity**:
     - "Generate new identity" (recommended for new users)
     - "Import existing identity" (for users with existing keys)
     - "Use existing identity file" (if identity.key already exists)
   - **Step 3 - Providers**:
     - Auto-scan for Ollama (port 11434), LM Studio (port 1234)
     - Show discovered providers with model counts
     - Allow manual provider addition
     - "Which providers should be enabled?" (checkboxes)
   - **Step 4 - Trust Settings**:
     - "Who should be allowed to use your compute?"
     - Options: "Just me" (private), "Specific friends" (manual keys), "Public" (not recommended)
     - If "friends" selected: interface to add trusted public keys
   - **Step 5 - Public Endpoint** (optional):
     - "Do you want a public HTTPS endpoint?"
     - If yes: domain preference, Let's Encrypt email
     - Warning about exposing compute publicly
   - **Step 6 - Review**: Show all settings before saving
   - **Step 7 - Complete**: Save config, start daemon, redirect to dashboard
   - **Testing**: Multi-step wizard flow, data persistence, validation at each step

## Phase 5: Request Routing and Processing

### 5.1 Request Router Implementation
**Objective**: Route requests between local and remote providers with load balancing

**Tasks:**
1. **Implement request routing logic**
   ```rust
   // crates/daemon/src/router.rs
   pub struct RequestRouter {
       local_providers: Arc<ProviderManager>,
       p2p_node: Arc<P2PNode>,
       routing_strategy: RoutingStrategy,
       load_balancer: LoadBalancer,
   }

   pub enum RoutingStrategy {
       LocalOnly,                    // Never route to remote peers
       PreferLocal,                  // Try local first, fallback to remote
       LoadBalance,                  // Distribute based on capacity
       ExplicitTarget(NodeId),       // Route to specific peer
   }

   impl RequestRouter {
       pub async fn route_request(&self, request: ChatCompletionRequest) -> Result<ChatCompletionResponse> {
           // 1. Determine routing strategy based on request and config
           // 2. Select target provider (local vs remote)
           // 3. Execute request with timeout and retry logic
           // 4. Return response or error
       }

       async fn select_target(&self, model: &str) -> Result<RoutingTarget> {
           // Consider model availability, load factors, latency
       }
   }

   pub enum RoutingTarget {
       Local(String),               // Local provider name
       Remote(NodeId, String),      // Remote node + model name
   }
   ```
   - Model-aware routing based on provider capabilities
   - Load balancing with peer capacity consideration
   - Explicit targeting via request headers
   - **Testing**: Routing decisions, load balancing, target selection

2. **Add request correlation and tracking**
   ```rust
   // crates/daemon/src/tracking.rs
   #[derive(Debug, Clone)]
   pub struct RequestInfo {
       pub id: String,
       pub timestamp: SystemTime,
       pub model: String,
       pub provider: String,
       pub source: RequestSource,
       pub target: RoutingTarget,
       pub status: RequestStatus,
       pub latency: Option<Duration>,
       pub tokens: Option<TokenUsage>,
   }

   pub enum RequestSource {
       Local(IpAddr),               // Local HTTP client
       Remote(NodeId),              // Remote peer
       WebUI,                       // Web frontend
   }

   pub enum RequestStatus {
       Pending,
       InProgress,
       Completed,
       Failed(String),
       Timeout,
   }

   pub struct TokenUsage {
       pub prompt_tokens: u32,
       pub completion_tokens: u32,
       pub total_tokens: u32,
   }
   ```
   - Comprehensive request tracking and metrics
   - Token usage monitoring for cost estimation
   - Request source identification
   - **Testing**: Request tracking, metrics collection, token counting

3. **Implement timeout and retry logic**
   ```rust
   // crates/daemon/src/retry.rs
   pub struct RetryConfig {
       pub max_attempts: u32,       // Default: 3
       pub initial_timeout: Duration, // Default: 30s
       pub backoff_multiplier: f32, // Default: 2.0
       pub max_timeout: Duration,   // Default: 120s
   }

   pub struct RetryManager {
       config: RetryConfig,
   }

   impl RetryManager {
       pub async fn execute_with_retry<F, T>(&self, operation: F) -> Result<T>
       where
           F: Fn() -> Pin<Box<dyn Future<Output = Result<T>> + Send>>,
       {
           // Exponential backoff retry with jitter
           // Circuit breaker for repeated failures
       }
   }
   ```
   - Exponential backoff with jitter
   - Circuit breaker pattern for failing providers
   - Per-provider retry configuration
   - **Testing**: Retry behavior, backoff timing, circuit breaker logic

### 5.2 HTTP Proxy Implementation
**Objective**: Transparent HTTP proxying over P2P connections

**Tasks:**
1. **Create HTTP-over-P2P proxy**
   ```rust
   // crates/daemon/src/proxy.rs
   pub struct HttpProxy {
       p2p_node: Arc<P2PNode>,
   }

   impl HttpProxy {
       pub async fn proxy_request(&self, target: NodeId, request: hyper::Request<hyper::body::Incoming>) -> Result<hyper::Response<hyper::body::Incoming>> {
           // 1. Open P2P stream to target node
           // 2. Send HTTP request over stream
           // 3. Read HTTP response from stream
           // 4. Return response to client
       }

       pub async fn handle_incoming_proxy(&self, stream: impl AsyncRead + AsyncWrite + Unpin) -> Result<()> {
           // 1. Read HTTP request from P2P stream
           // 2. Forward to local provider
           // 3. Stream response back over P2P
       }
   }
   ```
   - Raw HTTP forwarding over Iroh streams
   - Streaming request/response support
   - Connection pooling for efficiency
   - **Testing**: HTTP proxy behavior, streaming support, connection management

2. **Add request authentication and authorization**
   ```rust
   // crates/daemon/src/auth.rs
   pub struct RequestAuthenticator {
       trusted_peers: HashSet<NodeId>,
       rate_limiter: RateLimiter,
   }

   impl RequestAuthenticator {
       pub fn authenticate_peer(&self, peer_id: NodeId) -> Result<()> {
           // Verify peer is in trusted list
           // Check rate limits
       }

       pub fn authorize_request(&self, peer_id: NodeId, request: &ChatCompletionRequest) -> Result<()> {
           // Check model access permissions
           // Validate request parameters
       }
   }
   ```
   - Trust-based peer authentication
   - Rate limiting per peer
   - Model-specific access control
   - **Testing**: Authentication logic, rate limiting, authorization rules

## Phase 6: CLI Tool

### 6.1 Command-Line Interface
**Objective**: Comprehensive CLI for daemon management

**Tasks:**
1. **Implement core CLI commands**
   ```rust
   // crates/cli/src/main.rs
   use clap::{Parser, Subcommand};

   #[derive(Parser)]
   #[command(name = "gate")]
   #[command(about = "Private Gate P2P AI Compute Network")]
   struct Cli {
       #[command(subcommand)]
       command: Commands,

       /// Config file path
       #[arg(long, global = true)]
       config: Option<PathBuf>,

       /// Log level
       #[arg(long, global = true, value_enum)]
       log_level: Option<LogLevel>,
   }

   #[derive(Subcommand)]
   enum Commands {
       /// Start the daemon
       Start {
           /// Run in background
           #[arg(long)]
           daemon: bool,

           /// Bind to specific interface
           #[arg(long)]
           bind: Option<IpAddr>,
       },

       /// Stop the daemon
       Stop,

       /// Show daemon status
       Status,

       /// Peer management
       Peers {
           #[command(subcommand)]
           command: PeerCommands,
       },

       /// Configuration management
       Config {
           #[command(subcommand)]
           command: ConfigCommands,
       },

       /// Interactive setup wizard
       Setup,

       /// View logs
       Logs {
           /// Follow log output
           #[arg(short, long)]
           follow: bool,

           /// Log level filter
           #[arg(long)]
           level: Option<LogLevel>,

           /// Number of lines to show
           #[arg(short, long)]
           lines: Option<usize>,
       },
   }
   ```
   - Comprehensive command structure with subcommands
   - Global options for config and logging
   - Help documentation for all commands
   - **Testing**: Command parsing, help output, option validation

2. **Add peer management commands**
   ```rust
   // crates/cli/src/commands/peers.rs
   #[derive(Subcommand)]
   enum PeerCommands {
       /// Add a new peer
       Add {
           /// Peer node ID (public key)
           node_id: String,

           /// Peer address (optional)
           #[arg(long)]
           address: Option<SocketAddr>,

           /// Mark as trusted
           #[arg(long)]
           trusted: bool,
       },

       /// Remove a peer
       Remove {
           /// Peer node ID
           node_id: String,
       },

       /// List all peers
       List {
           /// Show only connected peers
           #[arg(long)]
           connected: bool,

           /// Output format
           #[arg(long, value_enum)]
           format: Option<OutputFormat>,
       },

       /// Show detailed peer information
       Info {
           /// Peer node ID
           node_id: String,
       },

       /// Test connection to peer
       Test {
           /// Peer node ID
           node_id: String,
       },
   }

   #[derive(Clone, ValueEnum)]
   enum OutputFormat {
       Table,
       Json,
       Yaml,
   }
   ```
   - Full peer lifecycle management
   - Connection testing and diagnostics
   - Multiple output formats for scripting
   - **Testing**: Peer operations, output formatting, error handling

3. **Create interactive setup wizard**
   ```rust
   // crates/cli/src/setup.rs
   pub struct SetupWizard {
       config: Configuration,
   }

   impl SetupWizard {
       pub async fn run(&mut self) -> Result<Configuration> {
           self.welcome_screen()?;
           self.setup_identity().await?;
           self.discover_providers().await?;
           self.configure_trust().await?;
           self.setup_public_endpoint().await?;
           self.review_configuration()?;
           self.save_configuration().await?;
           Ok(self.config.clone())
       }

       async fn setup_identity(&mut self) -> Result<()> {
           println!("Setting up node identity...");

           let choice = Select::new("Choose identity option:")
               .item("Generate new identity (recommended)", "generate")
               .item("Import existing identity", "import")
               .item("Use existing identity file", "existing")
               .prompt()?;

           match choice {
               "generate" => {
                   let identity = Identity::generate();
                   println!("Generated new identity: {}", identity.public_key.to_hex());
                   self.config.node.identity = identity;
               },
               "import" => {
                   let pem_data = Input::new("Enter PEM-encoded private key:").prompt()?;
                   let identity = Identity::from_pem(&pem_data)?;
                   self.config.node.identity = identity;
               },
               "existing" => {
                   // Check for existing identity file
               },
           }

           Ok(())
       }

       async fn discover_providers(&mut self) -> Result<()> {
           println!("Discovering local AI providers...");

           // Scan common ports for Ollama, LM Studio
           let discovered = self.scan_for_providers().await?;

           if discovered.is_empty() {
               println!("No providers found. You can add them manually later.");
           } else {
               println!("Found {} providers:", discovered.len());
               for provider in &discovered {
                   println!("  - {} at {} ({} models)", provider.name, provider.url, provider.models.len());
               }

               let selected = MultiSelect::new("Select providers to enable:")
                   .items(&discovered)
                   .prompt()?;

               self.config.providers = selected;
           }

           Ok(())
       }
   }
   ```
   - Interactive prompts with validation
   - Provider auto-discovery and selection
   - Configuration review before saving
   - **Testing**: Wizard flow, input validation, configuration generation

## Phase 7: Relay Server and Public HTTPS

### 7.1 DNS and Certificate Management
**Objective**: Automatic subdomain provisioning and SSL certificate generation with robust error handling

**Tasks:**
1. **Research and select TLS termination library**
   - **Requirements**: Support for SNI extraction, custom certificate loading, raw TLS byte handling
   - **Likely candidate**: `rustls` with `tokio-rustls` for async support
   - **Security validation needed**: Certificate validation, private key protection, TLS version support
   - **Testing requirements**: Generate test certificates, simulate browser connections, validate SNI extraction
   - **Deliverable**: Library choice documented with rationale and security analysis
   - **Testing**: Unit tests for TLS termination, integration tests with real certificates

2. **Implement Cloudflare DNS integration**
   ```rust
   // crates/relay/src/dns.rs
   pub struct CloudflareManager {
       client: reqwest::Client,
       api_token: String,
       zone_id: String,
       base_domain: String, // "private.hellas.ai"
   }

   impl CloudflareManager {
       pub async fn provision_subdomain(&self, node_id: &NodeId) -> Result<String> {
           // 1. Generate subdomain: {node_id_hex}.private.hellas.ai
           // 2. Create A/AAAA records pointing to relay IPs
           // 3. Return full domain name
       }

       pub async fn create_dns_challenge(&self, domain: &str, token: &str) -> Result<()> {
           // Create TXT record for Let's Encrypt DNS challenge
       }

       pub async fn cleanup_subdomain(&self, domain: &str) -> Result<()> {
           // Remove DNS records when node disconnects
       }
   }
   ```
   - Subdomain generation from node IDs
   - Anycast IP management for relay fleet
   - DNS challenge support for Let's Encrypt
   - **Testing**: DNS record creation/deletion, challenge handling

2. **Add Let's Encrypt certificate generation with robust validation**
   ```rust
   // crates/relay/src/acme.rs
   pub struct AcmeManager {
       client: AcmeClient,
       dns_manager: Arc<CloudflareManager>,
       cert_storage: Arc<CertificateStorage>,
   }

   impl AcmeManager {
       pub async fn request_certificate(&self, domain: &str, node_id: NodeId) -> Result<Certificate> {
           // 1. Create ACME account if needed
           // 2. Request certificate for domain
           // 3. Complete DNS challenge via Cloudflare
           // 4. Validate DNS propagation before proceeding
           // 5. Store certificate for node
           // 6. Return certificate for node to use
       }

       pub async fn validate_dns_propagation(&self, domain: &str, challenge_token: &str) -> Result<()> {
           // Query multiple DNS servers to confirm challenge record exists
           // Wait for propagation with exponential backoff
       }

       pub async fn renew_certificate(&self, domain: &str) -> Result<Certificate> {
           // Automatic certificate renewal before expiry
       }
   }

   pub struct Certificate {
       pub domain: String,
       pub certificate_pem: String,
       pub private_key_pem: String,
       pub expires_at: SystemTime,
   }
   ```
   - **Error handling**: DNS propagation validation with multiple resolver checks
   - **Node validation**: Nodes perform periodic DNS checks of their own domains
   - **Loopback testing**: Nodes make HTTPS requests to themselves to validate certificate chain
   - **Retry logic**: Exponential backoff for DNS propagation delays
   - **Monitoring**: Certificate expiry tracking and proactive renewal
   - **Testing**: Certificate generation, renewal, validation, DNS propagation, loopback requests

### 7.2 SNI Proxy Implementation
**Objective**: Route HTTPS traffic to appropriate nodes via P2P

**Tasks:**
1. **Create SNI-based HTTPS proxy**
   ```rust
   // crates/relay/src/proxy.rs
   pub struct SniProxy {
       p2p_node: Arc<P2PNode>,
       node_registry: Arc<NodeRegistry>,
       active_connections: Arc<RwLock<HashMap<ConnectionId, ConnectionInfo>>>,
   }

   impl SniProxy {
       pub async fn handle_https_connection(&self, stream: TcpStream) -> Result<()> {
           // 1. Extract SNI from TLS ClientHello
           // 2. Look up node ID from domain
           // 3. Open P2P stream to target node
           // 4. Proxy raw TCP traffic bidirectionally
       }

       async fn extract_sni(&self, stream: &mut TcpStream) -> Result<String> {
           // Parse TLS ClientHello to extract SNI extension
       }

       async fn proxy_bidirectional(&self, client_stream: TcpStream, p2p_stream: impl AsyncRead + AsyncWrite) -> Result<()> {
           // Proxy data in both directions until connection closes
       }
   }

   pub struct NodeRegistry {
       nodes: RwLock<HashMap<String, NodeId>>, // domain -> node_id
   }
   ```
   - SNI extraction from TLS handshake
   - Bidirectional TCP proxying
   - Connection tracking and cleanup
   - **Testing**: SNI parsing, proxy behavior, connection management

2. **Add load balancing and failover**
   ```rust
   // crates/relay/src/loadbalancer.rs
   pub struct RelayLoadBalancer {
       relay_nodes: Vec<RelayNode>,
       health_checker: HealthChecker,
   }

   pub struct RelayNode {
       pub id: String,
       pub addresses: Vec<SocketAddr>,
       pub status: RelayStatus,
       pub load: f32,
   }

   pub enum RelayStatus {
       Healthy,
       Degraded,
       Unhealthy,
   }

   impl RelayLoadBalancer {
       pub fn select_relay(&self, client_ip: IpAddr) -> Result<&RelayNode> {
           // Geographic/network proximity selection
           // Load-based selection
           // Health status consideration
       }
   }
   ```
   - Multiple relay instances with anycast IPs
   - Geographic load balancing
   - Health monitoring and failover
   - **Testing**: Load balancing decisions, failover behavior

## Phase 8: Testing and Quality Assurance (Continuous)

### 8.1 Comprehensive Testing Strategy
**Objective**: Ensure reliability and correctness across all components

**Testing approach integrated throughout development:**

1. **Unit Tests** (Written with each component):
   ```rust
   // Example: crates/core/tests/identity_tests.rs
   #[cfg(test)]
   mod tests {
       use super::*;

       #[test]
       fn test_identity_generation() {
           let identity = Identity::generate();
           assert_eq!(identity.public_key.as_bytes().len(), 32);
           assert_eq!(identity.private_key.as_bytes().len(), 32);
       }

       #[test]
       fn test_signing_and_verification() {
           let identity = Identity::generate();
           let message = b"test message";
           let signature = identity.sign(message);
           assert!(Identity::verify(&identity.public_key, message, &signature));
       }

       #[tokio::test]
       async fn test_p2p_handshake() {
           let node1 = create_test_node().await;
           let node2 = create_test_node().await;

           let capabilities = node1.handshake_with_peer(node2.node_id()).await.unwrap();
           assert!(capabilities.supported_models.len() > 0);
       }
   }
   ```

2. **Integration Tests** (Added as components are integrated):
   ```rust
   // tests/integration/p2p_communication.rs
   #[tokio::test]
   async fn test_end_to_end_inference_request() {
       // Set up two nodes with mock providers
       let node1 = setup_test_node_with_provider().await;
       let node2 = setup_test_node().await;

       // Connect nodes
       node2.connect_to_peer(node1.node_id()).await.unwrap();

       // Send inference request from node2 to node1
       let request = ChatCompletionRequest { /* ... */ };
       let response = node2.send_inference_request(node1.node_id(), request).await.unwrap();

       assert_eq!(response.model, "test-model");
       assert!(response.choices.len() > 0);
   }
   ```

3. **End-to-End Tests** (Added as major features complete):
   ```rust
   // tests/e2e/web_interface.rs
   use playwright::Playwright;

   #[tokio::test]
   async fn test_web_interface_setup_wizard() {
       // Start daemon in test mode
       let daemon = start_test_daemon().await;

       // Open browser and navigate to setup wizard
       let playwright = Playwright::initialize().await.unwrap();
       let browser = playwright.chromium().launcher().headless(true).launch().await.unwrap();
       let page = browser.new_page().await.unwrap();

       page.goto("http://localhost:8145/setup").await.unwrap();

       // Complete setup wizard
       page.click("button:text('Generate new identity')").await.unwrap();
       page.click("button:text('Next')").await.unwrap();
       // ... test each step

       // Verify configuration was saved
       let config = daemon.get_config().await.unwrap();
       assert!(config.node.identity.is_some());
   }
   ```

4. **Performance Tests** (Added during optimization phases):
   ```rust
   // benches/throughput.rs
   use criterion::{criterion_group, criterion_main, Criterion};

   fn benchmark_request_routing(c: &mut Criterion) {
       let rt = tokio::runtime::Runtime::new().unwrap();
       let router = rt.block_on(setup_test_router());

       c.bench_function("route_request", |b| {
           b.to_async(&rt).iter(|| async {
               let request = create_test_request();
               router.route_request(request).await.unwrap()
           })
       });
   }

   criterion_group!(benches, benchmark_request_routing);
   criterion_main!(benches);
   ```

### 8.2 Continuous Integration Pipeline

**GitHub Actions workflow** (`.github/workflows/ci.yml`):
```yaml
name: CI
on: [push, pull_request]

jobs:
  test:
    strategy:
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
        rust: [stable, beta]
    runs-on: ${{ matrix.os }}

    steps:
    - uses: actions/checkout@v4
    - uses: dtolnay/rust-toolchain@master
      with:
        toolchain: ${{ matrix.rust }}
        components: clippy, rustfmt

    - name: Cache cargo registry
      uses: actions/cache@v3
      with:
        path: ~/.cargo/registry
        key: ${{ runner.os }}-cargo-registry-${{ hashFiles('**/Cargo.lock') }}

    - name: Check formatting
      run: cargo fmt --all -- --check

    - name: Run clippy
      run: cargo clippy --workspace --all-targets -- -D warnings

    - name: Run unit tests
      run: cargo test --workspace

    - name: Run integration tests
      run: cargo test --test '*' --workspace

    - name: Build all binaries
      run: cargo build --workspace --release

    - name: Run end-to-end tests
      if: matrix.os == 'ubuntu-latest' && matrix.rust == 'stable'
      run: cargo test --test e2e --workspace
```

## Implementation Approach

### Development Phases

**Phase 1: Foundations**
- Project setup and core types
- Cryptographic foundations
- Basic configuration management

**Phase 2: P2P Networking**
- Iroh integration and peer discovery
- Message protocol implementation
- Connection management and keep-alive

**Phase 3: HTTP API**
- OpenAI-compatible endpoints
- Provider integration (Ollama, LM Studio)
- Request/response handling with streaming

**Phase 4: Web Frontend**
- JSON-RPC control interface
- Yew web application
- Real-time dashboard and management

**Phase 5: Request Routing**
- Load balancing and failover
- TLS-over-P2P proxying
- Request tracking and metrics

**Phase 6: CLI Tool**
- Command-line interface
- Interactive setup wizard
- Comprehensive management commands

**Phase 7: Relay System**
- DNS management and certificate generation
- SNI proxy implementation
- Public HTTPS endpoints

### MVP Definition

**Minimum Viable Product includes:**
- Local daemon with OpenAI-compatible API
- P2P networking between trusted nodes
- Web-based management interface
- Provider integration (Ollama/LM Studio)
- Basic request routing and load balancing
- Configuration management and setup wizard

**Post-MVP features:**
- Public HTTPS endpoints via relay system
- Mobile applications (iOS/Android)
- Advanced load balancing and routing
- Blockchain integration for payments/trust
- Catgrad compute graph support

### Implementation Notes

- **Timeline**: "Fast as we can correctly" - no artificial deadlines
- **AI assistance**: Leverage AI tools for rapid, correct implementation
- **Iterative approach**: Complete phases fully before moving to next
- **Continuous testing**: Each task includes comprehensive testing requirements
- **Documentation first**: Update docs before/during implementation to maintain accuracy
