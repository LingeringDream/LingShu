#!/usr/bin/env python3
"""End-to-end LoCoMo QA evaluation with a real LLM (Ollama llama3.1:8b).

Legacy exploratory judge-accuracy experiment, NOT official LoCoMo token F1.
For category-aware F1 and a paired protocol, use locomo_qa_v2.py.
Here we (1) retrieve evidence with two
strategies (static RAG vs LingShu adaptive), (2) generate an answer with a real
LLM over that evidence, and (3) judge answer equivalence against the gold
answer. The judge is the same local model, temperature 0, one token output.

Retrieval below is an experimental Python approximation, not the production
Rust scorer (its weight composition and tie-breaking differ).
Embeddings are the frozen matrix from `build_locomo_corpus.py`.
"""

from __future__ import annotations

import argparse
import json
import math
import random
import struct
import time
import urllib.request
from collections import Counter
from datetime import datetime
from pathlib import Path

import numpy as np

ROOT = Path(__file__).resolve().parent.parent
PUBLIC = ROOT / "datasets" / "public"
OLLAMA = "http://localhost:11434/api/generate"
MODEL = "llama3.1:8b"
DIM = 384

# --- faithful port of evolutionary_memory.rs scoring ------------------------

DEFAULT_W = {
    "semantic": 0.34, "lexical": 0.18, "recency": 0.16,
    "trust": 0.13, "frequency": 0.08, "importance": 0.11,
}
FORWARD = ["now", "current", "currently", "latest", "recent", "recently", "these days", "at present"]
BACKWARD = ["when did", "what time", "how long ago", "last time", "back then", "at that time", "did you ever"]
SYNTHESIS = ["summarize", "consolidate", "integrate", "across"]


def norm(w: dict[str, float]) -> dict[str, float]:
    w = {k: max(0.0, v) for k, v in w.items()}
    tot = sum(w.values())
    if tot <= 1e-6:
        return {k: 1.0 / len(w) for k in w}
    return {k: v / tot for k, v in w.items()}


def for_query(query: str) -> dict[str, float]:
    w = dict(DEFAULT_W)
    q = query.lower()
    forward = any(n in q for n in FORWARD)
    backward = any(n in q for n in BACKWARD)
    synthesis = any(n in q for n in SYNTHESIS)
    if forward and not backward:
        w["recency"] += 0.30; w["trust"] += 0.08; w["semantic"] -= 0.14
        w["lexical"] -= 0.08; w["importance"] -= 0.10; w["frequency"] -= 0.06
    elif backward and not forward:
        w["recency"] = max(0.0, w["recency"] - 0.12); w["semantic"] += 0.10
        w["lexical"] += 0.08; w["trust"] -= 0.04; w["importance"] -= 0.02
    elif synthesis:
        w["recency"] = 0.0; w["semantic"] += 0.12; w["lexical"] += 0.06; w["trust"] += 0.02
    return norm(w)


def lexical(a: str, b: str) -> float:
    def terms(t: str) -> set[str]:
        cs = [c for c in t.lower() if c.isalnum()]
        s = set(cs)
        s.update("".join(p) for p in zip(cs, cs[1:]))
        return s
    x, y = terms(a), terms(b)
    if not x or not y:
        return 0.0
    return len(x & y) / len(x | y)


def recency_score(age_days: float) -> float:
    return 2.0 ** (-age_days / 30.0)


def frequency_score(access: int) -> float:
    return min(1.0, math.log(max(0, access) + 1) / math.log(5.0))


def score_memory(query, content, importance, trust, access, age_days, semantic, weights):
    s = semantic * weights["semantic"] + lexical(query, content) * weights["lexical"]
    s += recency_score(age_days) * weights["recency"] + trust * weights["trust"]
    s += frequency_score(access) * weights["frequency"] + importance * weights["importance"]
    return s


# --- data loading -----------------------------------------------------------

def load_corpus():
    corpus = json.loads((PUBLIC / "locomo_corpus.json").read_text(encoding="utf-8"))
    with (PUBLIC / "locomo_emb.f32").open("rb") as fh:
        n_rows, dim = struct.unpack("<II", fh.read(8))
        vec = np.frombuffer(fh.read(n_rows * dim * 4), dtype="<f4").reshape(n_rows, dim)
    return corpus, vec


def call_llm(prompt: str, temperature: float = 0.0) -> str:
    body = json.dumps({"model": MODEL, "prompt": prompt, "stream": False,
                       "options": {"temperature": temperature}}).encode()
    req = urllib.request.Request(OLLAMA, data=body, headers={"Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=300) as resp:
        return json.loads(resp.read())["response"].strip()


def generate_answer(question: str, context: list[str]) -> str:
    ctx = "\n".join(f"- {c}" for c in context)
    prompt = (
        "Answer the question using ONLY the provided context. Be concise (a phrase or short sentence).\n"
        f"Context:\n{ctx}\n\nQuestion: {question}\nAnswer:"
    )
    return call_llm(prompt, 0.0)


def judge(question: str, reference: str, candidate: str) -> bool:
    prompt = (
        "Task: answer equivalence check.\n"
        f"Question: {question}\n"
        f"Reference: {reference}\n"
        f"Candidate: {candidate}\n"
        "Are the reference and candidate equivalent in meaning? Reply with exactly one token: yes or no."
    )
    out = call_llm(prompt, 0.0)
    return out.lower().startswith("yes")


# --- retrieval (ports the Rust strategy) ------------------------------------

def retrieve(case, vec, query_row, question, strategy, topk):
    qv = vec[query_row]
    rows = [m["row"] for m in case["memories"]]
    mvec = vec[rows]
    sem = mvec @ qv

    now = case["_now"]
    age = np.array([
        (now - datetime.fromisoformat(m["occurred_at"].replace("Z", "+00:00"))).total_seconds() / 86400.0
        if m["occurred_at"] else 0.0 for m in case["memories"]
    ], dtype=np.float32)
    imp = np.array([m["importance"] for m in case["memories"]], dtype=np.float32)
    tru = np.array([m["trust"] for m in case["memories"]], dtype=np.float32)
    lex = np.array([lexical(question, m["content"]) for m in case["memories"]], dtype=np.float32)
    acc = np.array([m["access_count"] for m in case["memories"]], dtype=np.float32)

    if strategy == "static":
        scores = sem
    else:
        # LingShu tuned: semantic anchor + lexical + trust, recency off (per report)
        w = {"semantic": 1.0, "lexical": 0.25, "recency": 0.0, "trust": 0.25,
             "frequency": 0.0, "importance": 0.0}
        w = norm(w)
        scores = (sem * w["semantic"] + lex * w["lexical"] + tru * w["trust"])

    order = np.argsort(-scores, kind="stable")
    top = [(case["memories"][i]["content"], case["memories"][i]["id"], float(scores[i])) for i in order[:topk]]
    return top


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--per-cat", type=int, default=20, help="samples per category")
    parser.add_argument("--seed", type=int, default=42)
    parser.add_argument("--topk", type=int, default=5)
    parser.add_argument("--out", default="output/locomo_qa_results.json")
    args = parser.parse_args()

    random.seed(args.seed)
    corpus, vec = load_corpus()

    # compute per-case now anchor
    for case in corpus["cases"]:
        stamps = [m["occurred_at"] for m in case["memories"] if m["occurred_at"]]
        case["_now"] = max(datetime.fromisoformat(s.replace("Z", "+00:00")) for s in stamps)

    # stratified sample
    samples = []
    by_cat = {}
    for ci, case in enumerate(corpus["cases"]):
        for q in case["queries"]:
            by_cat.setdefault(q["category"], []).append((ci, q))
    for cat, items in sorted(by_cat.items()):
        random.shuffle(items)
        samples.extend(items[: args.per_cat])
    random.shuffle(samples)
    print(f"sampled {len(samples)} QA pairs across {len(by_cat)} categories")

    results = []
    for si, (ci, q) in enumerate(samples):
        case = corpus["cases"][ci]
        question = q["query"]
        gold = q["answer"]
        row = q["row"]

        entry = {"question": question, "gold": gold, "category": q["category"],
                 "expected": q["expected"]}
        for strategy in ["static", "adaptive"]:
            top = retrieve(case, vec, row, question, strategy, args.topk)
            ctx = [c for c, _, _ in top]
            ans = generate_answer(question, ctx)
            correct = judge(question, gold, ans)
            entry[strategy] = {"answer": ans, "correct": correct,
                               "evidence_ids": [i for _, i, _ in top]}
            print(f"[{si+1}/{len(samples)}] cat={q['category']} {strategy}: {'YES' if correct else 'NO '}  {question[:40]}", flush=True)

        results.append(entry)

    # aggregate
    for strategy in ["static", "adaptive"]:
        corr = sum(1 for r in results if r[strategy]["correct"])
        print(f"\n=== {strategy} ===")
        print(f"  judge accuracy (not F1): {corr}/{len(results)} = {corr/len(results)*100:.1f}%")
        for cat in sorted(set(r['category'] for r in results)):
            sub = [r for r in results if r['category'] == cat]
            c = sum(1 for r in sub if r[strategy]["correct"])
            print(f"  cat {cat}: {c}/{len(sub)} = {c/len(sub)*100:.1f}%")

    Path(args.out).parent.mkdir(parents=True, exist_ok=True)
    Path(args.out).write_text(json.dumps(results, ensure_ascii=False, indent=1), encoding="utf-8")
    print(f"\nwrote {args.out}")


if __name__ == "__main__":
    main()
