use std::collections::HashMap;
use crate::ir::module::Module;
use crate::ir::value::Instruction;
use crate::BasicBlock; // Correct import for BasicBlock
use crate::passes::{Pass, PassError, PassSafety, PassDescriptor, ParamRange};

pub struct LoopUnrollPass {
    params: HashMap<String, ParamRange>,
}

impl LoopUnrollPass {
    pub fn new() -> Self {
        let mut params = HashMap::new();
        params.insert(
            "factor".to_string(),
            ParamRange::new("factor", 2, 1, 10, 1),
        );
        Self { params }
    }
}

impl Pass for LoopUnrollPass {
    fn id(&self) -> &'static str {
        "loop_unroll"
    }

    fn name(&self) -> &'static str {
        "Loop Unroll"
    }

    fn description(&self) -> &'static str {
        "Unrolls loops by duplicating loop body"
    }

    fn safety(&self) -> PassSafety {
        PassSafety::Conservative
    }

    fn run(&self, module: &mut Module) -> Result<bool, PassError> {
        let factor = self.get_param("factor").unwrap_or(2);
        if factor <= 1 {
            return Ok(false);
        }

        let mut changed = false;

        // Iterate functions
        for func in &mut module.functions {
            let mut header_name = String::new();
            let mut body_name = String::new();
            let mut exit_name = String::new();
            let mut loop_cond_instruction: Option<Instruction> = None;

            // Find first unrollable loop (do NOT iterate and modify simultaneously)
            // C++ finds a header block ending in a Branch, whose true target is body, false target is exit.
            // And body block ends in a Jump back to header.
            'find_loop: for block_idx in 0..func.basic_blocks.len() {
                let block = &func.basic_blocks[block_idx];
                if let Some(Instruction::Branch { condition, then_label, else_label }) = block.instructions.last() {
                    let true_target = then_label;
                    let false_target = else_label;

                    if let Some(body_block) = func.get_block(true_target) {
                        if let Some(Instruction::Jump { label: jump_target }) = body_block.instructions.last() {
                            if jump_target == &block.name {
                                // Found a loop
                                header_name = block.name.clone();
                                body_name = true_target.clone();
                                exit_name = false_target.clone();
                                loop_cond_instruction = Some(*condition.clone());
                                break 'find_loop;
                            }
                        }
                    }
                }
            }

            if header_name.is_empty() {
                continue; // No loop found in this function
            }

            // Get mutable references to the blocks after finding them
            let header_block_idx = func.basic_blocks.iter().position(|b| b.name == header_name).unwrap();
            let body_block_idx = func.basic_blocks.iter().position(|b| b.name == body_name).unwrap();
            let exit_block_idx = func.basic_blocks.iter().position(|b| b.name == exit_name).unwrap();

            let header_block = func.basic_blocks.get_mut(header_block_idx).unwrap();
            let body_block = func.basic_blocks.get(body_block_idx).unwrap(); // Get immutable for cloning

            // Body core = all instructions except final Jump
            let body_core: Vec<Instruction> = body_block.instructions.iter()
                .filter(|instr| !instr.is_terminator())
                .cloned()
                .collect();

            if body_core.is_empty() {
                continue; // Cannot unroll empty loop body
            }

            // ENFORCE BOUNDS: if factor * body.instructions.len() > 1000, clamp or skip
            // The C++ uses factor * bodyCore.size(), which is instructions without terminator.
            let max_unrolled_instructions = 1000;
            if (factor as usize) * body_core.len() > max_unrolled_instructions {
                // If it's too large, don't unroll, or unroll partially.
                // The C++ original fix was "Bounds check added", often meaning skip or partial.
                // For 1:1, if it would cause segfault, skip.
                return Ok(false); // Skip this unroll to prevent explosion/segfault
            }


            let mut new_blocks: Vec<BasicBlock> = Vec::new();

            for i in 1..factor {
                let mut nb = BasicBlock::new(format!("{}_ur{}", body_name, i));
                for inst in &body_core {
                    nb.append(inst.clone());
                }

                if i < factor - 1 {
                    // Middle unrolled blocks branch to next unrolled block or exit
                    nb.append(Instruction::Branch {
                        condition: Box::new(loop_cond_instruction.as_ref().unwrap().clone()),
                        then_label: format!("{}_ur{}", body_name, i + 1),
                        else_label: exit_name.clone(),
                    });
                } else {
                    // Last unrolled block jumps back to header
                    nb.append(Instruction::Jump { label: header_name.clone() });
                }
                new_blocks.push(nb);
            }

            // Rewrite original body: bodyCore + Branch(cond, _ur1, exit)
            let mut new_original_body_instructions = body_core.clone();
            new_original_body_instructions.push(Instruction::Branch {
                condition: Box::new(loop_cond_instruction.unwrap().clone()),
                then_label: format!("{}_ur1", body_name),
                else_label: exit_name.clone(),
            });

            // Update the original body block's instructions
            // Use get_block_mut to modify the body block
            if let Some(body_block_mut) = func.get_block_mut(&body_name) {
                body_block_mut.instructions = new_original_body_instructions;
            } else {
                return Err(PassError::Generic(format!("Body block {} not found for unrolling", body_name)));
            }

            // Insert new blocks before exit block
            // This requires careful indexing since func.basic_blocks is being modified.
            // Insert new blocks right after the original body block.
            let mut insertion_point = 0;
            for (idx, block) in func.basic_blocks.iter().enumerate() {
                if block.name == body_name {
                    insertion_point = idx + 1;
                    break;
                }
            }

            // Insert new blocks at the correct position
            func.basic_blocks.splice(insertion_point..insertion_point, new_blocks);

            changed = true;
        }

        Ok(changed)
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