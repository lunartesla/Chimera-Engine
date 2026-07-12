use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Full experience record with all metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperienceRecord {
    // Problem identification
    pub module_shape_hash: String,
    pub goal_id: String,
    pub function_stats: FunctionStats,

    // Solution applied
    pub pipeline_id: String,
    pub passes: Vec<ExperiencePassDescriptor>,

    // Outcome metrics
    pub fitness: f64,
    pub baseline_instructions: usize,
    pub final_instructions: usize,
    pub instruction_reduction: usize,
    pub validation_passed: bool,
    pub execution_time_us: f64,
    pub memory_peak_bytes: usize,

    // Context
    pub generation: u64,
    pub island_id: Option<usize>,
    pub strain_id: Option<String>,
    pub temperature: f64,
    pub seed_fitness: f64,

    // Metadata
    pub timestamp: u64,
    pub run_id: String,
    pub success: bool,
    pub correction_needed: bool,
}

/// Function statistics for experience context
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FunctionStats {
    pub instruction_count: usize,
    pub block_count: usize,
    pub max_depth: usize,
    pub constant_count: usize,
    pub store_count: usize,
    pub branch_count: usize,
}

/// Pass descriptor for experience storage (serializable version)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperiencePassDescriptor {
    pub id: String,
    pub params: HashMap<String, i32>,
}

/// Query for experience replay
#[derive(Debug, Clone, Default)]
pub struct ReplayQuery {
    pub module_shape_hash: Option<String>,
    pub goal_id: Option<String>,
    pub min_fitness: f64,
    pub max_instructions: Option<usize>,
    pub limit: usize,
    pub time_range: Option<(u64, u64)>,
}

impl ReplayQuery {
    pub fn new() -> Self {
        Self { limit: 1000, ..Default::default() }
    }

    pub fn with_shape(mut self, hash: impl Into<String>) -> Self {
        self.module_shape_hash = Some(hash.into());
        self
    }

    pub fn with_goal(mut self, goal: impl Into<String>) -> Self {
        self.goal_id = Some(goal.into());
        self
    }

    pub fn with_min_fitness(mut self, min: f64) -> Self {
        self.min_fitness = min;
        self
    }

    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = limit;
        self
    }
}