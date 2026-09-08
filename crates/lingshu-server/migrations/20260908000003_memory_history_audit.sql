ALTER TABLE memory_retrieval_runs ADD COLUMN as_of timestamptz;
ALTER TABLE memory_retrieval_runs ADD COLUMN historical_result_ids uuid[] NOT NULL DEFAULT '{}';
