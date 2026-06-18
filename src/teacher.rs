use std::collections::{HashMap, VecDeque};
use std::env;
use log::{warn, info};
use serde::{Serialize, Deserialize};

use crate::neural_predictor::FunctionStats; // PassRecord removed

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MutationStep {
    pub mutation_type: String,
    pub pass_id: String,
    pub params: HashMap<String, i32>,
    #[serde(default)] // Default to 0.0 if not present in JSON
    pub fitness_after: f64,
    #[serde(default)] // Default to 0 if not present in JSON
    pub generation: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingLabel {
    #[serde(rename = "stateDescription")]
    pub state_description: String,
    #[serde(rename = "suggestedPassId")]
    pub suggested_pass_id: String,
    #[serde(rename = "expectedFitnessImprovement")]
    pub expected_fitness_improvement: f64,
}

#[derive(Clone)]
pub struct Teacher {
    api_key: String,
    model: String,
    base_url: String,
    #[allow(dead_code)] // C++ had confidenceHistory, but not directly used in the Rust port plan
    confidence_history: VecDeque<(f64, f64)>, // rolling 50
    last_call_failed: bool,
    client: reqwest::blocking::Client, // Using blocking client for simplicity, as per prompt
}

impl Teacher {
    pub fn new() -> Self {
        let api_key = env::var("OPENROUTER_API_KEY").unwrap_or_else(|_| {
            warn!("[Teacher] WARNING: OPENROUTER_API_KEY not set. Teacher will be unavailable.");
            // Return an empty string, as the prompt says to mark unavailable if missing
            String::new()
        });

        let mut teacher = Self {
            api_key,
            model: "nvidia/nemotron-3-super-120b-a12b:free".to_string(), // Default model
            base_url: "https://openrouter.ai/api/v1".to_string(),
            confidence_history: VecDeque::with_capacity(50),
            last_call_failed: false,
            client: reqwest::blocking::Client::new(),
        };

        if teacher.api_key.is_empty() {
            teacher.last_call_failed = true; // Mark as failed if API key is missing
        }

        teacher
    }

    pub fn is_available(&self) -> bool {
        !self.api_key.is_empty() && !self.last_call_failed
    }

    pub fn suggest_mutation(
        &mut self,
        ir: &str,
        goal: &str,
        current_pipeline: &[String],
        current_fitness: f64,
        best_fitness: f64,
        generations_stuck: i32,
    ) -> Option<MutationStep> {
        if !self.is_available() {
            return None;
        }

        let system_prompt = r#"You are an optimization advisor for a C++ metamorphic engine. Respond ONLY with valid JSON: {"mutation_type":"add|remove|tune|reorder|wild","pass_id":"constant_folding|dead_code|cse|loop_unroll|constant_propagation|block_merge|strength_reduction","params":{},"reasoning":"one sentence"}"#;

        #[derive(Serialize)]
        struct UserPromptData {
            ir: String,
            goal: String,
            current_pipeline: Vec<String>,
            current_fitness: f64,
            best_fitness: f64,
            generations_stuck: i32,
        }

        let user_prompt_data = UserPromptData {
            ir: ir.chars().take(500).collect(), // Truncate IR to 500 chars as in C++
            goal: goal.to_string(),
            current_pipeline: current_pipeline.to_vec(),
            current_fitness,
            best_fitness,
            generations_stuck,
        };

        let user_prompt = serde_json::to_string(&user_prompt_data).unwrap();

        let response = self.call_llm(system_prompt, &user_prompt);

        if response.is_empty() {
            self.last_call_failed = true;
            return None;
        }

        match serde_json::from_str::<MutationStep>(&response) {
            Ok(step) => {
                info!(
                    "[Teacher] {} {} — {}",
                    step.mutation_type,
                    step.pass_id,
                    step.params.iter().map(|(k,v)| format!("{}:{}", k, v)).collect::<Vec<String>>().join(", ")
                );
                self.last_call_failed = false;
                Some(step)
            }
            Err(e) => {
                warn!("[Teacher] Failed to parse LLM response: {} (Response: {})", e, response);
                self.last_call_failed = true;
                None
            }
        }
    }

    pub fn generate_training_labels(
        &mut self,
        goal_description: &str,
        records: &[String], // PassRecord removed — use String descriptions
    ) -> Vec<TrainingLabel> {
        if !self.is_available() || records.is_empty() {
            return Vec::new();
        }

        let system_prompt = r#"You are training a neural model for optimization prediction. Analyze these pass application records and return ONLY a JSON array: [{"stateDescription":"...","suggestedPassId":"...","expectedFitnessImprovement":0.0}]"#;

        // records is now Vec<String> — just pass them as raw strings
        let records_summary = records.iter().take(20)
            .enumerate()
            .map(|(i, r)| format!("{}: {}", i + 1, r))
            .collect::<Vec<_>>()
            .join(", ");

        let user_prompt = format!(
            "{{\"goal\": \"{}\", \"records\": [{}]}}",
            goal_description, records_summary
        );

        let response = self.call_llm(system_prompt, &user_prompt);

        if response.is_empty() {
            self.last_call_failed = true;
            return Vec::new();
        }

        match serde_json::from_str::<Vec<TrainingLabel>>(&response) {
            Ok(labels) => {
                self.last_call_failed = false;
                labels
            }
            Err(e) => {
                warn!("[Teacher] Failed to parse LLM response for training labels: {} (Response: {})", e, response);
                self.last_call_failed = true;
                Vec::new()
            }
        }
    }

    pub fn call_llm(&mut self, system_prompt: &str, user_prompt: &str) -> String {
        if self.api_key.is_empty() {
            return String::new();
        }

        #[derive(Serialize, Deserialize)]
        struct Message {
            role: String,
            content: String,
        }

        #[derive(Serialize)]
        struct RequestBody {
            model: String,
            max_tokens: u32,
            messages: Vec<Message>,
        }

        let body = RequestBody {
            model: self.model.clone(),
            max_tokens: 512,
            messages: vec![
                Message {
                    role: "system".to_string(),
                    content: system_prompt.to_string(),
                },
                Message {
                    role: "user".to_string(),
                    content: user_prompt.to_string(),
                },
            ],
        };

        let res = self.client.post(&format!("{}/chat/completions", self.base_url))
            .header(reqwest::header::AUTHORIZATION, format!("Bearer {}", self.api_key))
            .header("HTTP-Referer", "metamorphic-engine") // C++ had this header
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .json(&body)
            .send();

        match res {
            Ok(response) => {
                if response.status() == 429 {
                    warn!("[Teacher] Rate limited, retrying in 5s...");
                    std::thread::sleep(std::time::Duration::from_secs(5));
                    return self.call_llm(system_prompt, user_prompt); // Retry
                }
                if response.status().is_success() {
                    #[derive(Deserialize)]
                    struct Choice {
                        message: Message,
                    }
                    #[derive(Deserialize)]
                    struct LLMResponse {
                        choices: Vec<Choice>,
                    }

                    match response.json::<LLMResponse>() {
                        Ok(llm_response) => {
                            if let Some(choice) = llm_response.choices.into_iter().next() {
                                self.last_call_failed = false;
                                return choice.message.content;
                            }
                        }
                        Err(e) => {
                            warn!("[Teacher] Failed to parse LLM JSON response: {}", e);
                            self.last_call_failed = true;
                        }
                    }
                } else {
                    warn!("[Teacher] LLM API call failed with status: {}", response.status());
                    self.last_call_failed = true;
                }
            }
            Err(e) => {
                warn!("[Teacher] LLM HTTP request failed: {}", e);
                self.last_call_failed = true;
            }
        }
        String::new()
    }

    pub fn chat(&mut self, system: &str, user: &str) -> Option<String> {
        let result = self.call_llm(system, user);
        if result.is_empty() { None } else { Some(result) }
    }

    pub fn get_nm_confidence(&self) -> f64 {
        // Teacher doesn't own the NeuralPredictor — caller should query
        // persistent_predictor directly. This method exists for C++ API compat;
        // returning 0.0 here is safe since evolution_daemon.rs already reads
        // confidence from persistent_predictor.lock().unwrap().get_nm_confidence().
        0.0
    }
}