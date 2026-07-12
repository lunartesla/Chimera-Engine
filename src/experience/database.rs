use std::path::Path;
use anyhow::{Result, Context};
use rusqlite::{Connection, params};

use super::schema::{ExperienceRecord, ReplayQuery, ExperiencePassDescriptor, FunctionStats};

/// SQLite-backed experience database
pub struct ExperienceDatabase {
    conn: Connection,
}

impl ExperienceDatabase {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)
            .with_context(|| format!("Failed to open experience database at {:?}", path))?;

        Self::initialize_schema(&conn)?;

        Ok(Self { conn })
    }

    fn initialize_schema(conn: &Connection) -> Result<()> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS experiences (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                module_shape_hash TEXT NOT NULL,
                goal_id TEXT NOT NULL,
                pipeline_id TEXT NOT NULL,
                passes_json TEXT NOT NULL,
                function_stats_json TEXT,
                fitness REAL NOT NULL,
                baseline_instructions INTEGER NOT NULL,
                final_instructions INTEGER NOT NULL,
                instruction_reduction INTEGER NOT NULL,
                validation_passed INTEGER NOT NULL,
                execution_time_us REAL NOT NULL,
                memory_peak_bytes INTEGER NOT NULL,
                generation INTEGER NOT NULL,
                island_id INTEGER,
                strain_id TEXT,
                temperature REAL NOT NULL,
                seed_fitness REAL NOT NULL,
                timestamp INTEGER NOT NULL,
                run_id TEXT NOT NULL,
                success INTEGER NOT NULL,
                correction_needed INTEGER NOT NULL
            )",
            [],
        ).context("Failed to create experiences table")?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_shape_goal ON experiences(module_shape_hash, goal_id)",
            [],
        ).context("Failed to create index")?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_fitness ON experiences(fitness DESC)",
            [],
        ).context("Failed to create fitness index")?;

        Ok(())
    }

    pub fn insert(&self, record: &ExperienceRecord) -> Result<()> {
        let passes_json = serde_json::to_string(&record.passes)
            .context("Failed to serialize passes")?;

        let function_stats_json = serde_json::to_string(&record.function_stats)
            .context("Failed to serialize function_stats")?;

        self.conn.execute(
            "INSERT INTO experiences (
                module_shape_hash, goal_id, pipeline_id, passes_json,
                function_stats_json, fitness, baseline_instructions, final_instructions,
                instruction_reduction, validation_passed, execution_time_us,
                memory_peak_bytes, generation, island_id, strain_id,
                temperature, seed_fitness, timestamp, run_id, success, correction_needed
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21)",
            params![
                record.module_shape_hash,
                record.goal_id,
                record.pipeline_id,
                passes_json,
                function_stats_json,
                record.fitness,
                record.baseline_instructions as i64,
                record.final_instructions as i64,
                record.instruction_reduction as i64,
                record.validation_passed as i64,
                record.execution_time_us,
                record.memory_peak_bytes as i64,
                record.generation as i64,
                record.island_id.map(|i| i as i64),
                record.strain_id,
                record.temperature as f64,
                record.seed_fitness as f64,
                record.timestamp as i64,
                record.run_id,
                record.success as i64,
                record.correction_needed as i64,
            ],
        ).context("Failed to insert experience record")?;

        Ok(())
    }

    pub fn query(&self, query: &ReplayQuery) -> Result<Vec<ExperienceRecord>> {
        // Build SQL and params based on which filters are active
        // Use explicit columns to avoid SELECT * column index issues (id is auto-increment)
        let (sql, has_min_fitness) = if query.module_shape_hash.is_some() && query.goal_id.is_some() {
            if query.min_fitness != 0.0 {
                ("SELECT module_shape_hash, goal_id, pipeline_id, passes_json, function_stats_json, fitness, baseline_instructions, final_instructions, instruction_reduction, validation_passed, execution_time_us, memory_peak_bytes, generation, island_id, strain_id, temperature, seed_fitness, timestamp, run_id, success, correction_needed FROM experiences WHERE module_shape_hash = ? AND goal_id = ? AND fitness >= ? ORDER BY fitness DESC LIMIT ?", true)
            } else {
                ("SELECT module_shape_hash, goal_id, pipeline_id, passes_json, function_stats_json, fitness, baseline_instructions, final_instructions, instruction_reduction, validation_passed, execution_time_us, memory_peak_bytes, generation, island_id, strain_id, temperature, seed_fitness, timestamp, run_id, success, correction_needed FROM experiences WHERE module_shape_hash = ? AND goal_id = ? ORDER BY fitness DESC LIMIT ?", false)
            }
        } else if query.module_shape_hash.is_some() {
            if query.min_fitness != 0.0 {
                ("SELECT module_shape_hash, goal_id, pipeline_id, passes_json, function_stats_json, fitness, baseline_instructions, final_instructions, instruction_reduction, validation_passed, execution_time_us, memory_peak_bytes, generation, island_id, strain_id, temperature, seed_fitness, timestamp, run_id, success, correction_needed FROM experiences WHERE module_shape_hash = ? AND fitness >= ? ORDER BY fitness DESC LIMIT ?", true)
            } else {
                ("SELECT module_shape_hash, goal_id, pipeline_id, passes_json, function_stats_json, fitness, baseline_instructions, final_instructions, instruction_reduction, validation_passed, execution_time_us, memory_peak_bytes, generation, island_id, strain_id, temperature, seed_fitness, timestamp, run_id, success, correction_needed FROM experiences WHERE module_shape_hash = ? ORDER BY fitness DESC LIMIT ?", false)
            }
        } else {
            if query.min_fitness != 0.0 {
                ("SELECT module_shape_hash, goal_id, pipeline_id, passes_json, function_stats_json, fitness, baseline_instructions, final_instructions, instruction_reduction, validation_passed, execution_time_us, memory_peak_bytes, generation, island_id, strain_id, temperature, seed_fitness, timestamp, run_id, success, correction_needed FROM experiences WHERE fitness >= ? ORDER BY fitness DESC LIMIT ?", true)
            } else {
                ("SELECT module_shape_hash, goal_id, pipeline_id, passes_json, function_stats_json, fitness, baseline_instructions, final_instructions, instruction_reduction, validation_passed, execution_time_us, memory_peak_bytes, generation, island_id, strain_id, temperature, seed_fitness, timestamp, run_id, success, correction_needed FROM experiences ORDER BY fitness DESC LIMIT ?", false)
            }
        };

        let sql = sql.to_string();
        let limit = query.limit as i64;

        let results = match (&query.module_shape_hash, &query.goal_id, has_min_fitness) {
            (Some(shape), Some(goal), true) => {
                self.conn.prepare(&sql)?
                    .query_map(params![shape, goal, query.min_fitness, limit], Self::row_to_record)?
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .context("Failed to collect experiences")?
            }
            (Some(shape), Some(goal), false) => {
                self.conn.prepare(&sql)?
                    .query_map(params![shape, goal, limit], Self::row_to_record)?
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .context("Failed to collect experiences")?
            }
            (Some(shape), None, true) => {
                self.conn.prepare(&sql)?
                    .query_map(params![shape, query.min_fitness, limit], Self::row_to_record)?
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .context("Failed to collect experiences")?
            }
            (Some(shape), None, false) => {
                self.conn.prepare(&sql)?
                    .query_map(params![shape, limit], Self::row_to_record)?
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .context("Failed to collect experiences")?
            }
            (None, None, true) => {
                self.conn.prepare(&sql)?
                    .query_map(params![query.min_fitness, limit], Self::row_to_record)?
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .context("Failed to collect experiences")?
            }
            (None, None, false) => {
                self.conn.prepare(&sql)?
                    .query_map(params![limit], Self::row_to_record)?
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .context("Failed to collect experiences")?
            }
            (None, Some(_), _) => {
                // goal_id alone is rare - just query all
                self.conn.prepare(&sql)?
                    .query_map(params![limit], Self::row_to_record)?
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .context("Failed to collect experiences")?
            }
        };

        Ok(results)
    }

    fn row_to_record(row: &rusqlite::Row) -> rusqlite::Result<ExperienceRecord> {
        let passes_json: String = row.get(3)?;
        let passes: Vec<ExperiencePassDescriptor> = serde_json::from_str(&passes_json)
            .map_err(|_| rusqlite::types::FromSqlError::InvalidType)?;

        let function_stats_json: Option<String> = row.get(4)?;
        let function_stats: FunctionStats = match function_stats_json {
            Some(s) => serde_json::from_str(&s)
                .map_err(|_| rusqlite::types::FromSqlError::InvalidType)?,
            None => FunctionStats::default(),
        };

        Ok(ExperienceRecord {
            module_shape_hash: row.get(0)?,
            goal_id: row.get(1)?,
            pipeline_id: row.get(2)?,
            passes,
            function_stats,
            fitness: row.get(5)?,
            baseline_instructions: row.get(6)?,
            final_instructions: row.get(7)?,
            instruction_reduction: row.get(8)?,
            validation_passed: row.get(9)?,
            execution_time_us: row.get(10)?,
            memory_peak_bytes: row.get(11)?,
            generation: row.get(12)?,
            island_id: row.get(13)?,
            strain_id: row.get(14)?,
            temperature: row.get(15)?,
            seed_fitness: row.get(16)?,
            timestamp: row.get(17)?,
            run_id: row.get(18)?,
            success: row.get(19)?,
            correction_needed: row.get(20)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tempfile::TempDir;

    fn sample_record() -> ExperienceRecord {
        ExperienceRecord {
            module_shape_hash: "hash_abc".to_string(),
            goal_id: "goal_test".to_string(),
            pipeline_id: "pipeline_1".to_string(),
            passes: vec![ExperiencePassDescriptor {
                id: "cse".to_string(),
                params: HashMap::new(),
            }],
            function_stats: FunctionStats {
                instruction_count: 100,
                block_count: 5,
                max_depth: 2,
                constant_count: 10,
                store_count: 20,
                branch_count: 15,
            },
            fitness: 0.85,
            baseline_instructions: 150,
            final_instructions: 100,
            instruction_reduction: 50,
            validation_passed: true,
            execution_time_us: 45.5,
            memory_peak_bytes: 1024,
            generation: 1,
            island_id: Some(0),
            strain_id: Some("strain_1".to_string()),
            temperature: 0.5,
            seed_fitness: 0.7,
            timestamp: 1234567890,
            run_id: "run_001".to_string(),
            success: true,
            correction_needed: false,
        }
    }

    #[test]
    fn test_database_insert_and_query() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test_experience.db");
        let db = ExperienceDatabase::open(&db_path).unwrap();

        let record = sample_record();
        db.insert(&record).unwrap();

        let query = ReplayQuery::new()
            .with_shape("hash_abc")
            .with_goal("goal_test");

        let results = db.query(&query).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].module_shape_hash, "hash_abc");
        assert!(results[0].validation_passed);
    }

    #[test]
    fn test_query_empty_result() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test_experience_empty.db");
        let db = ExperienceDatabase::open(&db_path).unwrap();

        let query = ReplayQuery::new()
            .with_shape("nonexistent")
            .with_goal("goal_test");

        let results = db.query(&query).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_query_with_min_fitness() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test_experience_fitness.db");
        let db = ExperienceDatabase::open(&db_path).unwrap();

        let mut record1 = sample_record();
        record1.fitness = 0.5;
        db.insert(&record1).unwrap();

        let mut record2 = sample_record();
        record2.fitness = 0.95;
        db.insert(&record2).unwrap();

        let query = ReplayQuery::new()
            .with_shape("hash_abc")
            .with_goal("goal_test")
            .with_min_fitness(0.9);

        let results = db.query(&query).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].fitness, 0.95);
    }
}