use serde::{Deserialize, Serialize};
use crate::ir::basic_block::BasicBlock;
use crate::ir::value::ValueType;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Function {
    pub name: String,
    pub return_type: ValueType,
    pub basic_blocks: Vec<BasicBlock>,
}

impl Function {
    pub fn new(name: String, return_type: ValueType) -> Self {
        Self { name, return_type, basic_blocks: Vec::new() }
    }

    pub fn get_block(&self, name: &str) -> Option<&BasicBlock> {
        self.basic_blocks.iter().find(|b| b.name == name)
    }

    pub fn get_block_mut(&mut self, name: &str) -> Option<&mut BasicBlock> {
        self.basic_blocks.iter_mut().find(|b| b.name == name)
    }
}
