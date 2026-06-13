use metamorphic_engine::ir::value::{BinaryOp, CompareCondition, Instruction, ValueType};

#[test]
fn test_value_type_enum() {
    assert_eq!(std::mem::discriminant(&ValueType::Int), std::mem::discriminant(&ValueType::Int));
    assert_eq!(std::mem::discriminant(&ValueType::Float), std::mem::discriminant(&ValueType::Float));
    assert_eq!(std::mem::discriminant(&ValueType::Void), std::mem::discriminant(&ValueType::Void));
}

#[test]
fn test_binary_op_enum() {
    assert_eq!(std::mem::discriminant(&BinaryOp::Add), std::mem::discriminant(&BinaryOp::Add));
    assert_eq!(std::mem::discriminant(&BinaryOp::Sub), std::mem::discriminant(&BinaryOp::Sub));
    assert_eq!(std::mem::discriminant(&BinaryOp::Mul), std::mem::discriminant(&BinaryOp::Mul));
    assert_eq!(std::mem::discriminant(&BinaryOp::Div), std::mem::discriminant(&BinaryOp::Div));
}

#[test]
fn test_compare_condition_enum() {
    assert_eq!(std::mem::discriminant(&CompareCondition::Eq), std::mem::discriminant(&CompareCondition::Eq));
    assert_eq!(std::mem::discriminant(&CompareCondition::Ne), std::mem::discriminant(&CompareCondition::Ne));
    assert_eq!(std::mem::discriminant(&CompareCondition::Lt), std::mem::discriminant(&CompareCondition::Lt));
    assert_eq!(std::mem::discriminant(&CompareCondition::Le), std::mem::discriminant(&CompareCondition::Le));
    assert_eq!(std::mem::discriminant(&CompareCondition::Gt), std::mem::discriminant(&CompareCondition::Gt));
    assert_eq!(std::mem::discriminant(&CompareCondition::Ge), std::mem::discriminant(&CompareCondition::Ge));
}

#[test]
fn test_instruction_enum() {
    // We'll test that we can create each variant and call the methods.
    // Since we haven't implemented the methods yet, we expect them to fail or return placeholders.
    // We'll just check that we can create the variants for now.
    let _ = Instruction::Constant { value: 42 };
    let _ = Instruction::Variable { name: "x".to_string() };
    let _ = Instruction::BinaryOp {
        op: BinaryOp::Add,
        lhs: Box::new(Instruction::Constant { value: 1 }),
        rhs: Box::new(Instruction::Constant { value: 2 }),
    };
    let _ = Instruction::Compare {
        condition: CompareCondition::Eq,
        lhs: Box::new(Instruction::Constant { value: 1 }),
        rhs: Box::new(Instruction::Constant { value: 2 }),
    };
    let _ = Instruction::Store {
        pointer: Box::new(Instruction::Variable { name: "ptr".to_string() }),
        value: Box::new(Instruction::Constant { value: 10 }),
    };
    let _ = Instruction::Branch {
        condition: Box::new(Instruction::Constant { value: 1 }),
        then_label: "then".to_string(),
        else_label: "else".to_string(),
    };
    let _ = Instruction::Jump { label: "target".to_string() };
    let _ = Instruction::Return {
        value: Some(Box::new(Instruction::Constant { value: 100 })),
    };
}

#[test]
fn test_instruction_methods() {
    // We'll test the methods once we implement them.
    // For now, we just call them to see if they compile.
    let instr = Instruction::Constant { value: 42 };
    let _ = instr.clone();
    let _ = instr.to_string();
    let _ = instr.is_terminator();
}