use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::fs;
use serde::{Serialize, Deserialize};
use crate::ir::module::Module;
use crate::teacher::MutationStep; // Re-use MutationStep from teacher.rs
use log::info;
use crate::ir::value::Instruction;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Blueprint {
    #[serde(rename = "goalId")]
    pub goal_id: String,
    #[serde(rename = "functionShapeHash")]
    pub function_shape_hash: String,
    #[serde(rename = "finalFitness")]
    pub final_fitness: f64,
    pub timestamp: String,
    pub partial: bool,
    pub steps: Vec<MutationStep>,
}

pub struct BlueprintArchive {
    directory: PathBuf,
    blueprints: Vec<Blueprint>,
}

impl BlueprintArchive {
    pub fn new(dir: &str) -> Self {
        let directory = PathBuf::from(dir);
        fs::create_dir_all(&directory).expect("Failed to create blueprint archive directory");
        let mut archive = Self {
            directory,
            blueprints: Vec::new(),
        };
        archive.load_all();
        archive
    }

    pub fn load_all(&mut self) {
        self.blueprints.clear();
        if !self.directory.exists() {
            return;
        }

        for entry in fs::read_dir(&self.directory).expect("Failed to read blueprint directory") {
            let entry = entry.expect("Failed to read directory entry");
            let path = entry.path();

            if path.extension().map_or(false, |ext| ext == "json") {
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(bp) = serde_json::from_str(&content) {
                        self.blueprints.push(bp);
                    } else {
                        eprintln!("Failed to parse blueprint JSON: {:?}", path);
                    }
                }
            }
        }
    }

    pub fn save(&self, bp: &Blueprint) {
        let mut filename_timestamp = bp.timestamp.clone();
        filename_timestamp = filename_timestamp.replace(':', "-");
        filename_timestamp = filename_timestamp.replace(' ', "_");

        let filename = format!("{}/{}_{}.json", self.directory.display(), bp.goal_id, filename_timestamp);

        match serde_json::to_string_pretty(bp) {
            Ok(json_string) => {
                match fs::write(&filename, json_string) {
                    Ok(_) => info!("[Archive] Saved blueprint to {}", filename),
                    Err(e) => eprintln!("[Archive] Failed to write blueprint to {}: {}", filename, e),
                }
            },
            Err(e) => eprintln!("[Archive] Failed to serialize blueprint to JSON: {}", e),
        }
    }

    pub fn compute_shape_hash(module: &Module) -> String {
        let ic = module.instruction_count();
        let bc = module.block_count();
        let brc = module.branch_count();

        // C++ version: sc = store_count, lc = loop_count
        let mut store_count = 0;
        let mut loop_count = 0;
        for func in &module.functions {
            for bb in &func.basic_blocks {
                let mut block_names: Vec<String> = func.basic_blocks.iter().map(|b| b.name.clone()).collect();
                for inst in &bb.instructions {
                    if let Instruction::Store { .. } = inst {
                        store_count += 1;
                    }
                    if let Instruction::Jump { label } = inst {
                        // Loop = jump back to an earlier block
                        if let Some(it) = block_names.iter().position(|n| n == &bb.name) {
                            if let Some(tgt) = block_names.iter().position(|n| n == label) {
                                if tgt < it { // Target block appears earlier in the list
                                    loop_count += 1;
                                }
                            }
                        }
                    }
                }
            }
        }

        format!("{}_{}_{}_{}_{}", ic, bc, brc, store_count, loop_count)
    }

    pub fn shapes_are_similar(h1: &str, h2: &str) -> bool {
        let parse_hash = |h: &str| -> Vec<i32> {
            h.split('_')
                .filter_map(|s| s.parse::<i32>().ok())
                .collect()
        };

        let v1 = parse_hash(h1);
        let v2 = parse_hash(h2);

        if v1.len() != 5 || v2.len() != 5 {
            return false; // Malformed hash
        }

        for i in 0..5 {
            let a = v1[i];
            let b = v2[i];

            if a == 0 && b == 0 {
                continue;
            }
            let mx = a.abs().max(b.abs());
            if mx == 0 {
                continue;
            }
            if (a as f64 - b as f64).abs() / mx as f64 > 0.20 {
                return false;
            }
        }
        true
    }

    pub fn find_best(&self, goal_id: &str, shape_hash: &str) -> Option<Blueprint> {
        let mut best_bp: Option<Blueprint> = None;
        let mut best_fitness = -1e9f64; // C++ default

        for bp in &self.blueprints {
            if bp.goal_id != goal_id {
                continue;
            }
            if !Self::shapes_are_similar(&bp.function_shape_hash, shape_hash) {
                continue;
            }
            if bp.final_fitness > best_fitness {
                best_fitness = bp.final_fitness;
                best_bp = Some(bp.clone());
            }
        }
        best_bp
    }

    pub fn list_goals(&self) -> Vec<String> {
        let mut goals = Vec::new();
        let mut unique_goals = HashSet::new();
        for bp in &self.blueprints {
            if unique_goals.insert(bp.goal_id.clone()) {
                goals.push(bp.goal_id.clone());
            }
        }
        goals
    }

    pub fn total_blueprints(&self) -> usize {
        self.blueprints.len()
    }

    pub fn get_directory(&self) -> &PathBuf {
        &self.directory
    }
}