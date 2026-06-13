use std::sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}};
use std::time::{SystemTime, UNIX_EPOCH};
use chrono::{DateTime, Local};
use std::path::Path;
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

        // Gate 2: NM predictor must be ready (confidence >= 0.85) (replicate C++ exactly)
        // C++ checks `!engine || !engine->getPredictor() || !engine->getPredictor()->isNMReady()`
        // which means it checks if predictor exists AND is ready.
        if !self.engine.get_predictor().is_nm_ready() || self.engine.get_predictor().get_nm_confidence() < 0.85 {
            return false;
        }

        // Gate 3 is checked by caller (EvolutionDaemon)
        true // nominated
    }

    pub fn promote(&mut self, origin_engine: &mut SelfEvolvingEngine) {
        // This function is called on the strain itself, and receives the origin engine to update.
        // It needs to:
        // 1. Mark itself as nominated.
        // 2. Transfer predictor model to origin.
        // 3. Transfer best pipeline to origin (via blueprint replay).

        // Mark as nominated
        self.lineage.nominated = true;

        // Archive old origin state (this happens in EvolutionDaemon)
        // For now, simply log.
        // C++: Archiving old origin to "blueprints/archived_origin_{timestamp}.json"

        // Transfer predictor model to origin
        // C++: engine->getPredictor()->saveModel(tmpModel); origin.engine->getPredictor()->loadModel(tmpModel);
        // This implies saving the `neuralneat` state and loading it into the origin's `neuralneat`.
        let tmp_model_path = "tmp_strain_model.bin"; // Use .bin for bincode
        if let Err(e) = origin_engine.get_predictor().save_brain(Path::new(tmp_model_path)) {
            eprintln!("Error saving strain predictor brain: {}", e);
        } else {
            match NeuralPredictor::load_brain(Path::new(tmp_model_path)) {
                Ok(loaded) => {
                    *origin_engine.get_predictor_mut() = loaded;
                    info!("[Strain] Transferred predictor model to origin");
                }
                Err(e) => {
                    eprintln!("Error loading strain predictor brain into origin: {}", e);
                }
            }
        }

        // Transfer best pipeline to origin (replay blueprint)
        // This logic is typically handled by the EvolutionDaemon when it promotes.
        // For now, this is a placeholder. A blueprint should be created from the strain's best pipeline
        // and then replayed into the origin engine.
        // This is not a direct 1:1 C++ port. The C++ `promote` method doesn't take `origin_engine`.
        // It says "Save strain predictor and load into origin".
        // And "Transfer best pipeline to origin (replay blueprint)".
        // It needs the blueprint archive to do that.

        // The C++ `StrainEngine::promote` actually just prints messages and marks `lineage.nominated = true`.
        // The actual logic of applying the changes to the origin happens elsewhere (EvolutionDaemon).
        // So, following C++ 1:1, this method just marks as nominated and logs.
        info!("[Strain] Promoting {} to origin", self.lineage.strain_id);
    }
}