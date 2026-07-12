// Diagnostic: does Phase 2's memory model (AllocaArray/GetElementPtr/
// LoadPtr/StorePtr) actually execute correctly? Hand-built module, bypassing
// the parser entirely (which doesn't recognize array syntax yet) so this
// isolates just the new interpreter logic before the parser work.
//
// Equivalent to:
//   int arr[5]; arr[0..4] = {10,20,30,40,50};
//   int sum = 0, i = 0;
//   while (i < 5) { sum = sum + arr[i]; i = i + 1; }
//   return sum;  // expect 150
//
// Run with: cargo run --release --example diagnose_memory

use metamorphic_engine::ir::module::Module;
use metamorphic_engine::ir::function::Function;
use metamorphic_engine::ir::basic_block::BasicBlock;
use metamorphic_engine::ir::value::{Instruction, ValueType, BinaryOp, CompareCondition};
use metamorphic_engine::interpreter::Interpreter;

fn var(name: &str) -> Box<Instruction> { Box::new(Instruction::Variable { name: name.to_string() }) }
fn cst(v: i64) -> Box<Instruction> { Box::new(Instruction::Constant { value: v }) }

fn main() {
    let mut func = Function::new("array_sum".to_string(), ValueType::Int);

    let mut entry = BasicBlock::new("entry".to_string());
    entry.append(Instruction::AllocaArray { name: "arr".to_string(), count: 5 });
    // arr[k] = (k+1)*10 for k in 0..5
    for k in 0..5 {
        let ptr = Box::new(Instruction::GetElementPtr { base: var("arr"), index: cst(k as i64) });
        entry.append(Instruction::StorePtr { ptr, value: cst((k as i64 + 1) * 10) });
    }
    entry.append(Instruction::Store { var_name: "sum".to_string(), value: cst(0) });
    entry.append(Instruction::Store { var_name: "i".to_string(), value: cst(0) });
    entry.append(Instruction::Jump { label: "cond".to_string() });
    func.basic_blocks.push(entry);

    let mut cond = BasicBlock::new("cond".to_string());
    cond.append(Instruction::Branch {
        condition: Box::new(Instruction::Compare {
            condition: CompareCondition::Lt, lhs: var("i"), rhs: cst(5),
        }),
        then_label: "body".to_string(),
        else_label: "exit".to_string(),
    });
    func.basic_blocks.push(cond);

    let mut body = BasicBlock::new("body".to_string());
    let load_elem = Box::new(Instruction::LoadPtr {
        ptr: Box::new(Instruction::GetElementPtr { base: var("arr"), index: var("i") }),
    });
    body.append(Instruction::Store {
        var_name: "sum".to_string(),
        value: Box::new(Instruction::BinaryOp { op: BinaryOp::Add, lhs: var("sum"), rhs: load_elem }),
    });
    body.append(Instruction::Store {
        var_name: "i".to_string(),
        value: Box::new(Instruction::BinaryOp { op: BinaryOp::Add, lhs: var("i"), rhs: cst(1) }),
    });
    body.append(Instruction::Jump { label: "cond".to_string() });
    func.basic_blocks.push(body);

    let mut exit = BasicBlock::new("exit".to_string());
    exit.append(Instruction::Return { value: Some(var("sum")) });
    func.basic_blocks.push(exit);

    let mut module = Module::new("memtest".to_string());
    module.functions.push(func);

    let interpreter = Interpreter::new();
    let result = interpreter.execute_function(&module, &module.functions[0], &[], None);
    println!("Result: {:?}", result);
    println!("Expected: Ok(150)  (10+20+30+40+50)");

    // Also run it through the real validator-style pipeline check (no-op
    // pipeline) to make sure OptimizationEngine's machinery handles the new
    // instruction kinds without choking, same shape as earlier diagnostics.
    use metamorphic_engine::engine::OptimizationEngine;
    use metamorphic_engine::passes::OptimizationLevel;
    let mut eng = OptimizationEngine::new(OptimizationLevel::Conservative);
    eng.load_module(module.clone());
    let validation = eng.validate_optimization_result("array_sum");
    println!("No-op pipeline validation: passed={} details={}", validation.passed, validation.failure_details);
}
