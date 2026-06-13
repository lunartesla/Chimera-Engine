use crate::ir::basic_block::BasicBlock;
use crate::ir::value::{Instruction, BinaryOp, ValueType};

#[test]
fn test_basic_block_new() {
    let bb = BasicBlock::new("entry".to_string());
    assert_eq!(bb.name, "entry");
    assert!(bb.instructions.is_empty());
}

#[test]
fn test_basic_block_append() {
    let mut bb = BasicBlock::new("entry".to_string());
    let inst1 = Instruction::Constant { value: 1 };
    let inst2 = Instruction::Variable { name: "x".to_string() };
    bb.append(inst1.clone());
    bb.append(inst2.clone());
    assert_eq!(bb.instructions.len(), 2);
    assert_eq!(bb.instructions[0], inst1);
    assert_eq!(bb.instructions[1], inst2);
}

#[test]
fn test_basic_block_terminator() {
    let mut bb = BasicBlock::new("entry".to_string());
    let inst1 = Instruction::Constant { value: 1 };
    let inst2 = Instruction::Jump { label: "exit".to_string() };
    bb.append(inst1.clone());
    bb.append(inst2.clone());
    assert_eq!(bb.terminator(), Some(&inst2));

    let mut bb2 = BasicBlock::new("entry2".to_string());
    bb2.append(inst1);
    assert_eq!(bb2.terminator(), None);
}
