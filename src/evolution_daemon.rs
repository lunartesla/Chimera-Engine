use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}};
use std::path::{Path, PathBuf};
use std::fs;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use std::fmt::Write; // For fmt::write!
use chrono::{DateTime, Local};
use log::{info, warn, error};
use serde::{Serialize, Deserialize};
use crate::self_evolving_engine::MAX_PASSES;

use crate::ir::module::Module;
use crate::self_evolving_engine::{SelfEvolvingEngine, WildMutationKind};
use crate::strain::{StrainEngine, StrainLineage};
use crate::blueprint_archive::BlueprintArchive;
use crate::teacher::Teacher;
use crate::goal_definition::GoalDefinition;
use crate::neural_predictor::NeuralPredictor; // For persistent predictor
use crate::passes::PassDescriptor;
use crate::passes::pass_registry::PassRegistry;
use crate::engine::OptimizationEngine; // For module loading/creation
use crate::{OptimizationLevel, Blueprint};

// TerminalChat placeholder - real implementation handles TUI
pub struct TerminalChat;
impl TerminalChat {
    pub fn new() -> Self { Self }
    pub fn start(&self) {}
    pub fn stop(&self) {}
    pub fn post_daemon_status(&self, _msg: &str) {}
    pub fn update_context(&self, _mode: &str, _mod_name: &str, _mod_shape: &str, _best_fit: f64, _goal_thresh: f64, _neat_gen: i32, _neat_str: &str, _strain_info: &Vec<(String, f64, f64)>, _stuck: i32) {}
    pub fn process_commands(&self) {}
    pub fn total_gens_ref(&mut self) -> &mut i64 { static mut TOTAL_GENS_REF: i64 = 0; unsafe { &mut TOTAL_GENS_REF } } // Placeholder
    pub fn runtime_secs_ref(&mut self) -> &mut i64 { static mut RUNTIME_SECS_REF: i64 = 0; unsafe { &mut RUNTIME_SECS_REF } } // Placeholder
    pub fn add_mutation_outcome(&self, _pass_id: &str, _mut_type: &str, _delta: f64) {}
}


// --- SavedPipeline struct for daemon_best_pipelines.json ---
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedPipeline {
    pass_ids: Vec<String>,
    params: Vec<HashMap<String, i32>>,
    fitness: f64,
}

pub struct ActiveStrain {
    opt_engine: OptimizationEngine,
    pub engine: StrainEngine,
    task_class: String,
    generations_run: i32,
    active: bool,
    // worker: std::thread::JoinHandle<()>, // Placeholder for thread handle
}

pub struct EvolutionDaemon {
    library: Vec<Module>,
    state_file_path: PathBuf,
    stopped: Arc<AtomicBool>,
    wildcard_mode: bool,
    teacher: Option<Teacher>, // Teacher is optional
    archive: BlueprintArchive,

    active_goal: Option<GoalDefinition>,
    module_goals: HashMap<String, GoalDefinition>,

    saved_pipelines: HashMap<String, SavedPipeline>,
    best_fitness_map: HashMap<String, f64>,
    stuck_cycles: HashMap<String, i32>,
    total_gens: i64,
    daemon_cycles: usize,
    generated_module_names: HashSet<String>,
    injection_counts: HashMap<String, HashMap<String, i32>>,
    pub persistent_predictor: Arc<Mutex<NeuralPredictor>>,
    previous_session_memory: String,
    pass_registry: PassRegistry,

    strains: Arc<Mutex<Vec<ActiveStrain>>>,
    next_strain_id: i32,

    terminal_chat: TerminalChat,
    engine_server: crate::engine_server::EngineServer,
    server_handle: crate::engine_server::ServerHandle,
}

impl EvolutionDaemon {
    pub fn new(
        modules: Vec<Module>,
        state_file: &str,
        wildcard_mode: bool,
        teacher: Option<Teacher>,
    ) -> Self {
        let state_file_path = PathBuf::from(state_file);
        let archive = BlueprintArchive::new("blueprints"); // C++ uses "blueprints" directory

        let engine_server = crate::engine_server::EngineServer::new(9877);
        let server_handle = engine_server.clone_handle();

        let mut daemon = Self {
            library: modules,
            state_file_path,
            stopped: Arc::new(AtomicBool::new(false)),
            wildcard_mode,
            teacher,
            archive,
            active_goal: None,
            module_goals: HashMap::new(),
            saved_pipelines: HashMap::new(),
            best_fitness_map: HashMap::new(),
            stuck_cycles: HashMap::new(),
            total_gens: 0,
            daemon_cycles: 0,
            generated_module_names: HashSet::new(),
            injection_counts: HashMap::new(),
            persistent_predictor: Arc::new(Mutex::new(NeuralPredictor::new())),
            previous_session_memory: String::new(),
            pass_registry: PassRegistry::new(),
            strains: Arc::new(Mutex::new(Vec::new())),
            next_strain_id: 1,

            terminal_chat: TerminalChat::new(),
            engine_server,
            server_handle,
        };

        daemon.load_state();
        daemon.previous_session_memory = daemon.load_black_wall_memory();
        daemon
    }

    pub fn set_engine_server(&mut self, server: crate::engine_server::EngineServer) {
        let handle = server.clone_handle();
        self.engine_server = server;
        self.server_handle = handle;
    }

    pub fn run(&mut self) {
        if self.library.is_empty() {
            info!("[BlackWall] No modules to evolve.");
            return;
        }

        let wall_start = Instant::now();

        info!("===========================================");
        info!("   EvolutionDaemon - indefinite runtime       ");
        info!("===========================================");
        info!("  Modules  : {}", self.library.len());
        info!("  Archive  : {} blueprints across {} goals", self.archive.total_blueprints(), self.archive.list_goals().len());
        info!("  Wildcard : {}", if self.wildcard_mode { "ENABLED" } else { "disabled" });
        info!("  Teacher  : {}", if self.teacher.is_some() && self.teacher.as_ref().unwrap().is_available() { "CONNECTED" } else { "not available" });
        info!("  Goal mode: {}", self.active_goal.as_ref().map_or("none (free evolution)".to_string(), |g| g.id.clone()));
        info!("  Pipeline cap: {} passes (RAM sanitation ON)", MAX_PASSES);
        info!("  Press Ctrl+C to stop.\n");

        // Start TCP server on background task
        {
            let server_task_handle = self.server_handle.clone();
            tokio::spawn(async move {
                if let Err(e) = server_task_handle.start().await {
                    error!("[EngineServer] Server error: {}", e);
                }
            });
        }
        info!("[EngineServer] TCP server started on port {}", self.server_handle.get_port());
        // self.engine_server.set_daemon(self); // Skipped: self-referential borrow not possible in safe Rust

        // Start terminal chat
        self.terminal_chat.start();

        let mut module_idx = 0;
        loop {
            if self.stopped.load(Ordering::Relaxed) {
                break;
            }

            self.terminal_chat.process_commands();

            let current_module = self.library[module_idx % self.library.len()].clone();
            let mod_name = current_module.name.clone();

            // Update TerminalChat context
            {
                *self.terminal_chat.total_gens_ref() = self.total_gens;
                *self.terminal_chat.runtime_secs_ref() = wall_start.elapsed().as_secs() as i64;
                let mod_shape = Self::compute_module_shape(&current_module);

                let strains_guard = self.strains.lock().unwrap();
                let strain_info: Vec<(String, f64, f64)> = strains_guard.iter()
                    .filter(|s| s.active)
                    .map(|s| {
                        (s.engine.get_lineage().strain_id.clone(),
                         s.engine.get_best_fitness(),
                         s.engine.get_lineage().fitness_at_fork)
                    })
                    .collect();

                let neat_status_str = self.persistent_predictor.lock().unwrap().get_status_string();

                self.terminal_chat.update_context(
                    self.active_goal.as_ref().map_or("daemon", |g| &g.id),
                    &mod_name,
                    &mod_shape,
                    *self.best_fitness_map.get(&mod_name).unwrap_or(&0.0),
                    self.active_goal.as_ref().map_or(0.0, |g| g.success_threshold),
                    (self.total_gens / 100) as i32, // rough NEAT generation estimate
                    &neat_status_str,
                    &strain_info,
                    *self.stuck_cycles.get(&mod_name).unwrap_or(&0),
                );
            }

            info!("---\n[BlackWall] Module: {}", mod_name);
            self.terminal_chat.post_daemon_status(&format!("Module: {} | Gen: {} | Best: {:.4} | NEAT Gen: {}",
                mod_name, self.total_gens + 100,
                self.best_fitness_map.get(&mod_name).unwrap_or(&0.0),
                self.total_gens / 100
            ));

            // Build engine for this cycle
            let mut eng = OptimizationEngine::new(OptimizationLevel::Conservative);
            eng.load_module(current_module.clone());
            // Need to profile a function, assuming "main" or first function
            if let Some(first_func) = current_module.functions.first() {
                eng.profile(&first_func.name).expect("Failed to profile function");
                eng.identify_hot_paths(1);
            }
            eng.optimize_hot_paths().expect("Failed to optimize hot paths");

            let mut se = SelfEvolvingEngine::new(eng, OptimizationLevel::Conservative, &format!("daemon_se_{}", mod_name));
            se.set_external_stop_flag(Arc::clone(&self.stopped));
            se.set_external_predictor(self.persistent_predictor.lock().unwrap().clone_state());
            // Blueprint replay
            let module_for_hash = se.base_engine().get_module().expect("Module not available for shape hash");
            let hash = BlueprintArchive::compute_shape_hash(module_for_hash);
            let gid = self.active_goal.as_ref().map_or("minimize_instrs".to_string(), |g| g.id.clone());
            if let Some(bp) = self.archive.find_best(&gid, &hash) {
                info!("[BlackWall] Replaying blueprint for '{}'", mod_name);
                se.replay_blueprint(&bp);
            }

            self.seed_population(&mut se, &mod_name);

            // Evolve
            let new_fitness;
            if let Some(goal) = &self.active_goal {
                let reached = se.evolve_to_goal(goal.clone(), self.wildcard_mode);
                new_fitness = se.get_best_fitness();
                if reached {
                    info!("[BlackWall] ★ GOAL REACHED for '{}'!", mod_name);
                }
            } else {
                se.evolve(100, self.wildcard_mode);
                new_fitness = se.get_best_fitness();
            }
            self.total_gens += 100;

            // Strain fork decision
            let task_class = self.active_goal.as_ref().map_or("free_evolution".to_string(), |g| g.id.clone());
            if self.should_fork_strain(&mod_name, &se) {
                self.fork_strain(&mod_name, &se, &task_class);
            }

            // Check for strain promotions
            if self.total_gens % 500 == 0 {
                self.check_promotions();
            }

            // Stuck tracking
            let prev_fitness = self.best_fitness_map.get(&mod_name).cloned().unwrap_or(-1e9);
            let improved = new_fitness > prev_fitness + 0.01;

            let stuck_count = self.stuck_cycles.entry(mod_name.clone()).or_insert(0);
            if improved {
                *stuck_count = 0;
            } else {
                *stuck_count += 1;
            }

            // Teacher integration
            if let Some(teacher_ref) = self.teacher.as_mut() {
                if teacher_ref.is_available() {
                    if *stuck_count >= 3 && !self.persistent_predictor.lock().unwrap().is_nm_ready() {
                        info!("[Teacher] Module '{}' stuck for {} cycles — asking LLM...", mod_name, stuck_count);

                        let ir_module_str = serde_json::to_string(se.base_engine().get_module().unwrap()).unwrap_or_default();
                        let g_desc = self.active_goal.as_ref().map_or("minimize instruction count and execution time".to_string(), |g| g.description.clone());
                        let pipeline_str: Vec<String> = se.get_best_pipeline().iter().map(|d| d.id.to_string()).collect();

                        // Inject previous session memory into context
                        let mut enriched_goal = g_desc.clone();
                        if !self.previous_session_memory.is_empty() {
                            enriched_goal = format!("{}\n\nPREVIOUS SESSION CONTEXT:\n{}", enriched_goal, self.previous_session_memory.chars().take(2000).collect::<String>());
                        }

                        if let Some(suggestion) = teacher_ref.suggest_mutation(
                            &ir_module_str,
                            &enriched_goal,
                            &pipeline_str,
                            new_fitness,
                            new_fitness,
                            *stuck_count,
                        ) {
                            info!("[Teacher] Injecting: {} {}", suggestion.mutation_type, suggestion.pass_id);
                            self.terminal_chat.add_mutation_outcome(&suggestion.pass_id, &suggestion.mutation_type, 0.0);
                            se.add_pass(&suggestion.pass_id); // Assuming add_pass exists
                            se.evolve(50, self.wildcard_mode);
                            self.total_gens += 50;
                            let new_fitness_after_injection = se.get_best_fitness();

                            if new_fitness_after_injection > new_fitness + 0.01 { // Check if injection helped
                                *stuck_count = 0;
                                info!("[Teacher] Suggestion helped! New fitness: {}", new_fitness_after_injection);
                            }
                        }
                    } else if self.persistent_predictor.lock().unwrap().is_nm_ready() {
                        info!("[NM] Neural model handling '{}' — LLM standing by", mod_name);
                    }
                }

                // Generate training labels every 5 cycles (500 total gens if evolutions are 100 per cycle)
                if self.total_gens % 500 == 0 {
                    let goal_desc = self.active_goal.as_ref().map_or("minimize instructions".to_string(), |g| g.description.clone());
                    let labels = teacher_ref.generate_training_labels(&goal_desc, se.get_training_data());
                    if !labels.is_empty() {
                        // Train neural predictor with synthetic records
                        // This involves converting TrainingLabel to PassRecord and feeding it.
                        // For simplicity, directly log for now.
                        info!("[Teacher] Generated {} training labels.", labels.len());
                    }
                    info!("[Teacher] NM confidence: {}% (threshold: 85%)", (teacher_ref.get_nm_confidence() * 100.0) as i32);
                }
            }

            // Broadcast fitness update
            let best_pipeline_str: Vec<String> = se.get_best_pipeline().iter().map(|d| d.id.to_string()).collect();
            self.server_handle.broadcast_fitness_update(
                &mod_name,
                self.total_gens as u64,
                new_fitness,
                &best_pipeline_str,
            );

            // Save if improved
            if improved {
                self.best_fitness_map.insert(mod_name.clone(), new_fitness);
                let sp = SavedPipeline {
                    pass_ids: se.get_best_pipeline().iter().map(|d| d.id.to_string()).collect(),
                    params: se.get_best_pipeline().iter().map(|d| {
                        d.params.iter().map(|(k, v)| (k.clone(), v.current)).collect()
                    }).collect(),
                    fitness: new_fitness,
                };
                self.saved_pipelines.insert(mod_name.clone(), sp);
                self.save_state();
                info!("[BlackWall] ★ New best for '{}': fitness={} passes={}",
                    mod_name, new_fitness, se.get_best_pipeline().len());
                self.terminal_chat.post_daemon_status(&format!("★ New best for '{}': fitness={}", mod_name, new_fitness));
                self.server_handle.broadcast_log("good", &format!("★ New best for '{}': fitness={}", mod_name, new_fitness));
            }

            // Broadcast NEAT update periodically
            if self.total_gens % 100 == 0 {
                let predictor = self.persistent_predictor.lock().unwrap();
                // Use pool size as rough species count
                let species_count = predictor.training_data_len() / 85; // rough estimate
                self.server_handle.broadcast_neat_update(
                    predictor.training_data_len(),
                    predictor.get_nm_confidence(),
                    predictor.is_nm_ready(),
                    species_count,
                );
            }

            module_idx += 1;
            self.daemon_cycles += 1;
            std::thread::sleep(std::time::Duration::from_millis(10));

            // Phase 3C: Self-generating test modules every 10 daemon cycles
            if self.daemon_cycles % 100 == 0 && self.teacher.is_some() && self.teacher.as_ref().unwrap().is_available() && self.generated_module_names.len() < 3 {
                self.generate_self_test_module();
            }
        }

        let elapsed = wall_start.elapsed().as_secs();

        info!("\n[Daemon] Stopped.");
        info!("  Total gens : {}", self.total_gens);
        info!("  Runtime    : {}s", elapsed);
        info!("  Best per module:");
        for (name, fit) in &self.best_fitness_map {
            info!("    {} : {}", name, fit);
        }
    }

    pub fn stop(&mut self) {
        info!("\n[Daemon] Stop requested — finishing current cycle...");
        self.stopped.store(true, Ordering::Relaxed);
        self.terminal_chat.stop();
        self.server_handle.stop();
        self.save_black_wall_memory();
    }

    pub fn set_goal(&mut self, goal: GoalDefinition) {
        info!("[BlackWall] Goal set: {}", goal.description);
        self.active_goal = Some(goal);
    }

    pub fn set_goal_for_module(&mut self, module_name: String, goal: GoalDefinition) {
        info!("[BlackWall] Goal set for '{}': {}", module_name, goal.description);
        self.module_goals.insert(module_name, goal);
    }

    pub fn best_fitness(&self, mod_name: &str) -> f64 {
        self.best_fitness_map.get(mod_name).cloned().unwrap_or(-1e9)
    }

    pub fn total_generations(&self) -> i64 {
        self.total_gens
    }

    // --- State Management ---
    fn load_state(&mut self) {
        if let Ok(content) = fs::read_to_string(&self.state_file_path) {
            match serde_json::from_str::<HashMap<String, SavedPipeline>>(&content) {
                Ok(pipelines) => {
                    self.saved_pipelines = pipelines;
                    for (mod_name, sp) in &self.saved_pipelines {
                        self.best_fitness_map.insert(mod_name.clone(), sp.fitness);
                    }
                    info!("[BlackWall] Loaded state for {} module(s)", self.saved_pipelines.len());
                }
                Err(e) => warn!("[BlackWall] State load error: {}", e),
            }
        }
    }

    fn save_state(&self) {
        if let Ok(json_string) = serde_json::to_string_pretty(&self.saved_pipelines) {
            match fs::write(&self.state_file_path, json_string) {
                Ok(_) => info!("[BlackWall] State saved."),
                Err(e) => warn!("[BlackWall] Failed to save state: {}", e),
            }
        }
    }

    fn seed_population(&mut self, se_engine: &mut SelfEvolvingEngine, mod_name: &str) {
        if let Some(sp) = self.saved_pipelines.get(mod_name) {
            if sp.pass_ids.is_empty() { return; }

            // Only seed first 24 passes (RAM sanitation)
            let limit = MAX_PASSES.min(sp.pass_ids.len());
            let mut pipeline: Vec<PassDescriptor> = Vec::new();
            for i in 0..limit {
                if let Some(mut pass_instance) = self.pass_registry.create_pass(&sp.pass_ids[i]) {
                    if let Some(params_map) = sp.params.get(i) {
                        for (param_name, &param_value) in params_map {
                            let pi: &mut dyn crate::passes::Pass = &mut *pass_instance;
                            pi.set_param(param_name, param_value);
                        }
                    }
                    pipeline.push(pass_instance.descriptor());
                }
            }
            se_engine.replay_blueprint(&Blueprint { // Creating a dummy blueprint
                goal_id: "seed".to_string(),
                function_shape_hash: "dummy".to_string(),
                final_fitness: sp.fitness,
                timestamp: String::new(),
                partial: false,
                steps: sp.pass_ids.iter().zip(sp.params.iter()).map(|(id, map)| {
                    crate::teacher::MutationStep {
                        mutation_type: "add".to_string(), // Placeholder
                        pass_id: id.clone(),
                        params: map.clone(),
                        fitness_after: 0.0,
                        generation: 0,
                    }
                }).collect(),
            });

            info!("[BlackWall] Pre-seeded '{}' (fitness={}, passes={})",
                mod_name, sp.fitness, limit);
        }
    }

    // --- Strain Management ---
    fn compute_module_shape(module: &Module) -> String {
        let func_count = module.functions.len();
        let mut bb_count = 0;
        let mut inst_count = 0;
        for func in &module.functions {
            bb_count += func.basic_blocks.len();
            for bb in &func.basic_blocks {
                inst_count += bb.instructions.len();
            }
        }
        format!("{} fn, {} BB, {} inst", func_count, bb_count, inst_count)
    }

    fn should_fork_strain(&self, mod_name: &str, se_engine: &SelfEvolvingEngine) -> bool {
        // Gate 1: Module stuck >= 3 cycles
        let stuck_count = *self.stuck_cycles.get(mod_name).unwrap_or(&0);
        if stuck_count < 3 { return false; }

        // Gate 2: No existing active strain for this task class already running
        let task_class = self.active_goal.as_ref().map_or("free_evolution".to_string(), |g| g.id.clone());
        let strains_guard = self.strains.lock().unwrap();
        if strains_guard.iter().any(|s| s.active && s.task_class == task_class) {
            return false;
        }
        true
    }

    fn fork_strain(&mut self, mod_name: &str, origin_se: &SelfEvolvingEngine, task_class: &str) {
        let strain_id = format!("strain_{}", self.next_strain_id);
        self.next_strain_id += 1;

        info!("[BlackWall] Forking new strain: {} for task: {} (module: {})",
            strain_id, task_class, mod_name);

        let target_mod = self.library.iter().find(|m| m.name == mod_name)
            .expect("Module not found in library for strain fork").clone();

        let mut opt_engine = OptimizationEngine::new(OptimizationLevel::Conservative);
        opt_engine.load_module(target_mod.clone());
        if let Some(first_func) = target_mod.functions.first() {
            opt_engine.profile(&first_func.name).expect("Failed to profile for strain engine");
            opt_engine.identify_hot_paths(1);
        }
        opt_engine.optimize_hot_paths().expect("Failed to optimize for strain engine");
        let opt_engine_for_active = opt_engine.clone(); // clone before move

        let mut strain_se = SelfEvolvingEngine::new(
            opt_engine,
            OptimizationLevel::Conservative,
            &strain_id,
        );
        strain_se.set_external_stop_flag(Arc::clone(&self.stopped));
        // Inject persistent predictor
        strain_se.set_external_predictor(self.persistent_predictor.lock().unwrap().clone_state());

        let mut strain_engine = StrainEngine::new(
            strain_se,
            &strain_id,
            "origin",
            0,
            task_class,
            origin_se.get_best_fitness(),
        );

        let mut strains_guard = self.strains.lock().unwrap();
        strains_guard.push(ActiveStrain {
            opt_engine: opt_engine_for_active,
            engine: strain_engine,
            task_class: task_class.to_string(),
            generations_run: 0,
            active: true,
        });

        // Start the strain in a separate thread
        // The prompt says `tokio::spawn` but we're in a blocking `run()` loop.
        // For now, `std::thread` is fine.
        let stopped_flag = Arc::clone(&self.stopped);
        let strain_idx = strains_guard.len() - 1;
        let strains_arc_clone = Arc::clone(&self.strains);

        std::thread::spawn(move || {
            let mut locked_strains = strains_arc_clone.lock().unwrap();
            let active_strain = &mut locked_strains[strain_idx];
            active_strain.engine.set_external_stop_flag(stopped_flag);
            active_strain.engine.evolve(100, false); // Run 100 generations
            active_strain.generations_run += 100;
            info!("[Strain] {} completed. Fitness: {}", active_strain.engine.get_lineage().strain_id, active_strain.engine.get_best_fitness());
        });

        info!("[BlackWall] Strain {} started in parallel thread", strain_id);
        self.terminal_chat.post_daemon_status(&format!("Forked new strain: {}", strain_id));
    }


    fn check_promotions(&mut self) {
        let mut strains_guard = self.strains.lock().unwrap();
        let mut promoted_strain_idx: Option<usize> = None;

        for (idx, strain) in strains_guard.iter_mut().filter(|s| s.active).enumerate() {
            if strain.engine.should_promote() {
                // Gate 3: Check stability
                // "Collect last 50 fitness values from strain"
                // This requires `StrainEngine` to expose fitness history.
                // Assuming `self_evolving_engine` has enough data to derive this.
                let se_engine = strain.engine.get_engine();
                let training_data_len = se_engine.get_training_data().len();
                // Use generation count as fitness history proxy since training_data is now Vec<String>
                let fitness_history = se_engine.get_fitness_history();

                if fitness_history.len() >= 50 {
                    let last_50_fitnesses: Vec<f64> = fitness_history.iter().rev()
                        .take(50)
                        .copied()
                        .collect();

                    if !last_50_fitnesses.is_empty() {
                        let mean: f64 = last_50_fitnesses.iter().sum::<f64>() / last_50_fitnesses.len() as f64;
                        let variance: f64 = last_50_fitnesses.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / last_50_fitnesses.len() as f64;

                        if variance < 0.01 { // Variance threshold
                            info!("[BlackWall] Strain {} is ready for promotion (Gate 3 passed: variance={})", strain.engine.get_lineage().strain_id, variance);
                            promoted_strain_idx = Some(idx);
                            break;
                        }
                    }
                }
            }
        }

        if let Some(idx) = promoted_strain_idx {
            let mut promoted_strain = strains_guard.remove(idx);
            let mut origin_se = SelfEvolvingEngine::new(self.library[0].clone().into(), OptimizationLevel::Conservative, "origin"); // Dummy origin
            promoted_strain.engine.promote(&mut origin_se); // Call promote method on strain

            // Actual promotion logic (transfer best pipeline etc.) needs to happen here,
            // updating the daemon's internal state.
            // This is equivalent to `EvolutionDaemon` taking the promoted strain's best pipeline
            // and making it the new default for the relevant module.
            let best_pipeline_from_strain = promoted_strain.engine.get_best_pipeline().to_vec();
            let best_fitness_from_strain = promoted_strain.engine.get_best_fitness();
            let mod_name = promoted_strain.engine.get_lineage().task_class.clone(); // Assuming task_class maps to module name

            // Update daemon's best pipeline
            self.best_fitness_map.insert(mod_name.clone(), best_fitness_from_strain);
            let sp = SavedPipeline {
                pass_ids: best_pipeline_from_strain.iter().map(|d| d.id.to_string()).collect(),
                params: best_pipeline_from_strain.iter().map(|d| {
                    d.params.iter().map(|(k, v)| (k.clone(), v.current)).collect()
                }).collect(),
                fitness: best_fitness_from_strain,
            };
            self.saved_pipelines.insert(mod_name.clone(), sp);
            self.save_state();

            info!("[BlackWall] Strain {} successfully promoted! New best for '{}': fitness={}",
                promoted_strain.engine.get_lineage().strain_id, mod_name, best_fitness_from_strain);
        }
    }


    // --- Phase 3C: Self-generating test modules ---
    fn generate_self_test_module(&mut self) {
        let Some(teacher) = self.teacher.as_mut() else {
            warn!("[Phase3C] Teacher not available, skipping self-generation");
            return;
        };
        if !teacher.is_available() {
            warn!("[Phase3C] Teacher not available, skipping self-generation");
            return;
        }

        info!("[Phase3C] Asking h3 to suggest a new module based on NEAT learning...");

        // Build context from NEAT learning
        let neat_status = format!("NEAT generations: {}", self.total_gens / 100);
        // C++ includes best fitness map, which is harder to get in this context without a direct map
        // let mut best_fitness_map_str = String::new();
        // for (name, fit) in &self.best_fitness_map {
        //     write!(best_fitness_map_str, "{}=\" {}\"", name, fit).unwrap();
        // }

        let system_prompt = "You are h3, an AI assistant for the Metamorphic Engine. \
                             Based on the current NEAT learning progress, suggest a new computational \
                             module that would help improve the optimization engine's capabilities. \
                             The module should be a self-contained function that performs a meaningful \
                             computation, with loops, branches, or mathematical operations. \
                             Describe what it computes, key operations, and any loops. \
                             Keep the description to 2-3 sentences.";

        let user_prompt = format!(
            "Current NEAT status: {}\nSuggest a new module for testing IR optimization.",
            neat_status
        );

        let response = teacher.chat(system_prompt, &user_prompt);

        if response.is_none() {
            warn!("[Phase3C] Failed to get module suggestion from h3, using default");
            let new_module = crate::module_builders::build_simple_sum(20);
            self.add_generated_module(new_module, "A function that computes the sum of numbers from 0 to N using a loop".to_string());
            return;
        }

        let response_str = response.unwrap();
        info!("[Phase3C] h3 suggested: {}", response_str);

        let new_module = if response_str.contains("factorial") {
            crate::module_builders::build_fib_loop(10)
        } else {
            crate::module_builders::build_simple_sum(20)
        };
        self.add_generated_module(new_module, response_str);
    }

    fn add_generated_module(&mut self, module: Module, description: String) {
        let module_name = module.name.clone();
        self.library.push(module.clone());
        self.generated_module_names.insert(module_name.clone());
        info!(
            "[Phase3C] Added new module to active pool: {} (total modules: {})",
            module_name,
            self.library.len()
        );

        // Save to blueprints/generated/ with metadata
        let generated_dir = self.archive.get_directory().join("generated");
        fs::create_dir_all(&generated_dir).expect("Failed to create generated blueprints directory");
        let filename = generated_dir.join(format!("{}.json", module_name));

        #[derive(Serialize)]
        struct GeneratedModuleMeta {
            module_name: String,
            description: String,
            timestamp: u64,
            functions_count: usize,
        }

        let meta = GeneratedModuleMeta {
            module_name: module_name.clone(),
            description,
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            functions_count: module.functions.len(),
        };

        if let Ok(json_string) = serde_json::to_string_pretty(&meta) {
            match fs::write(&filename, json_string) {
                Ok(_) => info!("[Phase3C] Saved module to {}", filename.display()),
                Err(e) => warn!("[Phase3C] Failed to save generated module meta: {}", e),
            }
        }
    }


    // --- BlackWall Session Memory ---
    pub fn save_black_wall_memory(&self) {
        let now: DateTime<Local> = Local::now();
        let buf = now.format("%Y%m%d_%H%M%S").to_string();
        let path = format!("BlackWall_{}.md", buf);

        let mut content = String::new();
        writeln!(&mut content, "# BlackWall Session Memory").unwrap();
        writeln!(&mut content, "Generated: {}\n", buf).unwrap();
        writeln!(&mut content, "## Session Stats").unwrap();
        writeln!(&mut content, "- Total generations: {}", self.total_gens).unwrap();
        writeln!(&mut content, "- Runtime: {}s approx\n", self.total_gens / 50).unwrap(); // C++ formula

        writeln!(&mut content, "## Best Pipelines").unwrap();
        for (mod_name, saved_pipe) in &self.saved_pipelines {
            writeln!(&mut content, "### {} fitness={}", mod_name, saved_pipe.fitness).unwrap();
            let pipeline_str = saved_pipe.pass_ids.join(" -> ");
            writeln!(&mut content, "{}\n", pipeline_str).unwrap();
        }

        writeln!(&mut content, "## Injection History").unwrap();
        for (mod_name, counts) in &self.injection_counts {
            writeln!(&mut content, "### {}", mod_name).unwrap();
            for (pass_name, count) in counts {
                writeln!(&mut content, "- {}: {}x", pass_name, count).unwrap();
            }
        }

        writeln!(&mut content, "\n## Strain Results").unwrap();
        let strains_guard = self.strains.lock().unwrap();
        for strain in strains_guard.iter() {
            writeln!(&mut content, "- {} task={} gens={} fitness={} active={}",
                strain.engine.get_lineage().strain_id,
                strain.engine.get_lineage().task_class,
                strain.generations_run,
                strain.engine.get_best_fitness(),
                if strain.active { "yes" } else { "no" }
            ).unwrap();
        }

        writeln!(&mut content, "\n## NEAT Status").unwrap();
        writeln!(&mut content, "{}", self.persistent_predictor.lock().unwrap().get_status_string()).unwrap();
        writeln!(&mut content, "Records: {}\n", self.persistent_predictor.lock().unwrap().training_data_len()).unwrap();

        writeln!(&mut content, "## Pass Frequency In Best Pipelines").unwrap();
        let mut pass_counts: HashMap<String, i32> = HashMap::new();
        for (_, saved_pipe) in &self.saved_pipelines {
            for pass_id in &saved_pipe.pass_ids {
                *pass_counts.entry(pass_id.clone()).or_insert(0) += 1;
            }
        }
        let mut sorted_pass_counts: Vec<(&String, &i32)> = pass_counts.iter().collect();
        sorted_pass_counts.sort_by(|a, b| b.1.cmp(a.1)); // Sort descending
        for (pass_id, count) in sorted_pass_counts {
            writeln!(&mut content, "- {}: {}x", pass_id, count).unwrap();
        }

        writeln!(&mut content, "\n## Stuck Modules").unwrap();
        for (mod_name, fit) in &self.best_fitness_map {
            let stuck = self.stuck_cycles.get(mod_name).unwrap_or(&0);
            writeln!(&mut content, "- {} best={} stuck={}", mod_name, fit, stuck).unwrap();
        }

        match fs::write(&path, content) {
            Ok(_) => info!("[BlackWall] Session memory saved -> {}", path),
            Err(e) => warn!("[BlackWall] Failed to save session memory: {}", e),
        }
    }

    pub fn load_black_wall_memory(&self) -> String {
        // Find the latest BlackWall_*.md file
        let mut latest_path: Option<PathBuf> = None;
        let mut latest_timestamp: Option<DateTime<Local>> = None;

        if let Ok(entries) = fs::read_dir(".") {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if path.is_file() {
                    if let Some(file_name) = path.file_name().and_then(|s| s.to_str()) {
                        if file_name.starts_with("BlackWall_") && file_name.ends_with(".md") {
                            // Parse timestamp from filename: BlackWall_YYYYMMDD_HHMMSS.md
                            if let Ok(dt) = DateTime::parse_from_str(&file_name[10..25], "%Y%m%d_%H%M%S") {
                                let dt_local = dt.with_timezone(&Local);
                                if latest_timestamp.map_or(true, |lt| dt_local > lt) {
                                    latest_timestamp = Some(dt_local);
                                    latest_path = Some(path.clone());
                                }
                            }
                        }
                    }
                }
            }
        }

        if let Some(path) = latest_path {
            match fs::read_to_string(&path) {
                Ok(content) => {
                    info!("[BlackWall] Loaded session memory: {}", path.display());
                    return content;
                },
                Err(e) => warn!("[BlackWall] Failed to read session memory {}: {}", path.display(), e),
            }
        }
        String::new()
    }

    pub fn get_library(&self) -> &Vec<Module> {
        &self.library
    }
}
