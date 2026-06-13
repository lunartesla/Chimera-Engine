use std::collections::HashMap;
use crate::ir::module::Module;
use crate::ir::value::{Instruction, ValueType};
use crate::passes::{Pass, PassError, PassSafety, PassDescriptor, ParamRange};

pub struct BlockMergingPass {
    params: HashMap<String, ParamRange>,
}

impl BlockMergingPass {
    pub fn new() -> Self {
        let mut params = HashMap::new();
        params.insert(
            "max_iterations".to_string(),
            ParamRange::new("max_iterations", 5, 1, 20, 1),
        );
        Self { params }
    }
}

impl Pass for BlockMergingPass {
    fn id(&self) -> &'static str {
        "block_merge"
    }

    fn name(&self) -> &'static str {
        "Block Merging"
    }

    fn description(&self) -> &'static str {
        "Merges basic blocks when possible to reduce control flow overhead"
    }

    fn safety(&self) -> PassSafety {
        PassSafety::Safe
    }

    fn run(&self, module: &mut Module) -> Result<bool, PassError> {
        let mut changed_any_iter = false;
        let max_iterations = self.get_param("max_iterations").unwrap_or(5);

        for _ in 0..max_iterations {
            let mut changed_this_iter = false;
            // blocks_to_remove no longer needed — merges happen inline

            // Iterate over functions
            for func_idx in 0..module.functions.len() {
                let func = &mut module.functions[func_idx];
                let mut current_block_idx = 0;

                while current_block_idx < func.basic_blocks.len() {
                    let mut merge_candidate_found = false;
                    let block_name = func.basic_blocks[current_block_idx].name.clone();

                    // Extract target_name without holding a mutable borrow
                    let target_name_opt: Option<String> = {
                        let block = &func.basic_blocks[current_block_idx];
                        if let Some(Instruction::Jump { label }) = block.instructions.last() {
                            if label != &block.name { Some(label.clone()) } else { None }
                        } else { None }
                    };

                    if let Some(target_name) = target_name_opt {
                        // Count predecessors using only immutable borrows
                        let mut actual_predecessor_count = 0;
                        for p_block in func.basic_blocks.iter() {
                            for p_instr in &p_block.instructions {
                                match p_instr {
                                    Instruction::Branch { then_label, else_label, .. } => {
                                        if then_label == &target_name || else_label == &target_name {
                                            actual_predecessor_count += 1;
                                        }
                                    }
                                    Instruction::Jump { label } => {
                                        if label == &target_name {
                                            actual_predecessor_count += 1;
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }

                        let target_block_idx_opt = func.basic_blocks.iter().position(|b| b.name == target_name);

                        if actual_predecessor_count == 1 {
                            if let Some(target_block_idx) = target_block_idx_opt {
                                // Collect target instructions first (immutable)
                                let target_instrs: Vec<Instruction> = func.basic_blocks[target_block_idx].instructions.clone();

                                // Now mutate: remove Jump from current block, append target instrs
                                let block = &mut func.basic_blocks[current_block_idx];
                                block.instructions.pop(); // remove Jump
                                block.instructions.extend(target_instrs);

                                // Remove target block
                                func.basic_blocks.remove(target_block_idx);
                                merge_candidate_found = true;
                                changed_this_iter = true;
                            }
                        }
                    } // end if let Some(target_name)

                    if !merge_candidate_found {
                        current_block_idx += 1;
                    }
                }
            }
            if !changed_this_iter {
                break; // No changes in this iteration
            }
            changed_any_iter = true;
        }

        Ok(changed_any_iter)
    }

    fn get_param(&self, name: &str) -> Option<i32> {
        self.params.get(name).map(|p| p.current)
    }

    fn set_param(&mut self, name: &str, value: i32) -> bool {
        if let Some(param) = self.params.get_mut(name) {
            param.set_current(value);
            true
        } else {
            false
        }
    }

    fn descriptor(&self) -> PassDescriptor {
        PassDescriptor::new(
            self.id(),
            self.name(),
            self.description(),
            self.safety(),
            self.params.values().cloned().collect(),
        )
    }
}