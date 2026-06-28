use serde::{Deserialize, Serialize};
use crate::ir::basic_block::BasicBlock;
use crate::ir::value::ValueType;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Function {
    pub name: String,
    pub return_type: ValueType,
    pub basic_blocks: Vec<BasicBlock>,
    /// Formal parameter names, in order. Only meaningful for functions that
    /// get called from WITHIN the module (see Instruction::Call) — the
    /// interpreter binds call-site argument values to these names before
    /// executing the callee's body. Top-level entry functions (the ones
    /// execute_function is called on directly, not via a Call) don't need
    /// this populated: their "parameters" are already baked in as Constant
    /// stores in the entry block per the existing module_builders.rs /
    /// llvm_frontend.rs convention, so this defaults to empty for those.
    pub params: Vec<String>,
}

impl Function {
    pub fn new(name: String, return_type: ValueType) -> Self {
        Self { name, return_type, basic_blocks: Vec::new(), params: Vec::new() }
    }

    pub fn set_params(&mut self, params: Vec<String>) {
        self.params = params;
    }

    pub fn get_block(&self, name: &str) -> Option<&BasicBlock> {
        self.basic_blocks.iter().find(|b| b.name == name)
    }

    pub fn get_block_mut(&mut self, name: &str) -> Option<&mut BasicBlock> {
        self.basic_blocks.iter_mut().find(|b| b.name == name)
    }
}
