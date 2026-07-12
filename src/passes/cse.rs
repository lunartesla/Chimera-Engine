use std::collections::HashMap;
use crate::ir::module::Module;
use crate::ir::value::Instruction;
use crate::passes::{Pass, PassError, PassSafety, PassDescriptor};

pub struct CsePass;

impl CsePass {
    pub fn new() -> Self {
        Self
    }

    fn is_simple(inst: &Instruction) -> bool {
        matches!(inst, Instruction::Variable { .. } | Instruction::Constant { .. })
    }

    fn signature(op_display: &str, lhs: &Instruction, rhs: &Instruction) -> String {
        format!("{}|{}|{}", op_display, lhs.display(), rhs.display())
    }

    /// Recursively replaces any BinaryOp subtree (with simple Variable/
    /// Constant operands, matching the original scope) whose signature is
    /// ALREADY known — from an earlier top-level Store in this block — with
    /// a direct reference to that Store's own variable name. Pure
    /// substitution only: never manufactures a new temp/Store, so every
    /// change this makes strictly reduces (or, for a lone unmatched node,
    /// leaves unchanged) instruction count. It never inflates it.
    ///
    /// This intentionally does NOT catch every possible duplicate — a
    /// subexpression whose only occurrences are both buried somewhere with
    /// no natural top-level Store to name either one (e.g. nested two levels
    /// inside a Branch condition on both sides) won't be caught, since there
    /// is no existing name to substitute and creating one would cost an
    /// instruction. That's a real, narrower-than-ideal scope, chosen
    /// deliberately: the previous version's attempt at hoisting unconditionally
    /// added a Store+Variable pair on every first sighting regardless of
    /// whether a duplicate ever showed up, which made it INFLATE instruction
    /// count on the common case (a lone, non-duplicated BinaryOp) — worse
    /// than doing nothing. This version only ever helps, never hurts.
    fn replace_known_duplicates(inst: &mut Instruction, known: &HashMap<String, String>, changed: &mut bool) {
        match inst {
            Instruction::BinaryOp { op, lhs, rhs } => {
                Self::replace_known_duplicates(lhs, known, changed);
                Self::replace_known_duplicates(rhs, known, changed);
                if Self::is_simple(lhs) && Self::is_simple(rhs) {
                    let sig = Self::signature(op.display(), lhs, rhs);
                    if let Some(existing_name) = known.get(&sig) {
                        *inst = Instruction::Variable { name: existing_name.clone() };
                        *changed = true;
                    }
                }
            }
            Instruction::Store { value, .. } => {
                Self::replace_known_duplicates(value, known, changed);
            }
            Instruction::Compare { lhs, rhs, .. } => {
                Self::replace_known_duplicates(lhs, known, changed);
                Self::replace_known_duplicates(rhs, known, changed);
            }
            Instruction::Branch { condition, .. } => {
                Self::replace_known_duplicates(condition, known, changed);
            }
            Instruction::Return { value: Some(v) } => {
                Self::replace_known_duplicates(v, known, changed);
            }
            Instruction::Return { value: None } | Instruction::Jump { .. } => {}
            Instruction::Call { args, .. } => {
                for a in args {
                    Self::replace_known_duplicates(a, known, changed);
                }
            }
            Instruction::AllocaArray { .. } => {}
            Instruction::GetElementPtr { base, index } => {
                Self::replace_known_duplicates(base, known, changed);
                Self::replace_known_duplicates(index, known, changed);
            }
            Instruction::LoadPtr { ptr } => {
                Self::replace_known_duplicates(ptr, known, changed);
            }
            Instruction::StorePtr { ptr, value } => {
                Self::replace_known_duplicates(ptr, known, changed);
                Self::replace_known_duplicates(value, known, changed);
            }
            Instruction::Constant { .. } | Instruction::Variable { .. } => {}
        }
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
        "Replaces a recomputed expression (anywhere in the tree) with a reference to an earlier Store that already computed the same thing"
    }

    fn safety(&self) -> PassSafety {
        PassSafety::Conservative
    }

    fn run(&self, module: &mut Module) -> Result<bool, PassError> {
        let mut changed = false;

        for func in &mut module.functions {
            for bb in &mut func.basic_blocks {
                // signature -> the variable name of the earliest top-level
                // Store whose value directly computed that expression.
                let mut known: HashMap<String, String> = HashMap::new();

                for inst in &mut bb.instructions {
                    // First, replace any nested duplicate against what's
                    // already known from EARLIER instructions in this block
                    // (this instruction's own top-level Store, if any, is
                    // deliberately not registered until after this step —
                    // an instruction can't be a duplicate of itself).
                    Self::replace_known_duplicates(inst, &known, &mut changed);

                    // Then, if this is a Store whose value is (now, possibly
                    // after the substitution above) a qualifying BinaryOp
                    // not already known, register it at zero cost — no new
                    // instruction, just reusing the name this Store already
                    // has for real.
                    if let Instruction::Store { var_name, value } = inst {
                        if let Instruction::BinaryOp { op, lhs, rhs } = value.as_ref() {
                            if Self::is_simple(lhs) && Self::is_simple(rhs) {
                                let sig = Self::signature(op.display(), lhs, rhs);
                                known.entry(sig).or_insert_with(|| var_name.clone());
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
            vec![],
        )
    }
}
