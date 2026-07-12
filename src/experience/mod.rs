//! Experience System for Metamorphic Engine
//!
//! Accumulates and preserves optimization outcomes for replay, curriculum learning,
//! and knowledge discovery. Supports tiered storage (RAM → SQLite → archival).

mod schema;
mod database;

pub use schema::{ExperienceRecord, ReplayQuery, FunctionStats, ExperiencePassDescriptor};
pub use database::ExperienceDatabase;

use std::path::Path;
use std::sync::Arc;
use anyhow::Result;
use std::sync::Mutex;
use std::collections::HashMap;

/// Main experience interface combining hot cache with persistent storage
pub struct ExperienceSystem {
    hot_cache: Mutex<HashMap<String, Vec<ExperienceRecord>>>,
    database: ExperienceDatabase,
    config: ExperienceConfig,
}

#[derive(Debug, Clone)]
pub struct ExperienceConfig {
    pub hot_capacity: usize,
    pub db_path: std::path::PathBuf,
    pub curriculum_enabled: bool,
}

impl Default for ExperienceConfig {
    fn default() -> Self {
        Self {
            hot_capacity: 100_000,
            db_path: std::path::PathBuf::from("experience.db"),
            curriculum_enabled: true,
        }
    }
}

impl ExperienceSystem {
    pub fn open(config: ExperienceConfig) -> Result<Self> {
        let database = ExperienceDatabase::open(&config.db_path)?;

        Ok(Self {
            hot_cache: Mutex::new(HashMap::new()),
            database,
            config,
        })
    }

    /// Record an optimization outcome
    pub fn observe(&self, record: ExperienceRecord) -> Result<()> {
        let key = format!("{}_{}", record.module_shape_hash, record.goal_id);

        {
            let mut cache = self.hot_cache.lock().unwrap();
            cache.entry(key)
                .or_insert_with(Vec::new)
                .push(record.clone());

            // Enforce capacity
            for entries in cache.values_mut() {
                if entries.len() > self.config.hot_capacity {
                    entries.drain(0..entries.len() - self.config.hot_capacity);
                }
            }
        }

        // Async write to database (fire and forget for performance)
        self.database.insert(&record)?;

        Ok(())
    }

    /// Query experiences for replay
    pub fn replay(&self, query: &ReplayQuery) -> Result<Vec<ExperienceRecord>> {
        // First check hot cache
        if let Some(shape) = &query.module_shape_hash {
            if let Some(goal) = &query.goal_id {
                let cache = self.hot_cache.lock().unwrap();
                let key = format!("{}_{}", shape, goal);
                if let Some(entries) = cache.get(&key) {
                    let filtered: Vec<ExperienceRecord> = entries
                        .iter()
                        .filter(|r| r.fitness >= query.min_fitness)
                        .take(query.limit)
                        .cloned()
                        .collect();
                    if !filtered.is_empty() {
                        return Ok(filtered);
                    }
                }
            }
        }

        // Fall back to database
        self.database.query(query)
    }

    /// Get current curriculum stage (stub for Phase 3B)
    pub fn curriculum_stage(&self) -> CurriculumStage {
        CurriculumStage::Novice
    }
}

/// Curriculum progression stages
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CurriculumStage {
    Novice,
    Intermediate,
    Advanced,
    Expert,
}