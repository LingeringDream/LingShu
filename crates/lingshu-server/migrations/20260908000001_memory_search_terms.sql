-- Indexed lexical candidate retrieval, including Chinese text without spaces.
-- This is token overlap, not BM25; final ranking lives in the shared service.
CREATE FUNCTION memory_search_terms(input text) RETURNS text[]
LANGUAGE sql IMMUTABLE PARALLEL SAFE STRICT AS $$
    SELECT COALESCE(array_agg(DISTINCT term ORDER BY term), ARRAY[]::text[])
    FROM (
        SELECT regexp_split_to_table(lower(input), '[^[:alnum:]_]+') AS term
        UNION ALL
        SELECT substr(lower(input), pos, width)
        FROM generate_series(1, char_length(input)) AS pos
        CROSS JOIN generate_series(1, 2) AS width
        WHERE substr(input, pos, 1) ~ '[一-鿿]'
          AND char_length(substr(input, pos, width)) = width
          AND substr(input, pos, width) ~ '^[一-鿿]+$'
    ) tokens
    WHERE term <> ''
$$;

CREATE INDEX memories_lexical_terms_idx ON memories
USING gin (memory_search_terms(content)) WHERE deleted_at IS NULL;
