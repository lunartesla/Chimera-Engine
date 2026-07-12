use std::sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}};
use std::time::{SystemTime, UNIX_EPOCH};
use chrono::{DateTime, Local};
use log::info;

use crate::self_evolving_engine::SelfEvolvingEngine;
use crate::neural_predictor::NeuralPredictor; // Assuming NeuralPredictor is used
use crate::goal_definition::GoalDefinition; // Assuming GoalDefinition is used
use crate::passes::{OptimizationLevel, PassDescriptor}; // For pipeline information

pub struct StrainLineage {
    pub strain_id: String,        // unique ID e.g. "strain_001"
    pub parent_id: String,        // "origin" for first gen
    pub generation: i32,          // 0 = original, 1 = first fork, etc.
    pub task_class: String,       // what task triggered the fork
    pub mod_name: String,         // which module in the library this strain forked from —
                                   // task_class is the GOAL id (e.g. "free_evolution"), not
                                   // the module name; conflating the two silently misfiled
                                   // every promoted result under the wrong key (see
                                   // EvolutionDaemon::check_promotions).
    pub fork_timestamp: String,
    pub fitness_at_fork: f64,
    pub generations_run: i32,
    pub nominated: bool,          // nominated for promotion?
    pub archived: bool,
    // Note: C++ had `nominated` and `archived` as separate bools in the struct.
    // The design doc had `gate_level` on Strain, but C++ `StrainLineage` does not.
    // Sticking to C++'s `StrainLineage` fields for 1:1 port.
}

pub struct StrainEngine {
    engine: SelfEvolvingEngine,
    lineage: StrainLineage,
    // Removed `strain_training_data` as SelfEvolvingEngine already manages it.
}

impl StrainEngine {
    pub fn get_engine(&self) -> &SelfEvolvingEngine {
        &self.engine
    }

    pub fn get_engine_mut(&mut self) -> &mut SelfEvolvingEngine {
        &mut self.engine
    }

    pub fn new(
        mut engine: SelfEvolvingEngine,
        strain_id: &str,
        parent_id: &str,
        generation: i32,
        task_class: &str,
        mod_name: &str,
        fitness_at_fork: f64,
    ) -> Self {
        let now = SystemTime::now();
        let datetime: DateTime<Local> = now.into();
        let fork_timestamp = datetime.format("%Y-%m-%dT%H-%M-%S").to_string();

        Self {
            engine,
            lineage: StrainLineage {
                strain_id: strain_id.to_string(),
                parent_id: parent_id.to_string(),
                generation,
                task_class: task_class.to_string(),
                mod_name: mod_name.to_string(),
                fork_timestamp,
                fitness_at_fork,
                generations_run: 0,
                nominated: false,
                archived: false,
            },
        }
    }

    pub fn evolve(&mut self, generations: u32, wildcard: bool) {
        self.engine.evolve(generations, wildcard);
        self.lineage.generations_run += generations as i32;
    }

    pub fn evolve_to_goal(&mut self, goal: GoalDefinition, wildcard: bool) -> bool {
        let reached = self.engine.evolve_to_goal(goal.clone(), wildcard);
        self.lineage.generations_run += goal.max_generations as i32; // Assuming max_generations passed are always run
        reached
    }

    pub fn set_external_stop_flag(&mut self, flag: Arc<AtomicBool>) {
        self.engine.set_external_stop_flag(flag);
    }

    pub fn get_best_fitness(&self) -> f64 {
        self.engine.get_best_fitness()
    }

    pub fn get_best_pipeline(&self) -> &[PassDescriptor] {
        self.engine.get_best_pipeline()
    }

    pub fn get_predictor(&self) -> &NeuralPredictor {
        self.engine.get_predictor()
    }

    pub fn get_predictor_mut(&mut self) -> &mut NeuralPredictor {
        self.engine.get_predictor_mut()
    }

    pub fn get_lineage(&self) -> &StrainLineage {
        &self.lineage
    }

    pub fn get_lineage_mut(&mut self) -> &mut StrainLineage {
        &mut self.lineage
    }

    pub fn should_promote(&mut self) -> bool {
        // Gate 1: must have run >= 500 generations (exact from src/StrainEngine.cpp)
        if self.lineage.generations_run < 500 {
            return false;
        }

        // Gate 2: NM predictor must be ready AND have positive success rate
        // Original C++ used confidence >= 0.85, but that was misleading:
        // confidence was just record_count / threshold, not actual success rate.
        // Fixed: require both enough records AND success rate > 50%.
        let predictor = self.engine.get_predictor();
        if !predictor.is_nm_ready() || predictor.get_nm_confidence() <= 0.5 {
            return false;
        }

        // Gate 3 is checked by caller (EvolutionDaemon)
        true // nominated
    }

    pub fn promote(&mut self, origin_engine: &mut SelfEvolvingEngine) {
        // Mark as nominated
        self.lineage.nominated = true;

        // Transfer predictor model to origin using clone_state (direct in-memory transfer)
        // This replaces the file-based save/load approach with the same pattern used elsewhere.
        *origin_engine.get_predictor_mut() = self.get_predictor().clone_state();
        info!("[Strain] Transferred predictor model to origin");

        info!("[Strain] Promoting {} to origin", self.lineage.strain_id);
    }
}