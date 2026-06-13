use serde::{Deserialize, Serialize};
use crate::ir::value::Instruction;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BasicBlock {
    pub name: String,
    pub instructions: Vec<Instruction>,
}

impl BasicBlock {
    pub fn new(name: String) -> Self {
        Self { name, instructions: Vec::new() }
    }

    pub fn append(&mut self, inst: Instruction) {
        self.instructions.push(inst);
    }

    pub fn terminator(&self) -> Option<&Instruction> {
        self.instructions.last().filter(|i| i.is_terminator())
    }
}
