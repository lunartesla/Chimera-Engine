use std::collections::HashMap;
use crate::ir::module::Module;
use crate::ir::value::{BinaryOp, Instruction, ValueType};
use crate::passes::{Pass, PassError, PassSafety, PassDescriptor, ParamRange};

pub struct CsePass;

impl CsePass {
    pub fn new() -> Self {
        Self
    }
}

impl Pass for CsePass {
    fn id(&self) -> &'static str {
        "cse"
    }

    fn name(&self) -> &'static str {
        "Common Subexpression Elimination"
    }

    fn description(&self) -> &'static str {
        "Eliminates redundant computations within basic blocks"
    }

    fn safety(&self) -> PassSafety {
        PassSafety::Conservative
    }

    fn run(&self, module: &mut Module) -> Result<bool, PassError> {
        let mut changed = false;
        let mut temp_var_counter = 0;

        // Process each basic block independently
        for func in &mut module.functions {
            for bb in &mut func.basic_blocks {
                // Map from expression signature to the variable name that holds its result
                let mut expr_to_var_name: HashMap<String, String> = HashMap::new();
                let mut new_instructions = Vec::new();

                for instr_idx in 0..bb.instructions.len() {
                    let mut current_instr = bb.instructions[instr_idx].clone(); // Clone to modify if needed

                    // Only consider BinaryOp instructions for CSE
                    if let Instruction::BinaryOp { op, lhs, rhs } = &mut current_instr {
                        // Only consider BinaryOp with Variable or Constant operands for CSE
                        let lhs_is_simple = matches!(**lhs, Instruction::Variable { .. } | Instruction::Constant { .. });
                        let rhs_is_simple = matches!(**rhs, Instruction::Variable { .. } | Instruction::Constant { .. });

                        if !lhs_is_simple || !rhs_is_simple {
                            // Skip complex operands, just add the instruction to the new list
                            new_instructions.push(current_instr);
                            continue;
                        }

                        // Create string representations of operands
                        let signature = format!("{}|{}|{}", op.display(), lhs.display(), rhs.display());

                        // Check if we've seen this expression before in this block
                        if let Some(var_name) = expr_to_var_name.get(&signature) {
                            // Found a common subexpression! Replace with variable holding previous result
                            new_instructions.push(Instruction::Variable { name: var_name.clone() });
                            changed = true;
                        } else {
                            // First time seeing this expression in this block.
                            // Generate a temporary variable name to hold the result
                            temp_var_counter += 1;
                            let temp_var_name = format!("__cse_temp_{}", temp_var_counter);
                            expr_to_var_name.insert(signature, temp_var_name.clone());

                            // Insert a Store instruction for the new temporary variable *before* the current instruction
                            // This part deviates from the C++ to keep the IR valid.
                            // The C++ version in the description replaces the BinaryOp with a Variable.
                            // This would lose the computation. A store is needed to preserve the value.
                            // The C++ `CSEPass` in the prompt actually just replaces the `BinaryOp` with a `Variable`.
                            // This means the result of the `BinaryOp` is simply used directly as a `Variable`.
                            // This is a key difference. I will match the prompt's description precisely first.
                            // The prompt says: "replace BinaryOp with Variable(temp_var_name)"
                            // The C++ source code example in `src/Passes/CSEPass.cpp` also directly replaces.
                            // So the current instruction `current_instr` (which is a BinaryOp) should be replaced with a Variable
                            // but the value must be defined earlier. The C++ code in the example was slightly incomplete in this aspect,
                            // assuming a separate definition.

                            // Re-reading C++ `CSEPass.cpp`:
                            // block->instructions[i] = std::make_shared<Variable>(varName);
                            // This implies the result of the BinaryOp is not stored explicitly, but rather the Variable is a reference
                            // to where the result *would have been* if the computation was done.
                            // This looks problematic, but I will follow the spirit of the text in the prompt.
                            // If it's the *first* time, we keep the BinaryOp and associate its result with `temp_var_name`.
                            // Then *later uses* replace the BinaryOp with a Variable.

                            // To properly replicate, the CSE pass should identify the first instance of a subexpression,
                            // calculate its result, store that result in a new temporary variable, and then replace
                            // all subsequent instances of that subexpression with a reference to that temporary variable.
                            // The C++ code is implicitly assuming some form of SSA or value numbering already exists or
                            // that the IR is such that a variable directly represents the result of its defining instruction.

                            // Let's refine based on typical CSE. The C++ just replaces with variable, but the first one still needs to compute.
                            // The prompt's description "insert Store for temp" implies this.
                            // The C++ `CSEPass.cpp` was simplified and only replaced subsequent uses.
                            // I will add the instruction to compute the value for the first occurrence.
                            // The prompt states: "On first occurrence: record signature → temp var name; insert Store for temp"
                            // "On subsequent occurrence: replace BinaryOp with Variable(temp_var_name)"

                            new_instructions.push(Instruction::Store {
                                var_name: temp_var_name.clone(),
                                value: Box::new(current_instr.clone()), // The original BinaryOp is stored
                            });
                            new_instructions.push(Instruction::Variable { name: temp_var_name.clone() }); // And its result is used
                            changed = true; // Count this as a change because we added a store and a variable use
                        }
                    } else {
                        // Not a BinaryOp, just add it to the new list
                        new_instructions.push(current_instr);
                    }
                }
                bb.instructions = new_instructions;
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
            vec![], // No parameters for CSE
        )
    }
}