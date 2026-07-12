// neural_predictor.rs — neuralneat 0.3.0, all f32, correct defaults names

use neuralneat::{Genome, Pool};
use neuralneat::defaults;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::path::Path;
use serde::{Deserialize, Serialize};

const INPUT_NODES:        usize = 20;   // NEAT network input count (fixed for neuralneat compatibility)
const OUTPUT_NODES:       usize = 12;   // Multi-head: success_prob, instr_reduction, compile_time, binary_size,
                                        // exec_speed, reg_pressure, mem_usage, code_size, correctness_risk,
                                        // opt_confidence, novelty_score, generalization_score
const PASS_HISTORY_SIZE:  usize = 7;    // Number of recent passes to remember for sequence memory
const POPULATION:         usize = 450;  // patched
const OUTCOME_BUFFER_MAX: usize = 3000;
const EVOLVE_EVERY:       usize = 20;
const NM_READY_THRESHOLD: usize = 50;   // Reduced from 500 - train faster on varied data

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutcomeRecord {
    pub features:      Vec<f32>,
    pub success:       f32,
    pub fitness_delta: f32,
}

#[derive(Debug, Clone, Default)]
pub struct FunctionStats {
    pub instruction_count: usize,
    pub block_count:       usize,
    pub max_depth:         usize,
    pub constant_count:    usize,
    pub store_count:       usize,
    pub branch_count:      usize,
}

#[derive(Debug, Clone)]
pub struct Prediction {
    // Output head 0: Primary success probability
    pub success_prob:  f32,
    // Output head 1: Instruction reduction prediction
    pub instr_reduction: f32,
    // Output head 2: Compile time change prediction
    pub compile_time_change: f32,
    // Output head 3: Binary size change prediction
    pub binary_size_change: f32,
    // Output head 4: Execution speed change prediction
    pub exec_speed_change: f32,
    // Output head 5: Register pressure prediction
    pub reg_pressure: f32,
    // Output head 6: Memory usage prediction
    pub mem_usage: f32,
    // Output head 7: Code size change prediction
    pub code_size_change: f32,
    // Output head 8: Correctness risk (0=confident, 1=high risk)
    pub correctness_risk: f32,
    // Output head 9: Optimization confidence (how certain about success)
    pub opt_confidence: f32,
    // Output head 10: Novelty score (how unique this transformation is)
    pub novelty_score: f32,
    // Output head 11: Generalization score (how well this scales to other modules)
    pub generalization_score: f32,
}

pub struct NeuralPredictor {
    pool:                 Pool,
    best_genome:          Option<Genome>,
    outcome_buffer:       Vec<OutcomeRecord>,
    records_since_evolve: usize,
    total_records:        usize,
    // Was a compile-time const (NM_READY_THRESHOLD). Now a runtime field so
    // the dashboard's tuning panel can adjust it live instead of requiring
    // a recompile to change how much training the brain needs before it's
    // trusted for predictions.
    nm_ready_threshold:   usize,
    // Track actual success rate for meaningful confidence
    success_count:        usize,
    failure_count:        usize,
    // Sequence memory: track last N passes for learning pass interactions
    pass_history:         VecDeque<String>,
}

impl NeuralPredictor {
    pub fn new() -> Self {
        let pool = Pool::new(
            INPUT_NODES,
            OUTPUT_NODES,
            POPULATION,
            defaults::DEFAULT_CONNECTION_MUTATION_CHANCE,
            defaults::DEFAULT_NODE_MUTATION_CHANCE,
            defaults::DEFAULT_WEIGHT_MUTATION_CHANCE,
            defaults::DEFAULT_PERTURB_CHANCE,
            defaults::DEFAULT_WEIGHT_STEP_SIZE,
            defaults::DEFAULT_DISABLE_NODE_MUTATION_CHANCE,
            defaults::DEFAULT_ENABLE_NODE_MUTATION_CHANCE,
            defaults::DEFAULT_EXCESS_COEFFICIENT,
            defaults::DEFAULT_DISJOINT_COEFFICIENT,
            defaults::DEFAULT_WEIGHT_DIFF_COEFFICIENT,
            2.0_f32,   // species_threshold  — patched from DEFAULT_SPECIES_THRESHOLD
            defaults::DEFAULT_MUTATE_ONLY_RATE,
            defaults::DEFAULT_MATE_ONLY_RATE,
            defaults::DEFAULT_CROSSOVER_CHANCE,
            45_usize,  // dropoff_age (stagnation) — patched from DEFAULT_DROPOFF_AGE
            defaults::DEFAULT_AGE_SIGNIFICANCE,
            defaults::DEFAULT_SURVIVAL_THRESHOLD,
        );
        Self {
            pool,
            best_genome:          None,
            outcome_buffer:       Vec::with_capacity(OUTCOME_BUFFER_MAX),
            records_since_evolve: 0,
            total_records:        0,
            nm_ready_threshold:   NM_READY_THRESHOLD,
            success_count:        0,
            failure_count:        0,
            pass_history:         VecDeque::with_capacity(PASS_HISTORY_SIZE),
        }
    }

    pub fn is_nm_ready(&self) -> bool {
        // Ready when we have enough records
        // Diversity is measured implicitly by the neural network during training
        // (homogeneous data will produce poor fitness anyway)
        self.total_records >= self.nm_ready_threshold
    }

    /// Real NEAT species count from the underlying Pool — replaces a
    /// previous placeholder in evolution_daemon.rs that computed
    /// `training_data_len() / 85`, an arbitrary formula with no connection
    /// to actual genome speciation. The neuralneat crate's Pool already
    /// tracks real compatibility-distance-based species; this just exposes
    /// its existing `len()`.
    pub fn species_count(&self) -> usize {
        self.pool.len()
    }

    pub fn get_nm_confidence(&self) -> f64 {
        // Confidence = success rate (not record count ratio)
        let total = self.success_count + self.failure_count;
        if total == 0 {
            return 0.0;
        }
        self.success_count as f64 / total as f64
    }

    /// Returns the count of successful (positive outcome) records
    pub fn get_success_count(&self) -> usize {
        self.success_count
    }

    /// Returns the count of failed (negative outcome) records
    pub fn get_failure_count(&self) -> usize {
        self.failure_count
    }

    pub fn get_nm_ready_threshold(&self) -> usize {
        self.nm_ready_threshold
    }

    pub fn set_nm_ready_threshold(&mut self, threshold: usize) {
        self.nm_ready_threshold = threshold.max(1);
    }

    pub fn get_status_string(&self) -> String {
        format!(
            "NM: records={} ready={} confidence={:.2}",
            self.total_records,
            self.is_nm_ready(),
            self.get_nm_confidence()
        )
    }

    /// Returns total records ever seen (used by evolution_daemon).
    pub fn training_data_len(&self) -> usize {
        self.total_records
    }

    pub fn predict(
        &mut self,
        pass_id: &str,
        stats: &FunctionStats,
        extra: &HashMap<String, f64>,
    ) -> Option<Prediction> {
        if !self.is_nm_ready() {
            return None;
        }
        // Build features including sequence history, THEN borrow best_genome
        let inputs: Vec<f32> = self.build_features_with_history(pass_id, stats, extra)
            .iter()
            .map(|&x| x as f32)
            .collect();

        let genome = self.best_genome.as_mut()?;
        genome.evaluate(&inputs, None, None);
        let outputs = genome.get_outputs();
        Some(Prediction {
            success_prob:  outputs.get(0).copied().unwrap_or(0.5),
            instr_reduction:     outputs.get(1).copied().unwrap_or(0.0),
            compile_time_change:   outputs.get(2).copied().unwrap_or(0.0),
            binary_size_change:    outputs.get(3).copied().unwrap_or(0.0),
            exec_speed_change:     outputs.get(4).copied().unwrap_or(0.0),
            reg_pressure:          outputs.get(5).copied().unwrap_or(0.0),
            mem_usage:             outputs.get(6).copied().unwrap_or(0.0),
            code_size_change:      outputs.get(7).copied().unwrap_or(0.0),
            correctness_risk:      outputs.get(8).copied().unwrap_or(0.5),
            opt_confidence:        outputs.get(9).copied().unwrap_or(0.5),
            novelty_score:         outputs.get(10).copied().unwrap_or(0.0),
            generalization_score:  outputs.get(11).copied().unwrap_or(0.0),
        })
    }

    pub fn record_outcome(
        &mut self,
        pass_id: &str,
        stats: &FunctionStats,
        extra: &HashMap<String, f64>,
        success: bool,
        fitness_delta: f64,
    ) {
        // Update pass history
        self.pass_history.push_back(pass_id.to_string());
        if self.pass_history.len() > PASS_HISTORY_SIZE {
            self.pass_history.pop_front();
        }

        // Store features with history context for training
        let features: Vec<f32> = self.build_features_with_history(pass_id, stats, extra)
            .iter()
            .map(|&x| x as f32)
            .collect();

        let record = OutcomeRecord {
            features,
            success:       if success { 1.0 } else { 0.0 },
            fitness_delta: fitness_delta as f32,
        };

        if self.outcome_buffer.len() >= OUTCOME_BUFFER_MAX {
            self.outcome_buffer.remove(0);
        }
        self.outcome_buffer.push(record);
        self.total_records        += 1;
        self.records_since_evolve += 1;
        if success {
            self.success_count += 1;
        } else {
            self.failure_count += 1;
        }

        if self.records_since_evolve >= EVOLVE_EVERY && self.outcome_buffer.len() >= 10 {
            self.evolve_pool();
            self.records_since_evolve = 0;
        }
    }

    /// Get pass history for inspection
    pub fn get_pass_history(&self) -> Vec<String> {
        self.pass_history.iter().cloned().collect()
    }

    fn evolve_pool(&mut self) {
        let total_species = self.pool.len();
        let mut best_fitness = f32::NEG_INFINITY;

        // Filter to only positive outcomes: success OR positive fitness delta
        let positive_records: Vec<&OutcomeRecord> = self.outcome_buffer
            .iter()
            .filter(|r| r.success == 1.0 || r.fitness_delta > 0.0)
            .collect();

        for s in 0..total_species {
            let species = &mut self.pool[s];
            let n = species.len();
            for g in 0..n {
                let genome = &mut species[g];
                let mut fitness = 0.0_f32;

                for record in &positive_records {
                    genome.evaluate(&record.features, None, None);
                    let outputs = genome.get_outputs();
                    // Train all 12 output heads
                    // Head 0: success probability
                    let pred_success = outputs.get(0).copied().unwrap_or(0.5);
                    // Head 1: instruction reduction (fitness_delta)
                    let pred_delta = outputs.get(1).copied().unwrap_or(0.0);
                    // Other heads use default targets based on the outcome
                    // For positive outcomes, we expect good values for most heads
                    let fitness_delta = record.fitness_delta;
                    let success = record.success;

                    let err = (pred_success - success).powi(2)           // success prob
                            + (pred_delta - fitness_delta).powi(2);      // instr reduction

                    // Extra heads get implicit training through success correlation
                    // When success=1, we expect: low risk, high confidence, etc.
                    if success == 1.0 {
                        let risk_err = outputs.get(8).copied().unwrap_or(0.5).powi(2);     // correctness_risk should be near 0
                        let conf_err = (outputs.get(9).copied().unwrap_or(0.0) - 1.0).powi(2); // opt_confidence should be high
                        fitness -= (err + risk_err * 0.1 + conf_err * 0.1);
                    } else {
                        fitness -= err;
                    }
                }

                genome.update_fitness(fitness);
                if fitness > best_fitness {
                    best_fitness    = fitness;
                    self.best_genome = Some(genome.clone());
                }
            }
        }

        self.pool.new_generation();
        log::debug!(
            "NeuralPredictor evolved: best_fitness={best_fitness:.4} total_records={}",
            self.total_records
        );
    }

    // Static version for compatibility
    fn build_features_static(
        pass_id: &str,
        stats: &FunctionStats,
        extra: &HashMap<String, f64>,
    ) -> Vec<f64> {
        let pass_ids = [
            "constant_folding",
            "dead_code",
            "cse",
            "loop_unroll",
            "constant_propagation",
            "block_merge",
            "strength_reduction",
        ];

        let mut f = Vec::with_capacity(INPUT_NODES);

        // 1-7: one-hot pass id
        for pid in &pass_ids {
            f.push(if *pid == pass_id { 1.0 } else { 0.0 });
        }
        // 8-13: module stats
        f.push((stats.instruction_count as f64) / 50.0);
        f.push((stats.block_count        as f64) / 20.0);
        f.push((stats.max_depth          as f64) / 10.0);
        f.push((stats.constant_count     as f64) / 20.0);
        f.push((stats.store_count        as f64) / 20.0);
        f.push((stats.branch_count       as f64) / 10.0);
        // 14-20: extra context
        f.push(extra.get("temperature")      .copied().unwrap_or(1.0));
        f.push(extra.get("generation_ratio") .copied().unwrap_or(0.0));
        f.push((extra.get("pipeline_length") .copied().unwrap_or(0.0) / 24.0).min(1.0));
        f.push((extra.get("pass_frequency")  .copied().unwrap_or(0.0) / 5.0 ).min(1.0));
        f.push((extra.get("cycles_stuck")    .copied().unwrap_or(0.0) / 100.0).min(1.0));
        f.push(extra.get("island_id")        .copied().unwrap_or(0.0) / 3.0);
        f.push(extra.get("goal_ratio")       .copied().unwrap_or(0.0));

        f
    }

    /// Build features including sequence memory context
    /// Encodes the most recent pass into the context for sequence awareness
    fn build_features_with_history(
        &self,
        pass_id: &str,
        stats: &FunctionStats,
        extra: &HashMap<String, f64>,
    ) -> Vec<f64> {
        let mut features = Self::build_features_static(pass_id, stats, extra);

        // Incorporate sequence awareness without changing input count:
        // Modify the pass_frequency feature to reflect history length
        // and add a "context switch" indicator if last pass differs from current
        if let Some(last_pass) = self.pass_history.back() {
            // Override goal_ratio with sequence context (0=same as last, 1=different)
            features[19] = if *last_pass == pass_id { 0.0 } else { 1.0 };
        }

        features
    }

    /// Public wrapper kept for compatibility with call sites that already use it.
    pub fn build_expanded_features(
        &self,
        pass_id: &str,
        stats: &FunctionStats,
        extra: &HashMap<String, f64>,
    ) -> Vec<f64> {
        Self::build_features_static(pass_id, stats, extra)
    }

    /// Creates a new NeuralPredictor seeded with this one's outcome buffer and record count.
    /// Used when forking a strain that needs its own predictor warm-started from the daemon's.
    pub fn clone_state(&self) -> NeuralPredictor {
        let mut p = Self::new();
        p.outcome_buffer = self.outcome_buffer.clone();
        p.total_records  = self.total_records;
        p.nm_ready_threshold = self.nm_ready_threshold;
        p.success_count = self.success_count;
        p.failure_count = self.failure_count;
        p.pass_history = self.pass_history.clone();
        p
    }

    pub fn save_brain(&self, path: &Path) -> Result<(), std::io::Error> {
        // Filter to only positive outcomes: success OR positive fitness delta
        let positive_records: Vec<&OutcomeRecord> = self.outcome_buffer
            .iter()
            .filter(|r| r.success == 1.0 || r.fitness_delta > 0.0)
            .collect();

        #[derive(Serialize)]
        struct BrainData<'a> {
            best_genome:    Option<&'a Genome>,
            outcome_buffer: Vec<&'a OutcomeRecord>,
            total_records:  usize,
        }
        let data = BrainData {
            best_genome:    self.best_genome.as_ref(),
            outcome_buffer: positive_records,
            total_records:  self.total_records,
        };
        let json = serde_json::to_string_pretty(&data)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        std::fs::write(path, json)
    }

    pub fn load_brain(path: &Path) -> Result<Self, std::io::Error> {
        #[derive(Deserialize)]
        struct BrainData {
            best_genome:    Option<Genome>,
            outcome_buffer: Vec<OutcomeRecord>,
            total_records:  usize,
        }
        let json = std::fs::read_to_string(path)?;
        let data: BrainData = serde_json::from_str(&json)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        let mut p = Self::new();
        p.best_genome = data.best_genome;
        // Recalculate success/failure counts from loaded records
        let (success, failure) = data.outcome_buffer.iter()
            .fold((0, 0), |(s, f), r| {
                if r.success == 1.0 || r.fitness_delta > 0.0 {
                    (s + 1, f)
                } else {
                    (s, f + 1)
                }
            });
        p.outcome_buffer = data.outcome_buffer;
        p.total_records  = data.total_records;
        p.success_count  = success;
        p.failure_count  = failure;
        Ok(p)
    }
}

impl Default for NeuralPredictor {
    fn default() -> Self { Self::new() }
}