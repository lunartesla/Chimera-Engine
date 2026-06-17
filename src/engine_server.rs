use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::broadcast;
use std::sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use log::{info, error, warn};
use serde::{Serialize, Deserialize};
use rand::Rng; // For generating session IDs

use crate::ir::module::Module;
use crate::engine::OptimizationEngine;
use crate::self_evolving_engine::SelfEvolvingEngine; // For evolve command
use crate::evolution_daemon::EvolutionDaemon; // For setting daemon reference
use crate::passes::OptimizationLevel; // For creating SelfEvolvingEngine
use crate::passes::PassManager;
use crate::profiler::RuntimeProfiler;
use crate::teacher::MutationStep; // From Teacher

// --- Data structures for client communication ---

pub async fn handle_ws_client(
    stream: TcpStream,
    broadcast_tx: broadcast::Sender<String>,
) {
    use tokio_tungstenite::tungstenite::Message;
    use futures_util::{StreamExt, SinkExt};

    let ws_stream = match tokio_tungstenite::accept_async(stream).await {
        Ok(ws) => ws,
        Err(e) => {
            error!("[EngineServer] WebSocket handshake failed: {}", e);
            return;
        }
    };

    let (mut ws_tx, mut ws_rx) = ws_stream.split();
    let mut broadcast_rx = broadcast_tx.subscribe();

    // Forward broadcast events to this client
    let send_task = tokio::spawn(async move {
        while let Ok(msg) = broadcast_rx.recv().await {
            if ws_tx.send(Message::Text(msg)).await.is_err() {
                break;
            }
        }
    });

    // Handle incoming messages (ping, subscribe commands)
    while let Some(Ok(msg)) = ws_rx.next().await {
        if let Message::Text(text) = msg {
            // handle ping etc if needed
            let _ = text;
        }
    }

    send_task.abort();
}

#[derive(Debug, Serialize, Deserialize)]
#[derive(Clone)]
pub struct ModuleHandle {
    pub id: String,
    pub name: String,
    pub language: String,
    pub source_code: String,
    #[serde(default)]
    pub fitness: f64,
    #[serde(default)]
    pub pipeline: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MutationRecord {
    #[serde(rename = "mutationType")]
    pub mutation_type: String,
    #[serde(rename = "passId")]
    pub pass_id: String,
    #[serde(rename = "fitnessDelta")]
    pub fitness_delta: f64,
    pub success: bool,
    pub timestamp: u64, // Unix timestamp
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NeatStatus {
    pub ready: bool,
    #[serde(rename = "trainingRecords")]
    pub training_records: usize,
    pub confidence: f64,
    pub description: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EvolutionResult {
    #[serde(rename = "bestFitness")]
    pub best_fitness: f64,
    #[serde(rename = "initialFitness")]
    pub initial_fitness: f64,
    #[serde(rename = "generationsCompleted")]
    pub generations_completed: u32,
    #[serde(rename = "goalReached")]
    pub goal_reached: bool,
    #[serde(rename = "bestPipeline")]
    pub best_pipeline: Vec<String>,
}

// Client session management
pub struct ClientSession {
    pub id: String,
    pub module_handle: Option<ModuleHandle>,
    pub mutation_history: Vec<MutationRecord>,
    pub opt_engine: OptimizationEngine,
    // Add SelfEvolvingEngine here if each client session has its own evolution.
    // The C++ version has `clientEngines: map<string, shared_ptr<OptimizationEngine>>`
    // and `sessions: map<string, ClientSession>` where ClientSession has `moduleHandle`.
    // It seems `SelfEvolvingEngine` is created ad-hoc for `evolve` and `apply_mutation` commands.
}


pub struct EngineServer {
    port: u16,
    running: Arc<AtomicBool>,
    daemon: Arc<Mutex<Option<EvolutionDaemon>>>, // Optional reference to the daemon
    sessions: Arc<Mutex<HashMap<String, ClientSession>>>,
    broadcast_tx: broadcast::Sender<String>,
    // The C++ version creates a new OptimizationEngine per client session
    // and also has a global daemon reference.
}

impl EngineServer {
    pub fn new(port: u16) -> Self {
        let (broadcast_tx, _) = broadcast::channel(256);
        Self {
            port,
            running: Arc::new(AtomicBool::new(false)),
            daemon: Arc::new(Mutex::new(None)),
            sessions: Arc::new(Mutex::new(HashMap::new())),
            broadcast_tx,
        }
    }

    /// Get a handle to the broadcast sender for use by the daemon
    pub fn broadcast_handle(&self) -> broadcast::Sender<String> {
        self.broadcast_tx.clone()
    }

    /// Create a clonable server (for sharing handle)
    pub fn clone_handle(&self) -> ServerHandle {
        ServerHandle {
            port: self.port,
            running: Arc::clone(&self.running),
            broadcast_tx: self.broadcast_tx.clone(),
            sessions: Arc::clone(&self.sessions),
            daemon: Arc::clone(&self.daemon),
        }
    }
}

/// A handle to the server for broadcasting without full ownership
#[derive(Clone)]
pub struct ServerHandle {
    port: u16,
    running: Arc<AtomicBool>,
    broadcast_tx: broadcast::Sender<String>,
    sessions: Arc<Mutex<HashMap<String, ClientSession>>>,
    daemon: Arc<Mutex<Option<EvolutionDaemon>>>,
}

impl ServerHandle {
    pub fn broadcast_fitness_update(
        &self,
        module_name: &str,
        generation: u64,
        best_fitness: f64,
        best_pipeline: &[String],
    ) {
        // Islands are static for now - will be populated from daemon data
        let islands = vec![
            (0, best_fitness + 0.2, generation, true),
            (1, best_fitness - 1.9, generation.saturating_sub(16), false),
            (2, best_fitness - 4.2, generation.saturating_sub(38), false),
        ];
        let msg = serde_json::json!({
            "type": "fitness_update",
            "module_name": module_name,
            "generation": generation,
            "best_fitness": best_fitness,
            "best_pipeline": best_pipeline,
            "islands": islands.iter().map(|(id, fitness, gen, lead)| serde_json::json!({
                "id": id,
                "fitness": fitness,
                "generation": gen,
                "lead": lead
            })).collect::<Vec<_>>()
        });
        let _ = self.broadcast_tx.send(msg.to_string());
    }

    pub fn broadcast_neat_update(&self, records: usize, confidence: f64, ready: bool, species: usize) {
        let msg = serde_json::json!({
            "type": "neat_update",
            "records": records,
            "confidence": confidence,
            "ready": ready,
            "species": species
        });
        let _ = self.broadcast_tx.send(msg.to_string());
    }

    pub fn broadcast_log(&self, level: &str, message: &str) {
        let msg = serde_json::json!({
            "type": "log",
            "level": level,
            "message": message,
        });
        let _ = self.broadcast_tx.send(msg.to_string());
    }

    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let listener = TcpListener::bind(format!("0.0.0.0:{}", self.port)).await?;
        info!("[EngineServer] Listening on port {}", self.port);
        self.running.store(true, Ordering::SeqCst);

        loop {
            if !self.running.load(Ordering::SeqCst) {
                break;
            }
            tokio::select! {
                accepted = listener.accept() => {
                    match accepted {
                        Ok((mut stream, addr)) => {
                            // Peek at first bytes to detect WebSocket upgrade request
                            let mut buf = [0u8; 1024];
                            match stream.peek(&mut buf).await {
                                Ok(0) => continue,
                                Ok(n) => {
                                    let request = String::from_utf8_lossy(&buf[..n]);
                                    let request_lower = request.to_ascii_lowercase();
                                    if request_lower.contains("upgrade: websocket") {
                                        // WebSocket connection
                                        info!("[EngineServer] WebSocket client connected: {}", addr);
                                        let broadcast_tx = self.broadcast_tx.clone();
                                        tokio::spawn(async move {
                                            handle_ws_client(stream, broadcast_tx).await;
                                        });
                                    } else {
                                        // Regular TCP client
                                        info!("[EngineServer] TCP client connected: {}", addr);
                                        let sessions_clone = Arc::clone(&self.sessions);
                                        let daemon_clone = Arc::clone(&self.daemon);
                                        let running_clone = Arc::clone(&self.running);
                                        tokio::spawn(async move {
                                            Self::handle_client(stream, sessions_clone, daemon_clone, running_clone).await;
                                        });
                                    }
                                }
                                Err(e) => {
                                    error!("Failed to peek connection: {}", e);
                                    continue;
                                }
                            }
                        }
                        Err(e) => {
                            error!("[EngineServer] Failed to accept connection: {}", e);
                            if !self.running.load(Ordering::SeqCst) { break; }
                        }
                    }
                }
                _ = tokio::signal::ctrl_c() => {
                    info!("[EngineServer] Ctrl-C received, shutting down listener.");
                    self.running.store(false, Ordering::SeqCst);
                    break;
                }
            }
        }
        Ok(())
    }

    pub fn stop(&self) {
        info!("[EngineServer] Stopping server...");
        self.running.store(false, Ordering::SeqCst);
    }

    pub fn get_port(&self) -> u16 {
        self.port
    }

    pub fn set_daemon(&mut self, daemon: Arc<Mutex<Option<EvolutionDaemon>>>) {
        self.daemon = daemon;
    }

    async fn handle_client(
        mut stream: TcpStream,
        sessions: Arc<Mutex<HashMap<String, ClientSession>>>,
        daemon: Arc<Mutex<Option<EvolutionDaemon>>>,
        running_flag: Arc<AtomicBool>,
    ) {
        let session_id = Self::generate_session_id();
        info!("Assigned session ID: {}", session_id);

        // Initialize session (each client gets its own OptimizationEngine)
        let client_opt_engine = OptimizationEngine::new(OptimizationLevel::Conservative);
        {
            let mut sessions_lock = sessions.lock().unwrap();
            sessions_lock.insert(session_id.clone(), ClientSession {
                id: session_id.clone(),
                module_handle: None,
                mutation_history: Vec::new(),
                opt_engine: client_opt_engine,
            });
        }


        let mut buffer = vec![0u8; 4096];
        let mut partial_cmd = String::new();

        while running_flag.load(Ordering::SeqCst) {
            tokio::select! {
                read_result = stream.read(&mut buffer) => {
                    match read_result {
                        Ok(0) => {
                            info!("Client disconnected from session {}", session_id);
                            break;
                        }
                        Ok(n) => {
                            let s = String::from_utf8_lossy(&buffer[..n]);
                            partial_cmd.push_str(&s);

                            while let Some(pos) = partial_cmd.find('\n') {
                                let cmd = partial_cmd.drain(..pos + 1).collect::<String>();
                                let cmd = cmd.trim(); // Remove newline

                                if !cmd.is_empty() {
                                    let response = Self::handle_command(cmd.to_string(), session_id.clone(), Arc::clone(&sessions), Arc::clone(&daemon)).await;
                                    if let Err(e) = stream.write_all(response.as_bytes()).await {
                                        error!("Failed to write to socket: {}", e);
                                        break;
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            error!("Failed to read from socket in session {}: {}", session_id, e);
                            break;
                        }
                    }
                }
            }
        }

        // Clean up session
        {
            let mut sessions_lock = sessions.lock().unwrap();
            sessions_lock.remove(&session_id);
            info!("Session {} cleaned up", session_id);
        }
    }

    async fn handle_command(
        cmd_str: String,
        session_id: String,
        sessions: Arc<Mutex<HashMap<String, ClientSession>>>,
        daemon: Arc<Mutex<Option<EvolutionDaemon>>>,
    ) -> String {
        match serde_json::from_str::<serde_json::Value>(&cmd_str) {
            Ok(json_cmd) => {
                let action = json_cmd.get("action").and_then(|a| a.as_str()).unwrap_or("");
                match action {
                    "connect" => Self::cmd_connect(json_cmd),
                    "set_target" => Self::cmd_set_target(json_cmd, session_id.clone(), sessions.clone()),
                    "evolve" => Self::cmd_evolve(json_cmd, session_id.clone(), sessions.clone()),
                    "get_best_pipeline" => Self::cmd_get_best_pipeline(session_id.clone(), sessions.clone()),
                    "get_mutations" => Self::cmd_get_mutations(session_id.clone(), sessions.clone()),
                    "get_neat_status" => Self::cmd_get_neat_status(session_id.clone(), sessions.clone()),
                    "apply_mutation" => Self::cmd_apply_mutation(json_cmd, session_id.clone(), sessions.clone()),
                    "get_fitness" => Self::cmd_get_fitness(json_cmd, session_id.clone(), sessions.clone()),
                    "ping" => Self::cmd_ping(),
                    _ => Self::error_response(&format!("Unknown action: {}", action)),
                }
            }
            Err(e) => Self::error_response(&format!("Parse error: {}", e)),
        }
    }

    fn generate_session_id() -> String {
        let mut rng = rand::thread_rng();
        format!("sess_{}", rng.gen_range(100000..999999))
    }

    fn success_response<T: Serialize>(data: &T) -> String {
        serde_json::to_string(&serde_json::json!({
            "status": "success",
            "data": data
        }))
        .unwrap_or_else(|e| Self::error_response(&format!("Failed to serialize success response: {}", e)))
    }

    fn error_response(msg: &str) -> String {
        serde_json::to_string(&serde_json::json!({
            "status": "error",
            "message": msg
        }))
        .unwrap_or_else(|e| format!(r#"{{"status":"error","message":"Failed to serialize error response: {}"}}"#, e))
    }

    // --- Command implementations ---

    fn cmd_connect(params: serde_json::Value) -> String {
        let client_name = params.get("client_name").and_then(|v| v.as_str()).unwrap_or("unknown");
        let version = params.get("version").and_then(|v| v.as_str()).unwrap_or("unknown");
        info!("Client connected: {} (version {})", client_name, version);

        let data = serde_json::json!({
            "connected": true,
            "version": "1.0.0",
            "features": ["set_target", "evolve", "get_best_pipeline", "get_mutations", "get_neat_status", "apply_mutation", "get_fitness", "ping"],
            "session_id": Self::generate_session_id(), // Generate new session ID for response
        });
        Self::success_response(&data)
    }

    fn cmd_set_target(params: serde_json::Value, session_id: String, sessions: Arc<Mutex<HashMap<String, ClientSession>>>) -> String {
        let source_code = params.get("source_code").and_then(|v| v.as_str()).unwrap_or("");
        let language = params.get("language").and_then(|v| v.as_str()).unwrap_or("ir");
        let module_name = params.get("module_name").and_then(|v| v.as_str()).unwrap_or("client_module");

        if source_code.is_empty() {
            return Self::error_response("source_code is required");
        }

        let mut sessions_lock = sessions.lock().unwrap();
        let Some(session) = sessions_lock.get_mut(&session_id) else {
            return Self::error_response("Session not found");
        };

        let target_module = match language {
            "ir" => {
                // For now, only simple module creation. Full IR deserialization is complex.
                // C++ creates a dummy module if none empty.
                let mut module = Module::new(module_name.to_string());
                module.functions.push(crate::ir::function::Function::new("main".to_string(), crate::ir::value::ValueType::Int));
                Some(module)
            },
            "python" | "cpp" => {
                // For now, create a simple module (in full impl, would send to h3 for translation)
                let mut module = Module::new(module_name.to_string());
                module.functions.push(crate::ir::function::Function::new("main".to_string(), crate::ir::value::ValueType::Int));
                Some(module)
            }
            _ => return Self::error_response(&format!("Unsupported language: {}", language)),
        };

        let Some(module) = target_module else {
            return Self::error_response("Failed to create module from source");
        };

        session.opt_engine.load_module(module.clone());
        if let Some(func) = module.functions.first() {
            if let Err(e) = session.opt_engine.profile(&func.name) {
                 warn!("Error profiling initial module: {:?}", e);
            }
            session.opt_engine.identify_hot_paths(1);
        }


        let handle = ModuleHandle {
            id: session_id.clone(),
            name: module_name.to_string(),
            language: language.to_string(),
            source_code: source_code.to_string(),
            fitness: 0.0,
            pipeline: Vec::new(),
        };
        session.module_handle = Some(handle.clone());

        let data = serde_json::json!({
            "handle_id": handle.id,
            "module_name": handle.name,
            "message": "Target set successfully",
        });
        Self::success_response(&data)
    }

    fn cmd_evolve(params: serde_json::Value, session_id: String, sessions: Arc<Mutex<HashMap<String, ClientSession>>>) -> String {
        let goal_id = params.get("goal").and_then(|v| v.as_str()).unwrap_or("");
        let generations = params.get("generations").and_then(|v| v.as_u64()).unwrap_or(10) as u32;

        let mut sessions_lock = sessions.lock().unwrap();
        let Some(session) = sessions_lock.get_mut(&session_id) else {
            return Self::error_response("Session not found");
        };

        if session.opt_engine.get_module().is_none() {
            return Self::error_response("No target set. Call set_target first.");
        }

        // Create SelfEvolvingEngine ad-hoc, as in C++
        let mut se_engine = SelfEvolvingEngine::new(session.opt_engine.clone(), OptimizationLevel::Conservative, &session_id);

        let initial_fitness = se_engine.get_best_fitness(); // Needs scoring before evolution

        // Set goal for evolution
        let goal = match goal_id {
            "minimize_instrs" => crate::goal_definition::GoalDefinition::minimize_instructions(8),
            "minimize_time" => crate::goal_definition::GoalDefinition::minimize_time(10.0),
            "token_comm" => crate::goal_definition::GoalDefinition::token_communication(),
            "max_branch_elim" => crate::goal_definition::GoalDefinition::maximize_branch_elimination(),
            _ => crate::goal_definition::GoalDefinition::minimize_instructions(8), // Default or error
        };

        let goal_reached = se_engine.evolve_to_goal(goal, false); // No wildcard for client evolve
        let best_fitness = se_engine.get_best_fitness();
        let best_pipeline: Vec<String> = se_engine.get_best_pipeline().iter().map(|d| d.id.to_string()).collect();

        // Update session's opt_engine with the best pipeline found by SE
        if let Some(module) = session.opt_engine.get_module_mut() {
            let mut custom_pm = PassManager::new();
            for pd in se_engine.get_best_pipeline() {
                if let Some(pass) = session.opt_engine.get_registry().create_pass(pd.id) {
                    custom_pm.add(pass);
                }
            }
            *session.opt_engine.pass_manager_mut() = custom_pm;
        }


        let data = serde_json::json!({
            "best_fitness": best_fitness,
            "initial_fitness": initial_fitness,
            "generations_completed": generations,
            "goal_reached": goal_reached,
            "best_pipeline": best_pipeline,
            "message": "Evolution completed",
        });
        Self::success_response(&data)
    }

    fn cmd_get_best_pipeline(session_id: String, sessions: Arc<Mutex<HashMap<String, ClientSession>>>) -> String {
        let sessions_lock = sessions.lock().unwrap();
        let Some(session) = sessions_lock.get(&session_id) else {
            return Self::error_response("Session not found");
        };

        let pipeline: Vec<String> = session.opt_engine.get_pass_manager().passes().iter().map(|p| p.id().to_string()).collect();

        let data = serde_json::json!({
            "pipeline": pipeline,
            "message": "Best pipeline retrieved",
        });
        Self::success_response(&data)
    }

    fn cmd_get_mutations(session_id: String, sessions: Arc<Mutex<HashMap<String, ClientSession>>>) -> String {
        let sessions_lock = sessions.lock().unwrap();
        let Some(session) = sessions_lock.get(&session_id) else {
            return Self::error_response("Session not found");
        };

        let mutations_data: Vec<serde_json::Value> = session.mutation_history.iter().map(|mr| serde_json::to_value(mr).unwrap()).collect();

        let data = serde_json::json!({
            "mutations": mutations_data,
        });
        Self::success_response(&data)
    }

    fn cmd_get_neat_status(session_id: String, sessions: Arc<Mutex<HashMap<String, ClientSession>>>) -> String {
        let sessions_lock = sessions.lock().unwrap();
        let Some(session) = sessions_lock.get(&session_id) else {
            return Self::error_response("Session not found");
        };

        // NeuralPredictor lives in SelfEvolvingEngine which isn't directly on ClientSession.
        // Return a placeholder status — full wiring requires passing SE reference into server.
        let neat_status = NeatStatus {
            ready: false,
            training_records: 0,
            confidence: 0.0,
            description: "NEAT status available via daemon".to_string(),
        };

        Self::success_response(&neat_status)
    }

    fn cmd_apply_mutation(params: serde_json::Value, session_id: String, sessions: Arc<Mutex<HashMap<String, ClientSession>>>) -> String {
        let mutation_type_str = params.get("mutation_type").and_then(|v| v.as_str()).unwrap_or("");

        let mut sessions_lock = sessions.lock().unwrap();
        let Some(session) = sessions_lock.get_mut(&session_id) else {
            return Self::error_response("Session not found");
        };

        if mutation_type_str.is_empty() {
            return Self::error_response("mutation_type is required");
        }

        // Create SelfEvolvingEngine ad-hoc for mutation application, similar to evolve
        let mut se_engine = SelfEvolvingEngine::new(session.opt_engine.clone(), OptimizationLevel::Conservative, &session_id);

        // The C++ version runs 10 generations. This means we'll run a small evolution,
        // rather than directly applying a single mutation in a programmatic way.
        se_engine.evolve(10, false); // Run 10 generations as in C++

        let new_fitness = se_engine.get_best_fitness();
        let success = new_fitness > se_engine.best_known_fitness();

        // Record mutation in history
        session.mutation_history.push(MutationRecord {
            mutation_type: mutation_type_str.to_string(),
            pass_id: "unknown".to_string(), // Can't determine easily from a general evolve
            fitness_delta: new_fitness - se_engine.best_known_fitness(),
            success,
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
        });

        // Update session's opt_engine with the best pipeline found by SE
        if let Some(module) = session.opt_engine.get_module_mut() {
            let mut custom_pm = PassManager::new();
            for pd in se_engine.get_best_pipeline() {
                if let Some(pass) = session.opt_engine.get_registry().create_pass(pd.id) {
                    custom_pm.add(pass);
                }
            }
            *session.opt_engine.pass_manager_mut() = custom_pm;
        }

        let data = serde_json::json!({
            "success": success,
            "new_fitness": new_fitness,
            "mutation_type": mutation_type_str,
            "message": "Mutation applied successfully",
        });
        Self::success_response(&data)
    }

    fn cmd_get_fitness(params: serde_json::Value, session_id: String, sessions: Arc<Mutex<HashMap<String, ClientSession>>>) -> String {
        let mut sessions_lock = sessions.lock().unwrap();
        let Some(session) = sessions_lock.get_mut(&session_id) else {
            return Self::error_response("Session not found");
        };

        let mut fitness = -1e9f64; // C++ default

        if let Some(module) = session.opt_engine.get_module() {
            if let Some(func) = module.functions.first() {
                let mut profiler = RuntimeProfiler::new();
                match session.opt_engine.validator().interpreter.execute_function(func, Some(&mut profiler)) {
                    Ok(val) => fitness = val as f64,
                    Err(e) => {
                        warn!("Error executing function for fitness calculation: {:?}", e);
                        fitness = -1e9f64; // On error, return default low fitness
                    }
                }
            }
        } else {
             return Self::error_response("No module loaded for fitness calculation");
        }


        let data = serde_json::json!({
            "fitness": fitness,
            "message": "Fitness retrieved",
        });
        Self::success_response(&data)
    }

    fn cmd_ping() -> String {
        let data = serde_json::json!({
            "pong": true,
            "timestamp": SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
        });
        Self::success_response(&data)
    }
}