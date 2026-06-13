use crate::ir::module::Module;
use thiserror::Error;
use std::collections::HashMap;

#[derive(Debug, Error)]
pub enum PassError {
    #[error("Pass failed: {0}")]
    Generic(String),
    #[error("Parameter '{0}' not found")]
    ParamNotFound(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PassSafety {
    Safe,
    Conservative,
    Risky,
}

#[derive(Debug, Clone)]
pub struct ParamRange {
    pub name: String,
    pub default: i32,
    pub min: i32,
    pub max: i32,
    pub step: i32,
    pub current: i32, // Current value of the parameter
}

impl ParamRange {
    pub fn new(name: &str, default_val: i32, min_val: i32, max_val: i32, step_val: i32) -> Self {
        Self {
            name: name.to_string(),
            default: default_val,
            min: min_val,
            max: max_val,
            step: step_val,
            current: default_val, // Initialize current with default
        }
    }

    pub fn set_current(&mut self, value: i32) {
        self.current = value.max(self.min).min(self.max);
    }
}

#[derive(Debug, Clone)]
pub struct PassDescriptor {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub safety: PassSafety,
    pub params: HashMap<String, ParamRange>, // Using HashMap for easy lookup by name
}

impl PassDescriptor {
    pub fn new(
        id: &'static str,
        name: &'static str,
        description: &'static str,
        safety: PassSafety,
        param_ranges: Vec<ParamRange>,
    ) -> Self {
        let mut params_map = HashMap::new();
        for param in param_ranges {
            params_map.insert(param.name.clone(), param);
        }
        Self {
            id,
            name,
            description,
            safety,
            params: params_map,
        }
    }

    pub fn get_param(&self, name: &str) -> Option<i32> {
        self.params.get(name).map(|p| p.current)
    }
}


pub trait Pass: Send + Sync {
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn safety(&self) -> PassSafety;
    fn run(&self, module: &mut Module) -> Result<bool, PassError>;
    fn get_param(&self, name: &str) -> Option<i32>;
    fn set_param(&mut self, name: &str, value: i32) -> bool;
    fn descriptor(&self) -> PassDescriptor; // Returns a descriptor for this pass instance
}

pub struct PassManager {
    passes: Vec<Box<dyn Pass>>,
}

impl PassManager {
    pub fn new() -> Self {
        Self { passes: Vec::new() }
    }

    pub fn add(&mut self, pass: Box<dyn Pass>) {
        self.passes.push(pass);
    }

    pub fn remove(&mut self, index: usize) {
        if index < self.passes.len() {
            self.passes.remove(index);
        }
    }

    pub fn run_all(&self, module: &mut Module) -> Result<bool, PassError> {
        let mut any_changed = false;
        for pass in &self.passes {
            any_changed |= pass.run(module)?;
        }
        Ok(any_changed)
    }

    pub fn passes(&self) -> &[Box<dyn Pass>] {
        &self.passes
    }

    pub fn populate_from_level(&mut self, level: OptimizationLevel, registry: &PassRegistry) {
        self.passes.clear();
        match level {
            OptimizationLevel::Safe => {
                if let Some(pass) = registry.create_pass("constant_folding") {
                    self.add(pass);
                }
            }
            OptimizationLevel::Conservative => {
                if let Some(pass) = registry.create_pass("constant_folding") {
                    self.add(pass);
                }
                if let Some(pass) = registry.create_pass("dead_code") {
                    self.add(pass);
                }
                if let Some(pass) = registry.create_pass("cse") {
                    self.add(pass);
                }
                if let Some(pass) = registry.create_pass("loop_unroll") {
                    self.add(pass);
                }
            }
            OptimizationLevel::Balanced => {
                if let Some(pass) = registry.create_pass("constant_folding") {
                    self.add(pass);
                }
                if let Some(pass) = registry.create_pass("dead_code") {
                    self.add(pass);
                }
                if let Some(pass) = registry.create_pass("cse") {
                    self.add(pass);
                }
                if let Some(pass) = registry.create_pass("loop_unroll") {
                    self.add(pass);
                }
                if let Some(pass) = registry.create_pass("constant_propagation") {
                    self.add(pass);
                }
                if let Some(pass) = registry.create_pass("block_merge") {
                    self.add(pass);
                }
                if let Some(pass) = registry.create_pass("strength_reduction") {
                    self.add(pass);
                }
            }
        }
    }

    pub fn pass_count(&self) -> usize {
        self.passes.len()
    }

    pub fn get_pass_info(&self, index: usize) -> Option<PassDescriptor> {
        self.passes.get(index).map(|p| p.descriptor())
    }
}

#[derive(Debug, Clone, Copy)]
pub enum OptimizationLevel {
    Safe,
    Conservative,
    Balanced,
}

// Module re-exports for passes
pub mod constant_folding;
pub mod dead_code_elimination;
pub mod constant_propagation;
pub mod strength_reduction;
pub mod cse;
pub mod block_merging;
pub mod loop_unroll;
pub mod pass_registry;

pub use constant_folding::ConstantFoldingPass;
pub use dead_code_elimination::DeadCodeEliminationPass;
pub use constant_propagation::ConstantPropagationPass;
pub use strength_reduction::StrengthReductionPass;
pub use cse::CsePass;
pub use block_merging::BlockMergingPass;
pub use loop_unroll::LoopUnrollPass;
pub use pass_registry::PassRegistry;