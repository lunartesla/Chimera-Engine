use serde::{Deserialize, Serialize};
use crate::ir::function::Function;
use crate::ir::value::Instruction;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Module {
    pub name: String,
    pub functions: Vec<Function>,
}

impl Module {
    pub fn new(name: String) -> Self {
        Self { name, functions: Vec::new() }
    }

    pub fn instruction_count(&self) -> usize {
        self.functions.iter().map(|f| {
            f.basic_blocks.iter().map(|bb| bb.instructions.len()).sum::<usize>()
        }).sum()
    }

    pub fn block_count(&self) -> usize {
        self.functions.iter().map(|f| f.basic_blocks.len()).sum()
    }

    pub fn branch_count(&self) -> usize {
        self.functions.iter().map(|f| {
            f.basic_blocks.iter().map(|bb| {
                bb.instructions.iter().filter(|instr| {
                    matches!(instr, Instruction::Branch { .. })
                }).count()
            }).sum::<usize>()
        }).sum()
    }

    pub fn get_function(&self, name: &str) -> Option<&Function> {
        self.functions.iter().find(|f| f.name == name)
    }

    pub fn get_function_mut(&mut self, name: &str) -> Option<&mut Function> {
        self.functions.iter_mut().find(|f| f.name == name)
    }
}
