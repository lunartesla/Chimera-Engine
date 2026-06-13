use std::env;
use std::fs;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use std::sync::Mutex;
use log::{info, warn, error, LevelFilter};
use env_logger::Builder;
use anyhow::Result;
use serde::Serialize;

use metamorphic_engine::ir::module::Module;
use metamorphic_engine::ir::function::Function;
use metamorphic_engine::ir::value::ValueType;
use metamorphic_engine::engine::OptimizationEngine;
use metamorphic_engine::passes::OptimizationLevel;
use metamorphic_engine::interpreter::Interpreter;
use metamorphic_engine::profiler::RuntimeProfiler;
use metamorphic_engine::self_evolving_engine::SelfEvolvingEngine;
use metamorphic_engine::goal_definition::GoalDefinition;
use metamorphic_engine::module_builders;
use metamorphic_engine::evolution_daemon::EvolutionDaemon;
use metamorphic_engine::teacher::Teacher;
use metamorphic_engine::ir_generator::IRGenerator;

// Constants from C++ main.cpp or defaults
const OPENROUTER_API_KEY_ENV: &str = "OPENROUTER_API_KEY";

fn init_logger() {
    Builder::new()
        .filter_level(LevelFilter::Info)
        .filter_module("neuralneat", LevelFilter::Warn) // silence NEAT genome spam
        .parse_default_env()
        .init();
}

#[tokio::main] // Use tokio for async operations like EngineServer
async fn main() -> Result<()> {
    init_logger(); // Initialize logging

    // BlackWall: set HIGH process priority for maximum throughput
    // and use all available CPU cores.
    // This is OS-specific and requires platform-specific Rust crates or FFI.
    // For now, log a message.
    info!("[BlackWall] Process priority: HIGH | CPU cores: (Rust equivalent not directly implemented yet)");


    let args: Vec<String> = env::args().collect();
    let mut demo_mode = false;
    let mut daemon_mode = false;
    let mut generate_mode = false;
    let mut target_mode = false;
    let mut wildcard_mode = false;
    let mut goal_name = "minimize_instrs".to_string();
    let mut target_file = String::new();

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--demo" => demo_mode = true,
            "--daemon" => daemon_mode = true,
            "--generate" => generate_mode = true,
            "--wild" => wildcard_mode = true,
            "--goal" => {
                i += 1;
                if i < args.len() {
                    goal_name = args[i].clone();
                }
            }
            "--target" => {
                i += 1;
                if i < args.len() {
                    target_mode = true;
                    target_file = args[i].clone();
                }
            }
            "--help" => {
                print_help();
                return Ok(());
            }
            _ => {}
        }
        i += 1;
    }

    // Default to demo if no mode specified
    if !demo_mode && !daemon_mode && !generate_mode && !target_mode {
        demo_mode = true;
    }

    if daemon_mode {
        run_daemon(wildcard_mode, &goal_name).await?;
    } else if generate_mode {
        run_generate_mode().await?;
    } else if target_mode {
        run_target_mode(&target_file).await?;
    } else {
        run_demo();
    }

    Ok(())
}

fn print_help() {
    println!("Metamorphic Engine");
    println!("Usage: metamorphic [options]");
    println!("\nOptions:");
    println!("  --demo            Run demo mode (default)");
    println!("  --daemon          Run in daemon mode (continuous evolution)");
    println!("  --generate        Generate new module via h3 (empty slate mode)");
    println!("  --wild            Enable wildcard IR mutation mode");
    println!("  --goal <name>     Set goal for daemon mode");
    println!("                     Options: minimize_instrs, minimize_time,");
    println!("                             token_comm, max_branch_elim");
    println!("  --help            Show this help message");
    println!("\nEnvironment Variables:");
    println!("  OPENROUTER_API_KEY  API key for Teacher (LLM integration)");
}

fn run_demo() {
    info!("Metamorphic Engine - Demo Mode");
    info!("===================================\n");

    let module = module_builders::build_sum_example(10);

    info!("=== Original IR ===\n{:?}", module);

    let mut engine = OptimizationEngine::new(OptimizationLevel::Conservative);
    engine.load_module(module.clone());
    engine.profile("compute_sum").expect("Failed to profile function");
    // C++ printed report directly
    engine.identify_hot_paths(5);

    info!("\n=== Pass Registry ===");
    for id in engine.get_registry().list_all() {
        if let Some(desc) = engine.get_registry().get_descriptor(&id) {
            info!("  [{:?}] {} ({})", desc.safety, desc.name, desc.id);
        }
    }

    // C++ prints pipeline info. PassManager needs method for this.
    // info!("\nPipeline: {}", engine.get_pass_manager().get_pipeline_info());

    info!("\n--- Optimization ---");
    let changed = engine.optimize_hot_paths().expect("Failed to optimize hot paths");
    info!("Optimization {}", if changed { "made changes" } else { "no changes" });

    if changed {
        info!("\n=== Optimized IR ===\n{:?}", engine.get_module().unwrap());
    }

    info!("\n--- Validation ---");
    engine.validate_optimization("compute_sum");

    info!("\n=== Functional Testing ===");
    for n in [5, 10, 20] {
        let test_mod = module_builders::build_sum_example(n);

        let mut p1 = RuntimeProfiler::new();
        let i1 = Interpreter::new();
        let orig = i1.execute_function(&test_mod.functions[0], Some(&mut p1)).expect("Original execution failed");

        let mut test_eng = OptimizationEngine::new(OptimizationLevel::Conservative);
        test_eng.load_module(test_mod.clone());
        test_eng.profile("compute_sum").expect("Failed to profile for test_eng");
        test_eng.identify_hot_paths(1);
        test_eng.optimize_hot_paths().expect("Failed to optimize for test_eng");

        let mut p2 = RuntimeProfiler::new();
        let i2 = Interpreter::new();
        let opt = i2.execute_function(&test_eng.get_module().unwrap().functions[0], Some(&mut p2)).expect("Optimized execution failed");

        let expected = n * (n - 1) / 2;
        info!("n={}: expected={} orig={} opt={} {}", n, expected, orig, opt, if orig == expected as i64 && opt == expected as i64 { "OK" } else { "FAIL" });
    }

    info!("\n=== IR Generation ===");
    {
        let orig = module.functions[0].clone();
        let mut consts = HashMap::new();
        consts.insert("n".to_string(), 5);
        let spec = IRGenerator::specialize_function(&orig, &consts);
        info!("Specialized (n=5):\n{:?}", spec);

        let mut known_values = HashMap::new();
        known_values.insert("i".to_string(), 0);
        known_values.insert("sum".to_string(), 0);
        known_values.insert("n".to_string(), 10);
        let variants = IRGenerator::generate_variants(&orig, &known_values, 4);
        info!("Generated {} variants", variants.len());
    }

    info!("\n--- Self-Evolving Engine ---");
    {
        let mut se = SelfEvolvingEngine::new(engine, OptimizationLevel::Conservative, "demo_se");
        // C++ printed registryInfo, which is PassRegistry.
        // info!("{}", se.registry_info());
        se.evolve(5, false);
    }

    info!("\n=== Demo Complete ===");
}

async fn run_daemon(wildcard: bool, goal_name: &str) -> Result<()> {
    println!("╔══════════════════════════════════════════════════╗");
    println!("║   Metamorphic Engine - Evolution Daemon          ║");
    if wildcard {
        println!("║   WILDCARD IR MUTATION MODE ENABLED              ║");
    }
    println!("╚══════════════════════════════════════════════════╝\n");

    let api_key = env::var(OPENROUTER_API_KEY_ENV).ok();
    let teacher = api_key.map(|_key| Teacher::new()); // Teacher needs to be constructed this way
    let teacher = Mutex::new(teacher);

    let modules = vec![
        module_builders::build_simple_sum(20),
        module_builders::build_fib_loop(15),
        module_builders::build_nested_loop(8),
        module_builders::build_branch_heavy(16),
        module_builders::build_entropy_loop(24),
    ];
    info!("Synthetic modules: {}", modules.len());

    let uro_mods = module_builders::load_uroboros_library();
    let mut all_modules = modules;
    all_modules.extend(uro_mods);
    info!("Total modules: {}", all_modules.len());

    let daemon_teacher = teacher.into_inner().ok().flatten();

    let mut daemon = EvolutionDaemon::new(
        all_modules,
        "daemon_best_pipelines.json",
        wildcard,
        daemon_teacher
    );

    let goal = match goal_name {
        "minimize_instrs" => GoalDefinition::minimize_instructions(8),
        "minimize_time" => GoalDefinition::minimize_time(10.0),
        "token_comm" => GoalDefinition::token_communication(),
        "max_branch_elim" => GoalDefinition::maximize_branch_elimination(),
        _ => GoalDefinition::minimize_instructions(8), // Default
    };
    daemon.set_goal(goal);

    daemon.run(); // This is blocking in C++, so keep it blocking in Rust for now

    // The C++ main.cpp handled TCP server start/stop explicitly around daemon.run().
    // The Rust daemon manages its own server.

    println!("\nFinal stats:");
    println!("  Total generations: {}", daemon.total_generations());
    for module in daemon.get_library().iter() {
        let fit = daemon.best_fitness(&module.name);
        if fit > -1e8 {
            println!("  {}: {}", module.name, fit);
        }
    }

    Ok(())
}

async fn run_generate_mode() -> Result<()> {
    info!("Metamorphic Engine - Empty Slate Mode");
    info!("==========================================\n");

    let _api_key = env::var(OPENROUTER_API_KEY_ENV).ok();
    let mut teacher = Teacher::new(); // Teacher needs to be constructed this way

    if !teacher.is_available() {
        error!("Teacher not available for generate mode. Set OPENROUTER_API_KEY.");
        return Ok(());
    }

    info!("[Generate] Asking h3 to suggest a module description...");
    let goal_desc = "minimize instruction count for a computational task";

    let system_prompt = "You are h3, an AI assistant for the Metamorphic Engine. \
                         Generate a clear, concise description of a computational module that \
                         would be useful for IR optimization testing. The module should be a \
                         self-contained function that performs a meaningful computation. \
                         Describe what it computes, any loops, and key operations. \
                         Keep the description to 2-3 sentences.";

    let user_prompt = format!("Please generate a module description for goal: {}", goal_desc);

    let response = teacher.chat(system_prompt, &user_prompt);

    let module_description = match response {
        Some(r) if !r.is_empty() => r,
        _ => {
            warn!("[Generate] Failed to get response from h3. Using default.");
            "A function that computes the sum of squares from 1 to N using a loop".to_string()
        }
    };

    info!("[h3] Module description: {}", module_description);

    info!("[Generate] Translating description to IR...");

    let mut new_module = if module_description.contains("sum") && module_description.contains("squares") {
        // Build sum of squares
        // Placeholder for a real sum of squares builder, using nested_loop for now
        module_builders::build_nested_loop(10)
    } else {
        // Default: simple sum loop
        module_builders::build_simple_sum(20)
    };
    new_module.name = "gen_default".to_string(); // Name should reflect generation, not builder

    info!("[Generate] Generated module:\n{:?}", new_module);

    // Save to blueprints/generated/
    let generated_dir = PathBuf::from("blueprints/generated");
    fs::create_dir_all(&generated_dir)?;
    let filename = generated_dir.join(format!("{}.json", new_module.name));

    #[derive(Serialize)]
    struct GeneratedModuleMeta {
        module_name: String,
        description: String,
        timestamp: u64,
        functions_count: usize,
    }

    let meta = GeneratedModuleMeta {
        module_name: new_module.name.clone(),
        description: module_description,
        timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
        functions_count: new_module.functions.len(),
    };

    if let Ok(json_string) = serde_json::to_string_pretty(&meta) {
        fs::write(&filename, json_string)?;
        info!("[Generate] Saved to {}", filename.display());
    }

    info!("\n[Generate] Starting evolution of generated module...");
    let modules = vec![new_module];

    let mut daemon = EvolutionDaemon::new(
        modules,
        "generated_best_pipelines.json",
        false, // No wildcard for generated
        Some(teacher),
    );
    daemon.run();

    Ok(())
}

async fn run_target_mode(target_file: &str) -> Result<()> {
    info!("Metamorphic Engine - Target Mode");
    info!("==========================================\n");

    let target_path = PathBuf::from(target_file);
    if !target_path.exists() {
        error!("[Target] File not found: {}", target_path.display());
        return Ok(());
    }

    info!("[Target] Loading target: {}", target_path.display());
    let mut target_module: Option<Module> = None;

    let extension = target_path.extension().and_then(|s: &std::ffi::OsStr| s.to_str()).unwrap_or("");

    if extension == "json" {
        info!("[Target] Loading IR from JSON...");
        if let Ok(_content) = fs::read_to_string(&target_path) {
            // Simplified: create a module from JSON
            // C++ also created a simplified module, full deserialization isn't done yet.
            let mod_name = target_path.file_stem().and_then(|s: &std::ffi::OsStr| s.to_str()).unwrap_or("target").to_string();
            let mut module = Module::new(mod_name);
            module.functions.push(Function::new("main".to_string(), ValueType::Int));
            target_module = Some(module);
            info!("[Target] Loaded JSON module: {}", target_module.as_ref().unwrap().name);
        } else {
            error!("[Target] Failed to read JSON file");
        }
    } else if extension == "py" || extension == "cpp" {
        info!("[Target] Sending {} file to h3 for IR translation...", extension);

        let content = fs::read_to_string(&target_path)?;

        let mut teacher = Teacher::new();
        if !teacher.is_available() {
            error!("[Target] Teacher not available. Cannot translate. Set OPENROUTER_API_KEY.");
            return Ok(());
        }

        let system_prompt = "You are h3, an AI assistant for the Metamorphic Engine. \
                             Translate the given code to IR (Intermediate Representation) description. \
                             Describe the functions, basic blocks, and instructions needed.";

        let response = teacher.chat(system_prompt, &format!("Translate this {} code to IR:\n{}", extension, content));

        info!("[h3] Translation:\n{}", response.as_deref().unwrap_or("(no response)"));

        // For now, create a simple module
        let mut module = module_builders::build_simple_sum(20);
        module.name = format!("target_{}", target_path.file_stem().and_then(|s: &std::ffi::OsStr| s.to_str()).unwrap_or("unknown"));
        target_module = Some(module);
    } else {
        error!("[Target] Unsupported file type: {}", extension);
        return Ok(());
    }

    let Some(modules) = target_module.map(|m| vec![m]) else {
        error!("[Target] Failed to load target module");
        return Ok(());
    };

    info!("\n[Target] Starting evolution of target module...");

    let teacher = Teacher::new();

    let mut daemon = EvolutionDaemon::new(
        modules,
        "target_best_pipelines.json",
        false, // No wildcard for target mode
        Some(teacher),
    );
    daemon.run();

    Ok(())
}
