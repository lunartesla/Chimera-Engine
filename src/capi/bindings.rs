use crate::engine::OptimizationEngine;
use crate::passes::OptimizationLevel;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use log::error;

// Opaque struct for the C API to represent the Rust OptimizationEngine
#[repr(C)]
pub struct MetamorphicEngine {
    _data: [u8; 0], // This makes the struct zero-sized and opaque
}

// Helper to convert raw pointer to mutable Rust reference
fn from_raw_ptr<'a>(ptr: *mut MetamorphicEngine) -> Option<&'a mut OptimizationEngine> {
    if ptr.is_null() {
        return None;
    }
    unsafe {
        Some(&mut *(ptr as *mut OptimizationEngine))
    }
}

#[no_mangle]
pub extern "C" fn metamorphic_engine_create() -> *mut MetamorphicEngine {
    let engine = Box::new(OptimizationEngine::new(OptimizationLevel::Conservative)); // Default level
    Box::into_raw(engine) as *mut MetamorphicEngine
}

#[no_mangle]
pub extern "C" fn metamorphic_engine_destroy(engine: *mut MetamorphicEngine) {
    if engine.is_null() {
        return;
    }
    // Take ownership and drop the Box, freeing the memory
    unsafe {
        drop(Box::from_raw(engine as *mut OptimizationEngine));
    }
}

#[no_mangle]
pub extern "C" fn metamorphic_engine_set_level(engine: *mut MetamorphicEngine, level: i32) -> i32 {
    let Some(eng) = from_raw_ptr(engine) else {
        error!("metamorphic_engine_set_level: Null engine pointer.");
        return -1;
    };

    let opt_level = match level {
        0 => OptimizationLevel::Safe,
        1 => OptimizationLevel::Conservative,
        2 => OptimizationLevel::Balanced,
        _ => {
            error!("metamorphic_engine_set_level: Invalid optimization level {}. Must be 0, 1, or 2.", level);
            return -1;
        }
    };
    eng.set_optimization_level(opt_level);
    0
}

#[no_mangle]
pub extern "C" fn metamorphic_engine_optimize(engine: *mut MetamorphicEngine) -> i32 {
    let Some(eng) = from_raw_ptr(engine) else {
        error!("metamorphic_engine_optimize: Null engine pointer.");
        return -1;
    };

    match eng.optimize_hot_paths() {
        Ok(changed) => if changed { 1 } else { 0 },
        Err(e) => {
            error!("metamorphic_engine_optimize: Optimization failed: {:?}", e);
            -1
        }
    }
}

// Example of a function that might take a module as string
#[no_mangle]
pub extern "C" fn metamorphic_engine_load_module_json(engine: *mut MetamorphicEngine, json_str: *const c_char) -> i32 {
    let Some(eng) = from_raw_ptr(engine) else {
        error!("metamorphic_engine_load_module_json: Null engine pointer.");
        return -1;
    };

    if json_str.is_null() {
        error!("metamorphic_engine_load_module_json: Null JSON string.");
        return -1;
    }

    let c_str = unsafe { CStr::from_ptr(json_str) };
    let r_str = match c_str.to_str() {
        Ok(s) => s,
        Err(e) => {
            error!("metamorphic_engine_load_module_json: Invalid UTF-8 string: {}", e);
            return -1;
        }
    };

    // For now, only a dummy module can be loaded, as full deserialization is complex.
    // Replace with actual serde_json::from_str::<Module>(r_str) once Module is fully serializable.
    let module: crate::ir::module::Module = match serde_json::from_str(r_str) {
        Ok(m) => m,
        Err(e) => {
            error!("Failed to deserialize module from JSON: {}", e);
            // Create a dummy module for now, as C++ might do something similar
            // in case of parsing failures.
            let mut dummy_module = crate::ir::module::Module::new("dummy_module".to_string());
            dummy_module.functions.push(crate::ir::function::Function::new("main".to_string(), crate::ir::value::ValueType::Int));
            dummy_module
        }
    };

    eng.load_module(module);
    0
}

#[no_mangle]
pub extern "C" fn metamorphic_engine_get_optimized_module_json(engine: *mut MetamorphicEngine) -> *mut c_char {
    let Some(eng) = from_raw_ptr(engine) else {
        error!("metamorphic_engine_get_optimized_module_json: Null engine pointer.");
        return std::ptr::null_mut();
    };

    let Some(module) = eng.get_module() else {
        error!("metamorphic_engine_get_optimized_module_json: No module loaded.");
        return std::ptr::null_mut();
    };

    match serde_json::to_string(module) {
        Ok(json_string) => {
            // Convert to CString and leak the memory. Caller is responsible for freeing.
            CString::new(json_string).expect("CString::new failed").into_raw()
        },
        Err(e) => {
            error!("metamorphic_engine_get_optimized_module_json: Failed to serialize module to JSON: {}", e);
            std::ptr::null_mut()
        }
    }
}

// Function to free CString returned by get_optimized_module_json
#[no_mangle]
pub extern "C" fn metamorphic_engine_free_string(s: *mut c_char) {
    if s.is_null() {
        return;
    }
    unsafe {
        drop(CString::from_raw(s));
    }
}
