use std::collections::HashMap;
use crate::ir::module::Module;
use crate::ir::value::{BinaryOp, Instruction};
use crate::passes::{Pass, PassError, PassSafety, PassDescriptor, ParamRange};

pub struct StrengthReductionPass;

impl StrengthReductionPass {
    pub fn new() -> Self {
        Self
    }

    // Helper function to check if a number is a power of two
    fn is_power_of_two(n: i64) -> bool {
        n > 0 && (n & (n - 1)) == 0
    }

    fn reduce_binary_op(op: BinaryOp, lhs: &Instruction, rhs: &Instruction) -> Option<Instruction> {
        match op {
            BinaryOp::Mul => {
                if let Instruction::Constant { value: l_val } = lhs {
                    if *l_val == 1 {
                        return Some(rhs.clone()); // 1 * x -> x
                    }
                    if *l_val == 0 {
                        return Some(Instruction::Constant { value: 0 }); // 0 * x -> 0
                    }
                }
                if let Instruction::Constant { value: r_val } = rhs {
                    if *r_val == 1 {
                        return Some(lhs.clone()); // x * 1 -> x
                    }
                    if *r_val == 0 {
                        return Some(Instruction::Constant { value: 0 }); // x * 0 -> 0
                    }
                }
            }
            BinaryOp::Add => {
                if let Instruction::Constant { value: l_val } = lhs {
                    if *l_val == 0 {
                        return Some(rhs.clone()); // 0 + x -> x
                    }
                }
                if let Instruction::Constant { value: r_val } = rhs {
                    if *r_val == 0 {
                        return Some(lhs.clone()); // x + 0 -> x
                    }
                }
            }
            _ => {}
        }
        None
    }
}

impl Pass for StrengthReductionPass {
    fn id(&self) -> &'static str {
        "strength_reduction"
    }

    fn name(&self) -> &'static str {
        "Strength Reduction"
    }

    fn description(&self) -> &'static str {
        "Replaces expensive operations with cheaper ones"
    }

    fn safety(&self) -> PassSafety {
        PassSafety::Risky
    }

    fn run(&self, module: &mut Module) -> Result<bool, PassError> {
        let mut changed = false;

        for func in &mut module.functions {
            for bb in &mut func.basic_blocks {
                for i in 0..bb.instructions.len() {
                    let mut current_inst = bb.instructions[i].clone(); // Clone to modify

                    if let Instruction::BinaryOp { op, lhs, rhs } = &mut current_inst {
                        if let Some(reduced_inst) = StrengthReductionPass::reduce_binary_op(*op, lhs, rhs) {
                            bb.instructions[i] = reduced_inst;
                            changed = true;
                        } else {
                            // Handle x * power_of_2 (identify and mark, no actual replacement in C++ code)
                            // We will consider it a change for reporting purposes if this pattern is found
                            let mut identified_power_of_two = false;
                            if let Instruction::Constant { value: l_val } = &**lhs {
                                if Self::is_power_of_two(*l_val) {
                                    identified_power_of_two = true;
                                }
                            }
                            if let Instruction::Constant { value: r_val } = &**rhs {
                                if Self::is_power_of_two(*r_val) {
                                    identified_power_of_two = true;
                                }
                            }
                            if identified_power_of_two {
                                changed = true; // Mark as changed for reporting, matching C++ behavior
                            }
                        }
                    }
                }
            }
        }

        Ok(changed)
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
            vec![], // No parameters for Strength Reduction
        )
    }
}