use std::collections::HashMap;
use crate::passes::{Pass, PassDescriptor, OptimizationLevel, PassSafety, PassError, ParamRange};
use crate::passes::constant_folding::ConstantFoldingPass;
use crate::passes::dead_code_elimination::DeadCodeEliminationPass;
use crate::passes::constant_propagation::ConstantPropagationPass;
use crate::passes::strength_reduction::StrengthReductionPass;
use crate::passes::cse::CsePass;
use crate::passes::block_merging::BlockMergingPass;
use crate::passes::loop_unroll::LoopUnrollPass;

pub type PassFactory = fn() -> Box<dyn Pass>;

#[derive(Clone)]
pub struct PassRegistry {
    catalog: HashMap<String, PassFactory>,
}

impl PassRegistry {
    pub fn new() -> Self {
        let mut registry = Self { catalog: HashMap::new() };
        registry.register_all_built_in_passes();
        registry
    }

    pub fn register_pass(&mut self, id: String, factory: PassFactory) {
        self.catalog.insert(id, factory);
    }

    pub fn create_pass(&self, id: &str) -> Option<Box<dyn Pass>> {
        self.catalog.get(id).map(|factory| factory())
    }

    pub fn get_descriptor(&self, id: &str) -> Option<PassDescriptor> {
        self.create_pass(id).map(|p| p.descriptor())
    }

    pub fn list_all(&self) -> Vec<String> {
        self.catalog.keys().cloned().collect()
    }

    pub fn exists(&self, id: &str) -> bool {
        self.catalog.contains_key(id)
    }

    pub fn register_all_built_in_passes(&mut self) {
        // These will be implemented in Phase 2
        self.register_pass(ConstantFoldingPass::new().id().to_string(), || Box::new(ConstantFoldingPass::new()));
        self.register_pass(DeadCodeEliminationPass::new().id().to_string(), || Box::new(DeadCodeEliminationPass::new()));
        self.register_pass(ConstantPropagationPass::new().id().to_string(), || Box::new(ConstantPropagationPass::new()));
        self.register_pass(StrengthReductionPass::new().id().to_string(), || Box::new(StrengthReductionPass::new()));
        self.register_pass(CsePass::new().id().to_string(), || Box::new(CsePass::new()));
        self.register_pass(BlockMergingPass::new().id().to_string(), || Box::new(BlockMergingPass::new()));
        self.register_pass(LoopUnrollPass::new().id().to_string(), || Box::new(LoopUnrollPass::new()));
    }
}