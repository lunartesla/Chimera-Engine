// neural_predictor.rs — neuralneat 0.3.0, all f32, correct defaults names

use neuralneat::{Genome, Pool};
use neuralneat::defaults;
use std::collections::HashMap;
use std::path::Path;
use serde::{Deserialize, Serialize};

const INPUT_NODES:        usize = 20;
const OUTPUT_NODES:       usize = 2;    // [0] success_prob  [1] fitness_delta
const POPULATION:         usize = 450;  // patched
const OUTCOME_BUFFER_MAX: usize = 3000;
const EVOLVE_EVERY:       usize = 20;
const NM_READY_THRESHOLD: usize = 500;

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
    pub success_prob:  f32,
    pub fitness_delta: f32,
}

pub struct NeuralPredictor {
    pool:                 Pool,
    best_genome:          Option<Genome>,
    outcome_buffer:       Vec<OutcomeRecord>,
    records_since_evolve: usize,
    total_records:        usize,
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
        }
    }

    pub fn is_nm_ready(&self) -> bool {
        self.total_records >= NM_READY_THRESHOLD
    }

    pub fn get_nm_confidence(&self) -> f64 {
        (self.total_records as f64 / NM_READY_THRESHOLD as f64).min(1.0)
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
        // Build features first, THEN borrow best_genome — avoids simultaneous borrow.
        let inputs: Vec<f32> = Self::build_features_static(pass_id, stats, extra)
            .iter()
            .map(|&x| x as f32)
            .collect();

        let genome = self.best_genome.as_mut()?;
        genome.evaluate(&inputs, None, None);
        let outputs = genome.get_outputs();
        Some(Prediction {
            success_prob:  outputs.get(0).copied().unwrap_or(0.5),
            fitness_delta: outputs.get(1).copied().unwrap_or(0.0),
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
        let features: Vec<f32> = Self::build_features_static(pass_id, stats, extra)
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

        if self.records_since_evolve >= EVOLVE_EVERY && self.outcome_buffer.len() >= 10 {
            self.evolve_pool();
            self.records_since_evolve = 0;
        }
    }

    fn evolve_pool(&mut self) {
        let total_species = self.pool.len();
        let mut best_fitness = f32::NEG_INFINITY;

        for s in 0..total_species {
            let species = &mut self.pool[s];
            let n = species.len();
            for g in 0..n {
                let genome = &mut species[g];
                let mut fitness = 0.0_f32;

                for record in &self.outcome_buffer {
                    genome.evaluate(&record.features, None, None);
                    let outputs = genome.get_outputs();
                    let pred_success = outputs.get(0).copied().unwrap_or(0.0);
                    let pred_delta   = outputs.get(1).copied().unwrap_or(0.0);
                    let err = (pred_success - record.success).powi(2)
                            + (pred_delta   - record.fitness_delta).powi(2);
                    fitness -= err;
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

    // Static version so predict() can call it without a second &self borrow.
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

        assert_eq!(f.len(), INPUT_NODES);
        f
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
        p
    }

    pub fn save_brain(&self, path: &Path) -> Result<(), std::io::Error> {
        #[derive(Serialize)]
        struct BrainData<'a> {
            best_genome:    Option<&'a Genome>,
            outcome_buffer: &'a Vec<OutcomeRecord>,
            total_records:  usize,
        }
        let data = BrainData {
            best_genome:    self.best_genome.as_ref(),
            outcome_buffer: &self.outcome_buffer,
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
        let mut p       = Self::new();
        p.best_genome    = data.best_genome;
        p.outcome_buffer = data.outcome_buffer;
        p.total_records  = data.total_records;
        Ok(p)
    }
}

impl Default for NeuralPredictor {
    fn default() -> Self { Self::new() }
}