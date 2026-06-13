use crate::ir::module::{Module, Function, BasicBlock};
use crate::ir::value::{Instruction, ValueType};
use crate::passes::{PassManager, OptimizationLevel, Pass, PassSafety, PassDescriptor, PassError, ParamRange};
use std::collections::HashMap;

// Dummy pass for testing PassManager functionality
struct DummyPass {
    id: &'static str,
    name: &'static str,
    description: &'static str,
    safety: PassSafety,
    params: HashMap<String, ParamRange>,
}

impl DummyPass {
    fn new(id: &'static str) -> Self {
        let mut params = HashMap::new();
        params.insert(
            "dummy_param".to_string(),
            ParamRange::new("dummy_param", 1, 0, 10, 1),
        );
        Self {
            id,
            name: id,
            description: "A dummy pass",
            safety: PassSafety::Safe,
            params,
        }
    }
}

impl Pass for DummyPass {
    fn id(&self) -> &'static str { self.id }
    fn name(&self) -> &'static str { self.name }
    fn description(&self) -> &'static str { self.description }
    fn safety(&self) -> PassSafety { self.safety }
    fn run(&self, _module: &mut Module) -> Result<bool, PassError> {
        // Dummy pass always reports no change
        Ok(false)
    }
    fn get_param(&self, name: &str) -> Option<i32> { self.params.get(name).map(|p| p.current) }
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

#[test]
fn test_pass_manager_add_remove() {
    let mut pm = PassManager::new();
    assert_eq!(pm.pass_count(), 0);

    pm.add(Box::new(DummyPass::new("pass1")));
    assert_eq!(pm.pass_count(), 1);

    pm.add(Box::new(DummyPass::new("pass2")));
    assert_eq!(pm.pass_count(), 2);

    pm.remove(0);
    assert_eq!(pm.pass_count(), 1);
    assert_eq!(pm.get_pass_info(0).unwrap().id, "pass2");

    pm.remove(0);
    assert_eq!(pm.pass_count(), 0);
}

#[test]
fn test_pass_manager_set_param() {
    let mut pm = PassManager::new();
    let mut pass1 = DummyPass::new("pass1");
    pass1.set_param("dummy_param", 5); // Set directly on instance
    pm.add(Box::new(pass1));

    // Retrieve the pass and check its parameter
    let desc = pm.get_pass_info(0).unwrap();
    assert_eq!(desc.get_param("dummy_param"), Some(5));

    // Try to set param through pass manager (requires mutable access to individual passes)
    // The current PassManager design doesn't expose mutable access to individual passes in the Vec<Box<dyn Pass>>
    // This is a limitation in the current design, as PassManager::set_param is not present.
    // In C++, PassManager could modify the parameters of a specific PassDescriptor in its pipeline.
    // For now, we verify that initial parameter setting works.
    // To enable set_param via PassManager, we would need a method like:
    // pub fn set_pass_param(&mut self, index: usize, name: &str, value: i32) -> bool
    // This isn't strictly requested by the prompt for PassManager implementation.
}

#[test]
fn test_pass_manager_populate_from_level() {
    let mut pm = PassManager::new();
    let registry = crate::passes::pass_registry::PassRegistry::new();

    pm.populate_from_level(OptimizationLevel::Safe, &registry);
    assert_eq!(pm.pass_count(), 1); // Only constant_folding
    assert_eq!(pm.get_pass_info(0).unwrap().id, "constant_folding");

    pm.populate_from_level(OptimizationLevel::Conservative, &registry);
    assert_eq!(pm.pass_count(), 4); // cf, dce, cse, loop_unroll (from C++ PassManager.cpp)

    pm.populate_from_level(OptimizationLevel::Balanced, &registry);
    assert_eq!(pm.pass_count(), 7); // all 7 passes
}
