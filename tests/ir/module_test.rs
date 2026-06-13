use crate::ir::module::Module;
use crate::ir::function::Function;
use crate::ir::basic_block::BasicBlock;
use crate::ir::value::{ValueType, Instruction, BinaryOp, CompareCondition};

#[test]
fn test_module_new() {
    let module = Module::new("my_module".to_string());
    assert_eq!(module.name, "my_module");
    assert!(module.functions.is_empty());
}

#[test]
fn test_module_instruction_count() {
    let mut module = Module::new("test".to_string());
    let mut func = Function::new("f1".to_string(), ValueType::Int);
    let mut bb1 = BasicBlock::new("entry".to_string());
    bb1.instructions.push(Instruction::Constant { value: 1 });
    bb1.instructions.push(Instruction::Return { value: None });
    func.basic_blocks.push(bb1);
    module.functions.push(func);

    assert_eq!(module.instruction_count(), 2);

    let mut func2 = Function::new("f2".to_string(), ValueType::Int);
    let mut bb2 = BasicBlock::new("entry2".to_string());
    bb2.instructions.push(Instruction::Constant { value: 2 });
    bb2.instructions.push(Instruction::Jump { label: "exit2".to_string() });
    func2.basic_blocks.push(bb2);
    module.functions.push(func2);

    assert_eq!(module.instruction_count(), 4); // 2 from f1 + 2 from f2
}

#[test]
fn test_module_block_count() {
    let mut module = Module::new("test".to_string());
    let mut func = Function::new("f1".to_string(), ValueType::Int);
    func.basic_blocks.push(BasicBlock::new("entry".to_string()));
    func.basic_blocks.push(BasicBlock::new("exit".to_string()));
    module.functions.push(func);

    assert_eq!(module.block_count(), 2);

    let mut func2 = Function::new("f2".to_string(), ValueType::Int);
    func2.basic_blocks.push(BasicBlock::new("entry2".to_string()));
    module.functions.push(func2);

    assert_eq!(module.block_count(), 3); // 2 from f1 + 1 from f2
}

#[test]
fn test_module_branch_count() {
    let mut module = Module::new("test".to_string());
    let mut func = Function::new("f1".to_string(), ValueType::Int);
    let mut bb1 = BasicBlock::new("entry".to_string());
    bb1.instructions.push(Instruction::Branch {
        condition: Box::new(Instruction::Constant { value: 1 }),
        then_label: "then".to_string(),
        else_label: "else".to_string(),
    });
    func.basic_blocks.push(bb1);
    module.functions.push(func);

    assert_eq!(module.branch_count(), 1);

    let mut func2 = Function::new("f2".to_string(), ValueType::Int);
    let mut bb2 = BasicBlock::new("entry2".to_string());
    bb2.instructions.push(Instruction::Constant { value: 0 });
    bb2.instructions.push(Instruction::Branch {
        condition: Box::new(Instruction::Constant { value: 1 }),
        then_label: "then2".to_string(),
        else_label: "else2".to_string(),
    });
    bb2.instructions.push(Instruction::Jump { label: "end".to_string() }); // Should not count as branch
    func2.basic_blocks.push(bb2);
    module.functions.push(func2);

    assert_eq!(module.branch_count(), 2); // 1 from f1 + 1 from f2
}
