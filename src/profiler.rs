use std::collections::HashMap;
use std::time::{Instant, Duration};

#[derive(Debug, Default, Clone, PartialEq)]
pub struct BlockProfileData {
    pub execution_count: u64,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct FunctionProfileData {
    pub call_count: u64,
    pub total_time_us: f64, // Use f64 for microseconds as in C++
    pub peak_memory: usize, // Changed from memory_usage to peak_memory to match C++
    pub blocks: HashMap<String, BlockProfileData>,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct ProfileData {
    pub functions: HashMap<String, FunctionProfileData>,
}

#[derive(Clone)]
pub struct RuntimeProfiler {
    data: ProfileData,
    // C++ used std::map<std::string, size_t> currentMemory;
    // and std::map<std::string, std::chrono::high_resolution_clock::time_point> currentStart;
    // for tracking active functions.
    current_memory: HashMap<String, usize>, // For tracking variable count as proxy for memory
    current_start: HashMap<String, Instant>, // For tracking function execution time
}

impl RuntimeProfiler {
    pub fn new() -> Self {
        Self {
            data: ProfileData::default(),
            current_memory: HashMap::new(),
            current_start: HashMap::new(),
        }
    }

    pub fn start_function(&mut self, func_name: &str) {
        self.current_memory.insert(func_name.to_string(), 0);
        self.current_start.insert(func_name.to_string(), Instant::now());
        // Ensure entry exists
        self.data.functions.entry(func_name.to_string()).or_default();
    }

    pub fn end_function(&mut self, func_name: &str) {
        if let Some(start_time) = self.current_start.remove(func_name) {
            let duration = start_time.elapsed();
            let duration_us = duration.as_micros() as f64;

            let func_data = self.data.functions.entry(func_name.to_string()).or_default();
            func_data.total_time_us += duration_us;
            func_data.call_count += 1;

            if let Some(mem) = self.current_memory.remove(func_name) {
                if mem > func_data.peak_memory {
                    func_data.peak_memory = mem;
                }
            }
        }
    }

    // C++ had startBlock(funcName, blockName) and endBlock(funcName, blockName)
    // The C++ implementation only increments executionCount in endBlock.
    pub fn start_block(&mut self, func_name: &str, block_name: &str) {
        self.data.functions.entry(func_name.to_string()).or_default()
            .blocks.entry(block_name.to_string()).or_default();
    }

    pub fn end_block(&mut self, func_name: &str, block_name: &str) {
        if let Some(func_data) = self.data.functions.get_mut(func_name) {
            if let Some(block_data) = func_data.blocks.get_mut(block_name) {
                block_data.execution_count += 1;
            }
        }
    }

    // C++ had trackMemory(funcName, varCount)
    pub fn track_memory(&mut self, func_name: &str, var_count: usize) {
        if let Some(mem_count) = self.current_memory.get_mut(func_name) {
            *mem_count = var_count;
        }
    }

    pub fn get_data(&self) -> &ProfileData {
        &self.data
    }

    pub fn reset(&mut self) {
        self.data = ProfileData::default();
        self.current_memory.clear();
        self.current_start.clear();
    }

    pub fn get_hot_functions(&self, top_n: usize) -> Vec<String> {
        let mut funcs: Vec<(&String, &FunctionProfileData)> = self.data.functions.iter().collect();
        funcs.sort_by(|a, b| b.1.call_count.cmp(&a.1.call_count)); // Sort by call_count descending
        funcs.into_iter().take(top_n).map(|(name, _)| name.clone()).collect()
    }
}
