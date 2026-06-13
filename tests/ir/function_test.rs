use crate::ir::function::Function;
use crate::ir::basic_block::BasicBlock;
use crate::ir::value::{ValueType, Instruction};

#[test]
fn test_function_new() {
    let func = Function::new("my_func".to_string(), ValueType::Int);
    assert_eq!(func.name, "my_func");
    assert_eq!(func.return_type, ValueType::Int);
    assert!(func.basic_blocks.is_empty());
}

#[test]
fn test_function_get_block() {
    let mut func = Function::new("my_func".to_string(), ValueType::Int);
    let bb1 = BasicBlock::new("entry".to_string());
    let bb2 = BasicBlock::new("exit".to_string());
    func.basic_blocks.push(bb1.clone());
    func.basic_blocks.push(bb2.clone());

    assert_eq!(func.get_block("entry").map(|b| &b.name), Some(&"entry".to_string()));
    assert_eq!(func.get_block("exit").map(|b| &b.name), Some(&"exit".to_string()));
    assert_eq!(func.get_block("non_existent"), None);
}

#[test]
fn test_function_get_block_mut() {
    let mut func = Function::new("my_func".to_string(), ValueType::Int);
    let bb1 = BasicBlock::new("entry".to_string());
    func.basic_blocks.push(bb1.clone());

    let mut_bb = func.get_block_mut("entry").expect("Should find entry block");
    mut_bb.instructions.push(Instruction::Constant { value: 10 });

    assert_eq!(func.get_block("entry").unwrap().instructions.len(), 1);
}
