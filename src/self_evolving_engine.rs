use std::sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}};
use std::collections::{HashMap, VecDeque};
use rand::{self, Rng, seq::SliceRandom};
use rayon::prelude::*;
use crate::ir::module::Module;
use log::info;
use crate::engine::OptimizationEngine;
use crate::passes::{PassManager, PassRegistry, OptimizationLevel, PassDescriptor};
use crate::neural_predictor::{NeuralPredictor, FunctionStats, Prediction};
use crate::goal_definition::GoalDefinition;
use crate::blueprint_archive::{Blueprint, BlueprintArchive}; // Assuming Blueprint and BlueprintArchive

// Constants (exact from C++ headers)
const NUM_ISLANDS: usize = 3;
const ISLAND_SIZE: usize = 7;
const TOTAL_POP: usize = NUM_ISLANDS * ISLAND_SIZE; // 21
pub const MAX_PASSES: usize = 24; // HARD LIMIT — enforce with assert!
const OUTCOME_WINDOW: usize = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Mutation {
    Add,
    Remove,
    Reorder,
    Duplicate,
    Tune,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WildMutationKind {
    SwapOperands,
    ReplaceOpcode,
    FoldConstant,
    InvertBranch,
    InsertNoop,
    DeleteInstruction,
    RewireJump,
    HoistStore,
    ConstantScale,
    CloneBlock,
    SplitBlock,
    MergeConstants,
    InsertBranch,
    HoistConstant,
    SpecializeLoop,
}
// PipelineScoreResult needs to be defined outside the impl block
struct PipelineScoreResult {
    fitness: f64,
    pipeline_len: usize,
    instruction_count: usize,
    block_count: usize,
    branch_count: usize,
}

pub struct SelfEvolvingEngine {
    base_engine: OptimizationEngine,
    pass_registry: Arc<PassRegistry>, // Shared registry
    neural_predictor: NeuralPredictor,
    strain_id: String,

    population: Vec<Vec<PassDescriptor>>, // Vec of pipelines (each a Vec of PassDescriptor)
    population_fitness: Vec<f64>,
    best_known_pipeline: Vec<PassDescriptor>,
    best_known_fitness: f64,
    temperature: f64,
    baseline_instruction_count: usize,

    // Adaptive mutation rates
    outcome_history: VecDeque<(Mutation, bool, f64)>, // (mutation type, success, fitness_delta)
    add_success_rate: f64,
    remove_success_rate: f64,
    reorder_success_rate: f64,
    duplicate_success_rate: f64,
    tune_success_rate: f64,

    validation_target_func: String,
    training_data: Vec<String>, // For recording pass outcomes
    pub total_generations: usize,

    wild_attempted: u64,
    wild_accepted: u64,
    fitness_history: Vec<f64>,

    external_stop_flag: Option<Arc<AtomicBool>>,
}

impl SelfEvolvingEngine {
    pub fn new(mut base_engine: OptimizationEngine, opt_level: OptimizationLevel, strain_id: &str) -> Self {
        let registry = base_engine.get_registry().clone();
        let np = NeuralPredictor::new();

        Self {
            base_engine,
            pass_registry: Arc::new(registry),
            neural_predictor: np,
            strain_id: strain_id.to_string(),
            population: Vec::new(),
            population_fitness: Vec::new(),
            best_known_pipeline: Vec::new(),
            best_known_fitness: -1e9, // C++ default
            temperature: 1.0,
            baseline_instruction_count: 1, // Placeholder, updated after first module load

            outcome_history: VecDeque::with_capacity(OUTCOME_WINDOW),
            add_success_rate: 0.25,
            remove_success_rate: 0.25,
            reorder_success_rate: 0.20,
            duplicate_success_rate: 0.15,
            tune_success_rate: 0.15,

            validation_target_func: "main".to_string(), // Default validation target
            training_data: Vec::new(),
            total_generations: 1000,
            wild_attempted: 0,
            wild_accepted: 0,
            fitness_history: Vec::new(),
            external_stop_flag: None,
        }
    }

    pub fn set_external_stop_flag(&mut self, flag: Arc<AtomicBool>) {
        self.external_stop_flag = Some(flag);
    }

    pub fn set_external_predictor(&mut self, predictor: NeuralPredictor) {
        self.neural_predictor = predictor;
    }

    pub fn get_predictor(&self) -> &NeuralPredictor {
        &self.neural_predictor
    }

    pub fn get_predictor_mut(&mut self) -> &mut NeuralPredictor {
        &mut self.neural_predictor
    }

    pub fn get_fitness_history(&self) -> &[f64] {
        &self.fitness_history
    }

    pub fn base_engine(&self) -> &OptimizationEngine {
        &self.base_engine
    }

    pub fn base_engine_mut(&mut self) -> &mut OptimizationEngine {
        &mut self.base_engine
    }

    pub fn best_known_fitness(&self) -> f64 {
        self.best_known_fitness
    }

    pub fn add_pass(&mut self, pass_id: &str) {
        if let Some(pass) = self.pass_registry.create_pass(pass_id) {
            // Add to a temporary pipeline, then score/evolve
            let mut temp_pipeline = self.best_known_pipeline.clone();
            temp_pipeline.push(pass.descriptor());
            if temp_pipeline.len() > MAX_PASSES {
                temp_pipeline.truncate(MAX_PASSES);
            }
            let new_fitness_record = self.score_pipeline(&temp_pipeline);
            if new_fitness_record.fitness > self.best_known_fitness {
                self.best_known_fitness = new_fitness_record.fitness;
                self.best_known_pipeline = temp_pipeline;
                info!("New best pipeline after add_pass: {}", self.best_known_fitness);
            }
        }
    }

    pub fn evolve(&mut self, generations: u32, wildcard_mode: bool) {
        // Ensure module is loaded
        if self.base_engine.get_module().is_none() {
            eprintln!("Error: No module loaded in OptimizationEngine for evolution.");
            return;
        }
        let initial_module_instruction_count = self.base_engine.get_module().map_or(1, |m| m.instruction_count());
        self.baseline_instruction_count = initial_module_instruction_count;


        // Initialize population if empty
        if self.population.is_empty() {
            self.init_population();
            // Score initial population
            self.score_population();
            // Update best known
            self.update_best_known();
        }

        for gen in 0..generations {
            let mut changed = false;
            if self.external_stop_flag.as_ref().map_or(false, |f| f.load(Ordering::Relaxed)) {
                info!("Evolution stopped by external signal.");
                break;
            }

            // Adaptive mutation rates adjustment every 100 generations
            if gen > 0 && gen % 100 == 0 {
                self.readjust_mutation_rates();
            }

            // Phase A (sequential, cheap): decide mutations per island.
            // Mutation decisions touch the shared NEAT predictor (&mut self),
            // so this part stays sequential — it's fast, not the bottleneck.
            struct IslandDecision {
                island_id: usize,
                i_start: usize,
                i_end: usize,
                child_pipeline: Vec<PassDescriptor>,
                applied_mutation_type: Mutation,
                mutated_pass_id: String,
            }

            let mut decisions: Vec<IslandDecision> = Vec::with_capacity(NUM_ISLANDS);

            for island_id in 0..NUM_ISLANDS {
                let i_start = island_id * ISLAND_SIZE;
                let i_end = i_start + ISLAND_SIZE;

                let (p1i, p2i) = self.tournament_select_indices(i_start, i_end);
                let mut child_pipeline_descriptors = self.crossover(
                    &self.population[p1i],
                    &self.population[p2i],
                );

                assert!(child_pipeline_descriptors.len() <= MAX_PASSES, "pass sequence exceeds 24-pass cap");
                if child_pipeline_descriptors.len() > MAX_PASSES {
                    child_pipeline_descriptors.truncate(MAX_PASSES);
                }

                let mut applied_mutation_type = Mutation::Add;
                let mut mutated_pass_id = String::new();

                if wildcard_mode {
                    let mut rng = rand::thread_rng();
                    let options = [Mutation::Add, Mutation::Remove, Mutation::Reorder, Mutation::Duplicate, Mutation::Tune];
                    applied_mutation_type = *options.choose(&mut rng).unwrap();
                    self.apply_mutation_to_pipeline(
                        &mut child_pipeline_descriptors,
                        self.temperature,
                        Some(applied_mutation_type),
                        &mut mutated_pass_id
                    );
                    self.wild_attempted += 1;
                } else if self.neural_predictor.is_nm_ready() {
                    let current_module = self.base_engine.get_module().expect("Module not loaded for stats");
                    let stats = self.compute_function_stats(current_module);
                    let mut extra_features = HashMap::new();
                    extra_features.insert("temperature".to_string(), self.temperature);
                    extra_features.insert("generation_ratio".to_string(), gen as f64 / self.total_generations as f64);
                    extra_features.insert("pipeline_length".to_string(), child_pipeline_descriptors.len() as f64);
                    extra_features.insert("pass_frequency".to_string(), 0.0);
                    extra_features.insert("cycles_stuck".to_string(), 0.0);
                    extra_features.insert("island_id".to_string(), island_id as f64);
                    extra_features.insert("goal_ratio".to_string(), 0.0);

                    let prediction = self.neural_predictor.predict("constant_folding", &stats, &extra_features);
                    let success_prob = prediction.as_ref().map(|p| p.success_prob as f64).unwrap_or(0.5);

                    let mut rng = rand::thread_rng();
                    if success_prob > rng.gen::<f64>() {
                        applied_mutation_type = Mutation::Add;
                    } else {
                        let opts = [Mutation::Remove, Mutation::Reorder, Mutation::Duplicate, Mutation::Tune];
                        applied_mutation_type = *opts.choose(&mut rng).unwrap();
                    }
                    self.apply_mutation_to_pipeline(
                        &mut child_pipeline_descriptors,
                        self.temperature,
                        Some(applied_mutation_type),
                        &mut mutated_pass_id
                    );
                } else {
                    let num_mutations = if self.temperature > 0.7 { 2 } else { 1 };
                    let mut rng = rand::thread_rng();
                    for _ in 0..num_mutations {
                        let options = [Mutation::Add, Mutation::Remove, Mutation::Reorder, Mutation::Duplicate, Mutation::Tune];
                        applied_mutation_type = *options.choose(&mut rng).unwrap();
                        self.apply_mutation_to_pipeline(
                            &mut child_pipeline_descriptors,
                            self.temperature,
                            Some(applied_mutation_type),
                            &mut mutated_pass_id
                        );
                    }
                }

                if child_pipeline_descriptors.is_empty() {
                    continue;
                }

                decisions.push(IslandDecision {
                    island_id, i_start, i_end,
                    child_pipeline: child_pipeline_descriptors,
                    applied_mutation_type,
                    mutated_pass_id,
                });
            }

            // Phase B (parallel, expensive): score_pipeline() clones the whole
            // engine + module and runs every pass — this is the real bottleneck,
            // and it's read-only (&self), so islands can score concurrently.
            // Uses rayon's persistent global thread pool (created once, reused
            // forever) instead of spawning fresh OS threads every generation —
            // std::thread::scope was paying thread-create/destroy cost on every
            // single call, which dominated runtime after the redundant clones
            // in score_pipeline() were removed.
            let self_ref: &Self = &*self;
            let scored: Vec<(IslandDecision, f64, usize)> = decisions
                .into_par_iter()
                .map(|d| {
                    let result = self_ref.score_pipeline(&d.child_pipeline);
                    (d, result.fitness, result.pipeline_len)
                })
                .collect();

            // Phase C (sequential, cheap): apply results — population writes,
            // adaptive-rate bookkeeping, and NEAT outcome recording all need
            // &mut self, so this runs back on the main thread.
            for (d, child_fitness, pipeline_len) in scored {
                let (worst_idx_in_island, &worst_fitness_in_island) = self.population_fitness[d.i_start..d.i_end]
                    .iter()
                    .enumerate()
                    .min_by(|(_, &a), (_, &b)| a.partial_cmp(&b).unwrap_or(std::cmp::Ordering::Equal))
                    .unwrap();
                let worst_idx = d.i_start + worst_idx_in_island;

                let delta_fitness = child_fitness - worst_fitness_in_island;

                let mut rng = rand::thread_rng();
                let accept = delta_fitness > 0.0
                    || rng.gen::<f64>() < (delta_fitness / (self.temperature + 1e-9)).exp()
                    || rng.gen::<f64>() < 0.05;

                if accept {
                    self.population[worst_idx] = d.child_pipeline.clone();
                    self.population_fitness[worst_idx] = child_fitness;
                    changed = true;

                    self.record_mutation_outcome(d.applied_mutation_type, delta_fitness > 0.0, delta_fitness);

                    let current_module = self.base_engine.get_module().expect("Module not loaded for stats");
                    let after_stats = self.compute_function_stats(current_module);
                    let mut extra_features = HashMap::new();
                    extra_features.insert("temperature".to_string(), self.temperature);
                    extra_features.insert("generation_ratio".to_string(), gen as f64 / self.total_generations as f64);
                    extra_features.insert("pipeline_length".to_string(), pipeline_len as f64);
                    extra_features.insert("pass_frequency".to_string(), 0.0);
                    extra_features.insert("cycles_stuck".to_string(), 0.0);
                    extra_features.insert("island_id".to_string(), d.island_id as f64);
                    extra_features.insert("goal_ratio".to_string(), 0.0);

                    self.neural_predictor.record_outcome(
                        &d.mutated_pass_id,
                        &after_stats,
                        &extra_features,
                        child_fitness > worst_fitness_in_island,
                        delta_fitness,
                    );
                }
            }

            // Update best known pipeline and fitness after each generation
            self.update_best_known();

            // Migrate islands every 20 generations
            if gen > 0 && (gen + 1) % 20 == 0 {
                self.migrate_islands();
            }

            // Simple temperature cooling
            self.temperature *= 0.99;
            if self.temperature < 0.01 { self.temperature = 0.01; }
        }
    }

    pub fn evolve_to_goal(&mut self, goal: GoalDefinition, wildcard_mode: bool) -> bool {
        // Ensure module is loaded
        if self.base_engine.get_module().is_none() {
            eprintln!("Error: No module loaded in OptimizationEngine for evolution.");
            return false;
        }
        let initial_module_instruction_count = self.base_engine.get_module().map_or(1, |m| m.instruction_count());
        self.baseline_instruction_count = initial_module_instruction_count;

        // Initialize population if empty
        if self.population.is_empty() {
            self.init_population();
            self.score_population();
            self.update_best_known();
        }

        let mut gen = 0;
        while gen < goal.max_generations as u32 {
            if self.external_stop_flag.as_ref().map_or(false, |f| f.load(Ordering::Relaxed)) {
                info!("Evolution stopped by external signal.");
                break;
            }

            // Check if goal reached
            if (goal.fitness_fn)(self.base_engine.get_module().unwrap()) >= goal.success_threshold {
                info!("Goal '{}' reached!", goal.id);
                return true;
            }

            // Adaptive mutation rates adjustment every 100 generations
            if gen > 0 && gen % 100 == 0 {
                self.readjust_mutation_rates();
            }

            // Evolution loop for islands - same as evolve, but check goal
            for island_id in 0..NUM_ISLANDS {
                let i_start = island_id * ISLAND_SIZE;
                let i_end = i_start + ISLAND_SIZE;

                let (p1i, p2i) = self.tournament_select_indices(i_start, i_end);
                let mut child_pipeline_descriptors = self.crossover(
                    &self.population[p1i],
                    &self.population[p2i],
                );

                assert!(child_pipeline_descriptors.len() <= MAX_PASSES, "pass sequence exceeds 24-pass cap");
                if child_pipeline_descriptors.len() > MAX_PASSES {
                    child_pipeline_descriptors.truncate(MAX_PASSES);
                }

                let mut applied_mutation_type = Mutation::Add;
                let mut mutated_pass_id = String::new();

                if wildcard_mode {
                    let mut rng = rand::thread_rng();
                    let options = [Mutation::Add, Mutation::Remove, Mutation::Reorder, Mutation::Duplicate, Mutation::Tune];
                    applied_mutation_type = *options.choose(&mut rng).unwrap();
                    self.apply_mutation_to_pipeline(
                        &mut child_pipeline_descriptors,
                        self.temperature,
                        Some(applied_mutation_type),
                        &mut mutated_pass_id
                    );
                    self.wild_attempted += 1;
                } else if self.neural_predictor.is_nm_ready() {
                    let current_module = self.base_engine.get_module().expect("Module not loaded for stats");
                    let stats = self.compute_function_stats(current_module);
                    let mut extra_features = HashMap::new();
                    extra_features.insert("temperature".to_string(), self.temperature);
                    extra_features.insert("generation_ratio".to_string(), gen as f64 / self.total_generations as f64);
                    extra_features.insert("pipeline_length".to_string(), child_pipeline_descriptors.len() as f64);
                    extra_features.insert("pass_frequency".to_string(), 0.0);
                    extra_features.insert("cycles_stuck".to_string(), 0.0);
                    extra_features.insert("island_id".to_string(), island_id as f64);
                    extra_features.insert("goal_ratio".to_string(), 0.0);

                    let prediction = self.neural_predictor.predict("constant_folding", &stats, &extra_features);
                    let success_prob = prediction.as_ref().map(|p| p.success_prob as f64).unwrap_or(0.5);

                    let mut rng = rand::thread_rng();
                    if success_prob > rng.gen::<f64>() {
                        applied_mutation_type = Mutation::Add;
                    } else {
                        let opts = [Mutation::Remove, Mutation::Reorder, Mutation::Duplicate, Mutation::Tune];
                        applied_mutation_type = *opts.choose(&mut rng).unwrap();
                    }
                    self.apply_mutation_to_pipeline(
                        &mut child_pipeline_descriptors,
                        self.temperature,
                        Some(applied_mutation_type),
                        &mut mutated_pass_id
                    );
                } else {
                    let num_mutations = if self.temperature > 0.7 { 2 } else { 1 };
                    let mut rng = rand::thread_rng();
                    for _ in 0..num_mutations {
                        let options = [Mutation::Add, Mutation::Remove, Mutation::Reorder, Mutation::Duplicate, Mutation::Tune];
                    applied_mutation_type = *options.choose(&mut rng).unwrap();
                        self.apply_mutation_to_pipeline(
                            &mut child_pipeline_descriptors,
                            self.temperature,
                            Some(applied_mutation_type),
                            &mut mutated_pass_id
                        );
                    }
                }


                if child_pipeline_descriptors.is_empty() {
                    continue;
                }

                let child_fitness_record = self.score_pipeline(&child_pipeline_descriptors);
                let child_fitness = child_fitness_record.fitness;

                let (worst_idx_in_island, &worst_fitness_in_island) = self.population_fitness[i_start..i_end]
                    .iter()
                    .enumerate()
                    .min_by(|(_, &a), (_, &b)| a.partial_cmp(&b).unwrap_or(std::cmp::Ordering::Equal))
                    .unwrap();
                let worst_idx = i_start + worst_idx_in_island;

                let delta_fitness = child_fitness - worst_fitness_in_island;

                let mut rng = rand::thread_rng();
                let accept = delta_fitness > 0.0
                    || rng.gen::<f64>() < (delta_fitness / (self.temperature + 1e-9)).exp()
                    || rng.gen::<f64>() < 0.05;

                if accept {
                    self.population[worst_idx] = child_pipeline_descriptors.clone();
                    self.population_fitness[worst_idx] = child_fitness;

                    self.record_mutation_outcome(applied_mutation_type, delta_fitness > 0.0, delta_fitness);

                    let current_module = self.base_engine.get_module().expect("Module not loaded for stats");
                    let before_stats = self.compute_function_stats(current_module);
                    let after_stats = self.compute_function_stats(current_module);
                    let mut extra_features = HashMap::new();
                    extra_features.insert("temperature".to_string(), self.temperature);
                    extra_features.insert("generation_ratio".to_string(), gen as f64 / self.total_generations as f64);
                    extra_features.insert("pipeline_length".to_string(), child_fitness_record.pipeline_len as f64);
                    extra_features.insert("pass_frequency".to_string(), 0.0);
                    extra_features.insert("cycles_stuck".to_string(), 0.0);
                    extra_features.insert("island_id".to_string(), island_id as f64);
                    extra_features.insert("goal_ratio".to_string(), 0.0);

                    self.neural_predictor.record_outcome(
                        &mutated_pass_id,
                        &after_stats,
                        &extra_features,
                        child_fitness > worst_fitness_in_island,
                        child_fitness - worst_fitness_in_island,
                    );
                }
            }

            self.update_best_known();

            if gen > 0 && (gen + 1) % 20 == 0 {
                self.migrate_islands();
            }

            self.temperature *= 0.99;
            if self.temperature < 0.01 { self.temperature = 0.01; }

            gen += 1;
        }

        (goal.fitness_fn)(self.base_engine.get_module().unwrap()) >= goal.success_threshold
    }


    pub fn replay_blueprint(&mut self, bp: &Blueprint) -> bool {
        if bp.steps.is_empty() { return false; }

        let mut pipeline: Vec<PassDescriptor> = Vec::new();
        for step in &bp.steps {
            // Need to retrieve the actual PassDescriptor from the registry based on ID
            if let Some(mut pass) = self.pass_registry.create_pass(&step.pass_id) {
                // Apply parameters from blueprint step
                for (param_name, &param_value) in &step.params {
                    pass.set_param(param_name, param_value);
                }
                pipeline.push(pass.descriptor()); // Store the descriptor
            }
        }

        if pipeline.is_empty() { return false; }

        self.population.clear(); // Clear existing population
        self.population_fitness.clear();

        // Seed population with blueprint
        for _ in 0..TOTAL_POP {
            self.population.push(pipeline.clone());
            self.population_fitness.push(self.score_pipeline(&pipeline).fitness);
        }

        self.update_best_known();
        true
    }


    pub fn get_best_fitness(&self) -> f64 {
        self.best_known_fitness
    }

    pub fn get_best_pipeline(&self) -> &[PassDescriptor] {
        &self.best_known_pipeline
    }

    pub fn get_training_data(&self) -> &[String] {
        &self.training_data
    }

    fn init_population(&mut self) {
        let all_pass_ids = self.pass_registry.list_all();
        let mut rng = rand::thread_rng();

        for _ in 0..TOTAL_POP {
            let num_passes = rng.gen_range(1..=MAX_PASSES);
            let mut pipeline = Vec::new();
            for _ in 0..num_passes {
                if let Some(pass_id) = all_pass_ids.choose(&mut rng) {
                    if let Some(pass) = self.pass_registry.create_pass(pass_id) {
                        pipeline.push(pass.descriptor());
                    }
                }
            }
            self.population.push(pipeline);
            self.population_fitness.push(-1e9); // Placeholder, will be scored
        }
    }

    fn score_population(&mut self) {
        for i in 0..TOTAL_POP {
            self.population_fitness[i] = self.score_pipeline(&self.population[i]).fitness;
        }
    }


    fn score_pipeline(&self, pipeline: &[PassDescriptor]) -> PipelineScoreResult {
        // `self.base_engine.clone()` already deep-clones both original_module and
        // working_module (see OptimizationEngine's Clone impl). The old code then
        // called load_module() again, cloning the module a 2nd+3rd time for nothing,
        // plus rebuilt a whole PassManager via set_optimization_level() just to
        // immediately overwrite it with the custom pipeline below. Both were pure
        // waste — temp_engine already has a correct working module from the clone,
        // and pass_manager is fully replaced two lines down regardless of opt_level.
        let mut temp_engine = self.base_engine.clone();
        let mut custom_pass_manager = PassManager::new();
        for pd in pipeline {
            if let Some(mut pass_instance) = self.pass_registry.create_pass(pd.id) {
                // Apply parameters from the descriptor
                for (param_name, param_range) in &pd.params {
                    pass_instance.set_param(param_name, param_range.current);
                }
                custom_pass_manager.add(pass_instance);
            }
        }
        *temp_engine.pass_manager_mut() = custom_pass_manager;

        let mut changed = false;
        changed = temp_engine.run_passes_on_module();

        let current_module = temp_engine.get_module().expect("Module not available after scoring pipeline");
        let stats = self.compute_function_stats(current_module);

        // Fitness formula: -(0.6 * inst_count + 0.4 * 1.0 * baseline_inst_count)
        let fitness = -(0.6 * stats.instruction_count as f64
                      + 0.4 * 1.0 * self.baseline_instruction_count as f64);

        PipelineScoreResult {
            fitness,
            pipeline_len: pipeline.len(),
            instruction_count: stats.instruction_count,
            block_count: stats.block_count,
            branch_count: stats.branch_count,
        }
    }

    fn update_best_known(&mut self) {
        let mut current_best_idx = 0;
        let mut current_best_fitness = -1e9;
        for i in 0..TOTAL_POP {
            if self.population_fitness[i] > current_best_fitness {
                current_best_fitness = self.population_fitness[i];
                current_best_idx = i;
            }
        }

        if current_best_fitness > self.best_known_fitness {
            self.best_known_fitness = current_best_fitness;
            self.best_known_pipeline = self.population[current_best_idx].clone();
            info!("New best pipeline found with fitness: {}", self.best_known_fitness);
        }
        self.fitness_history.push(self.best_known_fitness);
    }

    fn tournament_select_indices(&self, start: usize, end: usize) -> (usize, usize) {
        let mut rng = rand::thread_rng();
        let p1_idx = rng.gen_range(start..end);
        let p2_idx = rng.gen_range(start..end);
        (p1_idx, p2_idx)
    }

    fn crossover(&self, p1: &[PassDescriptor], p2: &[PassDescriptor]) -> Vec<PassDescriptor> {
        let mut rng = rand::thread_rng();
        let crossover_point = rng.gen_range(0..p1.len().max(p2.len()));
        let mut child = Vec::new();
        child.extend_from_slice(&p1[..crossover_point.min(p1.len())]);
        child.extend_from_slice(&p2[crossover_point.min(p2.len())..]);
        child
    }

    fn apply_mutation_to_pipeline(&mut self, pipeline: &mut Vec<PassDescriptor>, temperature: f64, mut_type_hint: Option<Mutation>, mutated_pass_id: &mut String) {
        let mut rng = rand::thread_rng();
        let mutation_type = mut_type_hint.unwrap_or_else(|| {
            // Select mutation type based on adaptive rates or randomness
            let r = rng.gen::<f64>();
            if r < self.add_success_rate { Mutation::Add }
            else if r < self.add_success_rate + self.remove_success_rate { Mutation::Remove }
            else if r < self.add_success_rate + self.remove_success_rate + self.reorder_success_rate { Mutation::Reorder }
            else if r < self.add_success_rate + self.remove_success_rate + self.reorder_success_rate + self.duplicate_success_rate { Mutation::Duplicate }
            else { Mutation::Tune }
        });


        // Apply mutation
        match mutation_type {
            Mutation::Add => {
                if pipeline.len() < MAX_PASSES {
                    let all_pass_ids = self.pass_registry.list_all();
                    if let Some(pass_id) = all_pass_ids.choose(&mut rng) {
                        if let Some(pass) = self.pass_registry.create_pass(pass_id) {
                            pipeline.insert(rng.gen_range(0..=pipeline.len()), pass.descriptor());
                            *mutated_pass_id = pass_id.to_string();
                        }
                    }
                }
            }
            Mutation::Remove => {
                if !pipeline.is_empty() {
                    let idx = rng.gen_range(0..pipeline.len());
                    *mutated_pass_id = pipeline[idx].id.to_string();
                    pipeline.remove(idx);
                }
            }
            Mutation::Reorder => {
                if pipeline.len() >= 2 {
                    let idx1 = rng.gen_range(0..pipeline.len());
                    let idx2 = rng.gen_range(0..pipeline.len());
                    pipeline.swap(idx1, idx2);
                    *mutated_pass_id = pipeline[idx1].id.to_string(); // Arbitrary
                }
            }
            Mutation::Duplicate => {
                if !pipeline.is_empty() && pipeline.len() < MAX_PASSES {
                    let idx = rng.gen_range(0..pipeline.len());
                    *mutated_pass_id = pipeline[idx].id.to_string();
                    pipeline.insert(rng.gen_range(0..=pipeline.len()), pipeline[idx].clone());
                }
            }
            Mutation::Tune => {
                if !pipeline.is_empty() {
                    let idx = rng.gen_range(0..pipeline.len());
                    let pass_desc = &mut pipeline[idx];
                    *mutated_pass_id = pass_desc.id.to_string();
                    if let Some((_, param_range)) = pass_desc.params.iter_mut().next() { // Tune first param for simplicity
                        let new_val = rng.gen_range(param_range.min..=param_range.max);
                        param_range.set_current(new_val);
                    }
                }
            }
        }
    }

    fn migrate_islands(&mut self) {
        let mut rng = rand::thread_rng();
        // Simple migration: each island sends its best to a random other island
        for i in 0..NUM_ISLANDS {
            let best_in_island_idx = (i * ISLAND_SIZE..(i + 1) * ISLAND_SIZE)
                .max_by(|&a, &b| self.population_fitness[a].partial_cmp(&self.population_fitness[b]).unwrap_or(std::cmp::Ordering::Equal))
                .unwrap();
            let best_pipeline = self.population[best_in_island_idx].clone();

            let mut target_island_idx = rng.gen_range(0..NUM_ISLANDS);
            while target_island_idx == i {
                target_island_idx = rng.gen_range(0..NUM_ISLANDS);
            }

            let worst_in_target_island_idx = (target_island_idx * ISLAND_SIZE..(target_island_idx + 1) * ISLAND_SIZE)
                .min_by(|&a, &b| self.population_fitness[a].partial_cmp(&self.population_fitness[b]).unwrap_or(std::cmp::Ordering::Equal))
                .unwrap();

            self.population[worst_in_target_island_idx] = best_pipeline;
            self.population_fitness[worst_in_target_island_idx] = self.score_pipeline(&self.population[worst_in_target_island_idx]).fitness;
        }
    }

    fn record_mutation_outcome(&mut self, mutation: Mutation, success: bool, fitness_delta: f64) {
        if self.outcome_history.len() == OUTCOME_WINDOW {
            self.outcome_history.pop_front();
        }
        self.outcome_history.push_back((mutation, success, fitness_delta));
    }

    fn readjust_mutation_rates(&mut self) {
        let mut success_counts: HashMap<Mutation, (f64, f64)> = HashMap::new(); // (total_delta, count)

        for (mut_type, success, delta) in &self.outcome_history {
            let entry = success_counts.entry(*mut_type).or_insert((0.0, 0.0));
            entry.0 += delta; // Accumulate fitness delta
            entry.1 += 1.0;   // Count occurrences
        }

        // Apply a simple adaptive strategy: prioritize mutations that led to higher fitness improvements
        let mut total_score = 0.0;
        let mut scores: HashMap<Mutation, f64> = HashMap::new();

        for (&mut_type, &(total_delta, count)) in &success_counts {
            let avg_delta = if count > 0.0 { total_delta / count } else { 0.0 };
            let score = avg_delta.max(0.0) + 0.1; // Add a baseline to avoid zero scores
            scores.insert(mut_type, score);
            total_score += score;
        }

        if total_score > 0.0 {
            self.add_success_rate = *scores.get(&Mutation::Add).unwrap_or(&0.0) / total_score;
            self.remove_success_rate = *scores.get(&Mutation::Remove).unwrap_or(&0.0) / total_score;
            self.reorder_success_rate = *scores.get(&Mutation::Reorder).unwrap_or(&0.0) / total_score;
            self.duplicate_success_rate = *scores.get(&Mutation::Duplicate).unwrap_or(&0.0) / total_score;
            self.tune_success_rate = *scores.get(&Mutation::Tune).unwrap_or(&0.0) / total_score;

            // Normalize to sum to 1.0, handling potential floating point inaccuracies
            let current_sum = self.add_success_rate + self.remove_success_rate + self.reorder_success_rate + self.duplicate_success_rate + self.tune_success_rate;
            if current_sum != 0.0 {
                self.add_success_rate /= current_sum;
                self.remove_success_rate /= current_sum;
                self.reorder_success_rate /= current_sum;
                self.duplicate_success_rate /= current_sum;
                self.tune_success_rate /= current_sum;
            }
        }
    }

    fn compute_function_stats(&self, module: &Module) -> FunctionStats {
        // This should be more sophisticated, potentially iterating all functions or a hot one.
        // For now, simplify and assume first function for instruction_count, block_count, branch_count.
        FunctionStats {
            instruction_count: module.instruction_count(),
            block_count: module.block_count(),
            branch_count: module.branch_count(),
            ..Default::default()
        }
    }
}

impl ToString for Mutation {
    fn to_string(&self) -> String {
        match self {
            Mutation::Add => "add".to_string(),
            Mutation::Remove => "remove".to_string(),
            Mutation::Reorder => "reorder".to_string(),
            Mutation::Duplicate => "duplicate".to_string(),
            Mutation::Tune => "tune".to_string(),
        }
    }
}
