use std::collections::HashSet;
use crate::ir::module::Module;
use crate::ir::value::Instruction;
use crate::passes::{Pass, PassError, PassSafety, PassDescriptor, ParamRange};
use std::collections::HashMap;

pub struct DeadCodeEliminationPass;

impl DeadCodeEliminationPass {
    pub fn new() -> Self {
        Self
    }

    // Recursively collect all Variable names referenced in an instruction tree
    fn collect_used_vars(inst: &Instruction, used: &mut HashSet<String>) {
        match inst {
            Instruction::Constant { .. } => {}
            Instruction::Variable { name } => {
                used.insert(name.clone());
            }
            Instruction::BinaryOp { lhs, rhs, .. } => {
                Self::collect_used_vars(lhs, used);
                Self::collect_used_vars(rhs, used);
            }
            Instruction::Compare { lhs, rhs, .. } => {
                Self::collect_used_vars(lhs, used);
                Self::collect_used_vars(rhs, used);
            }
            Instruction::Store { value, .. } => {
                // The VALUE of a store is used, but the var_name is a DEFINITION
                Self::collect_used_vars(value, used);
            }
            Instruction::Branch { condition, .. } => {
                Self::collect_used_vars(condition, used);
            }
            Instruction::Jump { .. } => {}
            Instruction::Return { value: Some(val_inst) } => {
                Self::collect_used_vars(val_inst, used);
            }
            Instruction::Return { value: None } => {}
        }
    }
}

impl Pass for DeadCodeEliminationPass {
    fn id(&self) -> &'static str {
        "dead_code"
    }

    fn name(&self) -> &'static str {
        "Dead Code Elimination"
    }

    fn description(&self) -> &'static str {
        "Removes stores to variables that are never read"
    }

    fn safety(&self) -> PassSafety {
        PassSafety::Safe
    }

    fn run(&self, module: &mut Module) -> Result<bool, PassError> {
        let mut changed_any_iter = false;

        loop {
            let mut changed_this_iter = false;
            let mut used_vars = HashSet::new();

            // Collect ALL variable names that are used anywhere in the function
            for func in &module.functions {
                for bb in &func.basic_blocks {
                    for inst in &bb.instructions {
                        Self::collect_used_vars(inst, &mut used_vars);
                    }
                }
            }

            // Remove Store instructions whose target variable is never used
            for func in &mut module.functions {
                for bb in &mut func.basic_blocks {
                    let mut i = 0;
                    while i < bb.instructions.len() {
                        let is_last = i == bb.instructions.len() - 1;
                        if let Instruction::Store { var_name, .. } = &bb.instructions[i] {
                            // Never remove the last instruction (terminator safety)
                            // A Store cannot be a terminator, but this check ensures it doesn't break basic block structure
                            if !is_last && !used_vars.contains(var_name) {
                                bb.instructions.remove(i);
                                changed_this_iter = true;
                                continue; // Don't increment i, check the new instruction at current position
                            }
                        }
                        i += 1;
                    }
                }
            }

            if !changed_this_iter {
                break; // No changes in this iteration, we're done
            }
            changed_any_iter = true;
        }

        Ok(changed_any_iter)
    }

    fn get_param(&self, _name: &str) -> Option<i32> {
        None
    }

    fn set_param(&mut self, _name: &str, _value: i32) -> bool {
        false
    }

    fn descriptor(&self) -> PassDescriptor {
        PassDescriptor::new(
            self.id(),
            self.name(),
            self.description(),
            self.safety(),
            vec![], // No parameters for DCE
        )
    }
}