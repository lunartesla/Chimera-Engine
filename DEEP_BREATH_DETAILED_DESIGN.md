# Deep Breath Phase 3: Detailed Systems Design
## Metamorphic Engine - Implementation-Ready Architecture

**Date:** 2026-07-12  
**Phase:** Detailed Design  
**Purpose:** Architecture specifications ready for implementation

---

## EXECUTIVE SUMMARY

## DETAILED SYSTEM DESIGNS

### 1. EXPERIENCE DATABASE SYSTEM

#### Purpose
Accumulate all optimization outcomes for replay, curriculum learning, and knowledge discovery.

#### Module Layout
```
src/experience/
├── mod.rs           (public API)
├── database.rs      (persistence layer)
├── schema.rs        (data structures)
├── replay.rs        (experience replay)
├── curriculum.rs    (progressive difficulty)
└── storage.rs       (tiered storage)
```

#### Public Interface
```rust
/// Trait for experience record operations
pub trait Experience: Send + Sync {
    fn observe(&mut self, record: ExperienceRecord) -> Result<()>;
    fn replay_for(&self, query: &ReplayQuery) -> Vec<ExperienceRecord>;
    fn current_curriculum_stage(&self) -> CurriculumStage;
    fn checkpoint(&self, path: &Path) -> Result<()>;
}
```

#### Data Structures
```rust
/// Full experience record with all metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperienceRecord {
    // Problem identification
    pub module_shape_hash: String,
    pub goal_id: String,
    pub function_stats: FunctionStats,
    
    // Solution applied
    pub pipeline_id: String,          // UUID for pipeline
    pub passes: Vec<PassDescriptor>,  // Full pipeline applied
    
    // Outcome metrics
    pub fitness: f64,
    pub baseline_instructions: usize,
    pub final_instructions: usize,
    pub instruction_reduction: usize,
    pub validation_passed: bool,
    pub execution_time_us: f64,
    pub memory_peak_bytes: usize,
    
    // Context
    pub generation: u64,
    pub island_id: Option<usize>,
    pub strain_id: Option<String>,
    pub temperature: f64,
    pub seed_fitness: f64,
    
    // Metadata
    pub timestamp: u64,
    pub run_id: String,
    pub success: bool,
    pub correction_needed: bool,  // Did we need to revert?
}

/// Query for experience replay
pub struct ReplayQuery {
    pub module_shape_hash: Option<String>,
    pub goal_id: Option<String>,
    pub min_fitness: f64,
    pub max_instructions: Option<usize>,
    pub limit: usize,
    pub time_range: Option<(u64, u64)>,
}
```

#### Persistence Strategy
- **Hot tier:** In-memory HashMap keyed by (shape_hash, goal_id)
- **Warm tier:** SQLite database with indexes on shape_hash, goal_id, fitness
- **Cold tier:** S3-compatible object storage for archived experiences

#### Configuration
```rust
pub struct ExperienceConfig {
    pub hot_capacity: usize,           // Default: 100,000
    pub warm_path: PathBuf,            // Default: "experiences.db"
    pub curriculum_enabled: bool,      // Default: true
    pub retain_failed: bool,          // Default: true
    pub retention_days: u32,           // Default: 365
}
```

#### Concurrency Model
- **Write:** Channel-based batching to avoid lock contention
- **Read:** Read-write lock on hot cache, connection pooling for warm
- **Checkpoint:** Async snapshot generation

---

### 2. NEURAL SYSTEM EVOLUTION

#### Purpose
Enable multi-objective prediction and experience-informed evolution.

#### Public Interface Evolution
```rust
// Current: predict() returns Option<Prediction>
// New: predict_with_uncertainty() returns DistributionPrediction

pub struct DistributionPrediction {
    pub mean: Prediction,
    pub variance: [f32; OUTPUT_NODES],
    pub confidence_interval: (f32, f32),
    pub model_version: u64,
}

trait AdaptivePredictor: Send + Sync {
    fn predict(&self, features: &[f32]) -> Result<DistributionPrediction>;
    fn update(&mut self, experience: &ExperienceRecord) -> UpdateResult;
    fn ensemble_predict(&self, experiences: &[ExperienceRecord]) -> DistributionPrediction;
}
```

#### Training Pipeline Design
```rust
pub struct TrainingPipeline {
    pub feature_extractor: FeatureExtractor,
    pub validator: ValidationPolicy,
    pub loss_computer: MultiHeadLoss,
    pub optimizer: NEATOptimizer,
}

// Multi-objective loss combining:
// 1. Instruction reduction accuracy
// 2. Correctness prediction
// 3. Execution time prediction
// 4. Stability (low variance predictions)
```

#### Alternative Approaches

**A. Ensemble NEAT (Recommended)**
- 3 independent NEAT populations
- Voting for final prediction
- Disagreement = exploration signal
- **Pros:** Robust, fault-tolerant, uncertainty quantification
- **Cons:** 3x compute, more complex management

**B. Bayesian Neural Networks**
- Dropout at inference for uncertainty
- **Pros:** Native uncertainty, single model
- **Cons:** Different crate, more experimental

**C. Transformer Attention (Future)**
- Self-attention on pass sequences
- **Pros:** Long-range dependencies, powerful
- **Cons:** High compute, needs more data

---

### 3. GENETIC PROGRAMMING ENHANCEMENTS

#### Genome Representation Extension
```rust
// Current: Vec<PassDescriptor>
// Proposed: Structured genome with metadata

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvolvedPipeline {
    pub id: Uuid,
    pub passes: Vec<PassInstance>,
    pub provenance: Provenance,
    pub validation_result: Option<ValidationResult>,
}

#[derive(Debug, Clone)]
pub struct PassInstance {
    pub descriptor: PassDescriptor,
    pub expected_effect: PassEffectPrediction,
    pub dependencies: Vec<PassId>,  // Ordering constraints
    pub confidence: f32,
}

// Dependency-aware ordering
pub enum PassEffectPrediction {
    ReducesInstructions { expected: usize, variance: f32 },
    EliminatesBranches { expected: usize },
    ImprovesCacheLocality { score: f32 },
    Unknown,
}
```

#### Mutation Operator Design
```rust
pub trait MutationOperator: Send + Sync {
    fn mutate(&self, genome: &mut EvolvedPipeline, context: &MutationContext) -> MutationResult;
    fn applicable(&self, genome: &EvolvedPipeline) -> bool;
}

pub struct MutationContext {
    pub temperature: f32,
    pub generation: u64,
    pub previous_fitness: f64,
    pub stagnation_count: u32,
    pub experience_replay: Option<Vec<ExperienceRecord>>,
}

// Implemented operators:
pub struct AdaptiveMutator {
    pub operators: Vec<Box<dyn MutationOperator>>,
    pub rates: AdaptiveRates,  // Updates based on success history
}
```

---

### 4. VALIDATION SYSTEM ENHANCEMENTS

#### Current Limitation
Validation is binary: passed/failed. No granularity for partial correctness or witness generation.

#### Proposed Enhancement
```rust
pub struct DetailedValidation {
    pub passed: bool,
    pub counterexamples: Vec<Counterexample>,
    pub invariants_checked: Vec<InvariantResult>,
    pub partial_correctness: CorrectnessMap,  // Per-function/block status
}

pub struct Counterexample {
    pub inputs: Vec<i64>,
    pub expected: i64,
    pub actual: i64,
    pub divergence_point: String,  // Which instruction caused difference
}

// Synthesize minimal failing case for faster debugging
pub fn synthesize_minimal_counterexample(
    original: &Module,
    modified: &Module,
) -> Option<Counterexample> {
    // Binary search through module to find fault location
    // Generate minimal test case
}
```

---

### 5. CURRICULUM ENGINE

#### Purpose
Progressively increase optimization difficulty to accelerate learning.

#### Design
```rust
pub enum CurriculumStage {
    Novice,        // Simple loops, 10-50 instructions
    Intermediate,  // Nested structures, 50-200 instructions
    Advanced,      // Branches, calls, 200-1000 instructions
    Expert,        // Real programs, 1000+ instructions
}

pub struct CurriculumEngine {
    pub current_stage: CurriculumStage,
    pub success_history: CircularBuffer<SuccessMetric>,
    pub stage_gate: StageGate,
}

pub struct SuccessMetric {
    pub fitness_improvement: f64,
    pub validation_pass_rate: f64,
    pub timestamp: u64,
}

impl CurriculumEngine {
    pub fn should_advance(&self) -> bool {
        // Require 80% validation pass rate and 10% avg improvement
        self.success_history.avg_improvement() > 0.10
        && self.success_history.avg_pass_rate() > 0.80
    }
}
```

---

## MODULE ARCHITECTURE

### New Module Dependencies
```
lib.rs
├── experience/          [NEW - Tiered persistence]
├── predictor/           [NEW - Advanced prediction]
├── curriculum/          [NEW - Progressive difficulty]
├── validator/           [ENHANCE - Counterexamples]
├── knowledge/           [NEW - Pattern mining]
└── passes/              [ENHANCE - Dependencies]
```

### Integration Points
1. **EvolutionDaemon** → Experience::observe every outcome
2. **NeuralPredictor** → Experience::replay_for training context
3. **SelfEvolvingEngine** → CurriculumEngine for stage-aware mutation
4. **Validator** → Knowledge to prune unsafe patterns

---

## CONCURRENCY MODEL

### Experience Database
- **Hot cache:** RwLock<HashMap<...>> with async background flush
- **Write batching:** Channel<ExperienceRecord> → batch writer
- **Read concurrency:** Multiple readers, single batch writer

### Predictor Updates
- **Lock-free reads:** Atomic swap on model updates
- **Training isolation:** Dedicated thread pool
- **Ensemble voting:** Parallel evaluation of member models

### Curriculum Advancement
- **Gate evaluation:** Single-threaded in daemon loop
- **History update:** Lock-free ring buffer

---

## PERSISTENCE DESIGN

### Database Schema (SQLite)
```sql
CREATE TABLE experiences (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    module_shape_hash TEXT,
    goal_id TEXT,
    pipeline_json TEXT,
    fitness REAL,
    validation_passed INTEGER,
    timestamp INTEGER,
    success INTEGER,
    INDEX idx_shape_goal (module_shape_hash, goal_id),
    INDEX idx_fitness (fitness DESC)
);

CREATE TABLE model_checkpoints (
    version INTEGER PRIMARY KEY,
    model_json TEXT,
    training_stats_json TEXT,
    timestamp INTEGER
);
```

### Checkpoint Strategy
- **Frequency:** Every 1000 new experiences or 1 hour
- **Format:** JSON snapshot of all model weights + stats
- **Retention:** Last 10 checkpoints + best-performing overall

---

## VALIDATION STRATEGY

### Unit Tests
- Experience serialization/deserialization
- Curriculum stage transitions
- Predictor distribution output shapes

### Integration Tests
- Full pipeline evolution with experience replay
- Multi-process experience sharing
- Crash recovery from checkpoints

### Property Tests
- Experience database maintains consistency under concurrent writes
- Predictor distributions have valid confidence intervals
- Curriculum stages are monotonic

### Regression Tests
- All existing evolution tests still pass
- Performance doesn't regress >10%

---

## PERFORMANCE ANALYSIS

### Memory Consumption Estimates
| Component | Hot Size | Warm Size | Notes |
|-----------|----------|-----------|-------|
| Experience | 100K records × 1KB = 100MB | 1M records × 1KB = 1GB | Prunable |
| Predictor | 450 genomes × 10KB = 4.5MB | Per checkpoint | Checkpointable |
| Curriculum | 100 records × 100B = 10KB | Negligible | In-memory |

### CPU Usage
- **Experience ingestion:** <1% (batched writes)
- **Replay queries:** <5% (indexed lookups)
- **Curriculum evaluation:** <1% (simple averages)
- **Ensembling:** +200% predictor compute (offset by better decisions)

---

## ALTERNATIVE DESIGNS

### Experience Backend Alternatives

**A. Rust-ADCursors/LMDB (Recommended for embedded)**
- Embedded, no external process
- High performance key-value
- **Trade-off:** No SQL queries

**B. PostgreSQL (For distributed)**
- Full SQL, concurrent access
- Network overhead
- **Trade-off:** External dependency

**C. Sled (Pure Rust)**
- No external deps, good performance
- Tree-based, not SQL-like
- **Trade-off:** Less familiar query model

### Recommendation: Start with SQLite
- Single file, no config
- Rich querying for analysis
- Easy to migrate later

---

## IMPLEMENTATION ORDER

### Phase 3A (Weeks 1-3): Foundation
1. Experience record schema and serialization
2. SQLite persistence layer
3. Basic replay query support
4. Integration with EvolutionDaemon::run

### Phase 3B (Weeks 4-6): Curriculum
5. Curriculum stage detection
6. Success rate tracking
7. Stage transition logic
8. Integration with SelfEvolvingEngine

### Phase 3C (Weeks 7-9): Advanced Prediction
9. DistributionPrediction struct
10. Ensemble predictor skeleton
11. Confidence interval computation
12. Integration with mutation decisions

### Phase 3D (Weeks 10-12): Knowledge
13. Successful pattern miner
14. Counterexample synthesizer
15. Knowledge database
16. Integration with strain system

---

## RECOMMENDED PHASE 4 OBJECTIVES

1. **Distributed Evolution Protocol** - Multiple daemons share experiences
2. **GPU-Accelerated Fitness** - Population evaluation on GPU
3. **Meta-Evolution Framework** - Evolves mutation operator weights
4. **Production Benchmark Suite** - Real-world program optimization
5. **Safety Witness Generation** - Formal proof templates for optimizations

---

## RISK ANALYSIS

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| SQLite lock contention | Medium | Medium | Batch writes, background flush |
| Curriculum premature advance | Medium | High | Conservative gating, rollback |
| Ensemble overfitting | Low | High | Cross-validation, pruning |
| Memory exhaustion | Low | Critical | Tiered retention, monitoring |

---

## DATA FLOW DIAGRAMS

### Experience Flow
```
[Pipeline Execution] 
    → OutcomeRecord 
    → ExperienceDatabase::observe 
    → Hot cache + Batch to disk
    → Curriculum::record_success 
    → [Advance stage?]
    → Predictor::update_with_experience 
    → [Train model if ready]
```

### Prediction Flow
```
Module + Stats → FeatureExtractor 
    → EnsemblePredictor::predict 
    → DistributionPrediction 
    → MutationOperator::mutate 
    → NewPipeline
    → Validation 
    → ExperienceRecord
```

---

## DEPENDENCY GRAPH

```
experience_database
    ↑
curriculum_engine → evolution_daemon
    ↑                   ↓
predictor_ensemble ← self_evolving_engine
    ↑                   ↓
knowledge_miner → strain_system
```

---

## CONCLUSION

The designs presented enable:
1. **Measurable experience accumulation** - Track every optimization
2. **Confidence-aware decisions** - Ensemble provides uncertainty
3. **Progressive difficulty** - Curriculum guides learning
4. **Safety-first evolution** - Counterexamples prevent repeated mistakes
5. **Scalable persistence** - Tiered storage handles any volume

Each design specifies interfaces, data structures, concurrency models, and validation strategies. Implementation can proceed module-by-module with clear integration points.