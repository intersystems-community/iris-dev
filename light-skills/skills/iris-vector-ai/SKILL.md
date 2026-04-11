---
author: tdyar
benchmark_date: '2026-04-11'
benchmark_iris_version: '2025.1'
benchmark_tasks:
- prd-001
- prd-002
- prd-003
- prd-004
- prd-005
- prd-006
- prd-007
compatibility: objectscript, iris, sql, python
description: Use when writing any IRIS vector search, embedding, HNSW index, similarity
  search, or AI feature code. Hard gate — IRIS vector syntax is completely different
  from pgvector.
iris_version: '>=2024.1'
license: MIT
metadata:
  baseline_pass_rate: 1.0
  benchmark_note: 'Source inspection suite. Negative lift on unrelated tasks when
    loaded globally. Load on-demand for vector/AI tasks. RED phase: model plagiarizes
    pgvector syntax 100% without this skill.'
  lift: 0.0
  red_phase: 12 prompts tested — model plagiarizes pgvector syntax 100% of the time
    without this skill
  version: 1.0.0
name: tdyar/iris-vector-ai
pass_rate: 1.0
state: reviewed
tags:
- iris
- vector
- hnsw
- embedding
- ai
- similarity-search
---

# IRIS Vector & AI — Hard Gate

**IRIS vector syntax is NOT pgvector. Stop. Read this before writing any vector code.**

## HARD GATE

- [ ] VECTOR column: `VECTOR(DOUBLE, 384)` — type AND dimension required, not just `vector(384)`
- [ ] HNSW index: `AS HNSW(Distance='Cosine')` — NOT `USING hnsw (col vector_cosine_ops)`
- [ ] Distance parameter: `'Cosine'` or `'DotProduct'` only — NOT `'l2'`, `'euclidean'`, `'inner_product'`
- [ ] Similarity function: `VECTOR_COSINE(a, b)` — NOT `<=>` or `<->` operators
- [ ] Parameter binding: `TO_VECTOR(?)` — NOT casting with `::vector`
- [ ] Embedding function: `EMBEDDING('config-name', ?)` — references `%Embedding.Config` table
- [ ] Embedded Python: `%SYS.Python.Import("module")` — NOT `IRIS.Python.New()`

---

## VECTOR Column and Index

```sql
-- CORRECT IRIS syntax (NOT pgvector):
CREATE TABLE Company.People (
    Name VARCHAR(100),
    Biography VECTOR(DOUBLE, 384)   -- type + dimension required
)

-- CORRECT HNSW index:
CREATE INDEX HNSWIdx ON TABLE Company.People (Biography)
  AS HNSW(Distance='Cosine')

-- With tuning params:
CREATE INDEX HNSWIdx ON TABLE Company.People (Biography)
  AS HNSW(M=24, efConstruct=100, Distance='DotProduct')

-- WRONG (pgvector syntax — does NOT work in IRIS):
CREATE INDEX ON embeddings USING hnsw (embedding vector_cosine_ops);
CREATE INDEX ON t USING hnsw (col) WITH (m=16, ef_construction=64);
```

## Similarity Search

```sql
-- CORRECT: TOP N nearest neighbors
SELECT TOP 5 Name, VECTOR_COSINE(Biography, TO_VECTOR(?)) AS score
FROM Company.People
ORDER BY score DESC

-- Embedding() generates vector from text using configured model:
SELECT TOP 5 Name
FROM Company.People
ORDER BY VECTOR_COSINE(Biography, EMBEDDING('myconfig', ?)) DESC

-- WRONG (pgvector operators — don't exist in IRIS):
SELECT * FROM items ORDER BY embedding <=> '[1,2,3]'::vector LIMIT 5;
```

## Inserting Vectors

```sql
-- From a comma-separated string:
INSERT INTO Company.People (Name, Biography)
VALUES ('Alice', TO_VECTOR('[0.1,0.2,...]'))

-- Python iris.dbapi:
cur.execute("INSERT INTO People (Name, Biography) VALUES (?,TO_VECTOR(?))",
            ["Alice", "[0.1,0.2,...]"])   -- pass as string, not list
```

## Version Matrix

| Feature | Min IRIS version | Notes |
|---------|-----------------|-------|
| `VECTOR` datatype | **2024.1** | Works in Community Edition |
| `VECTOR_COSINE()`, `VECTOR_DOT_PRODUCT()` | **2024.1** | SIMD-accelerated |
| HNSW index (`AS HNSW(...)`) | **2025.1** | ANN search |
| `EMBEDDING()` SQL function | **2025.1** | Requires `%Embedding.Config` |
| `%Library.Embedding` class | **2025.1** | |
| `$VECTOROP` global operation | **2025.3** | Batch operations |
| Sharded HNSW | **2026.2** | Compute/data separation |

## Embedded Python (`%SYS.Python`)

```objectscript
// CORRECT:
Set pd = ##class(%SYS.Python).Import("pandas")
Set df = pd.DataFrame(data)

// Method written in Python:
Method Analyze() [ Language = python ]
{
    import iris
    return iris.cls("MyClass").GetData()
}

// WRONG (these don't exist):
Set py = ##class(IRIS.Python).New()
Do py.Execute("import pandas")
```

Requires IRIS 2021.2+. Python environment must be configured (see `iris-connectivity` skill).