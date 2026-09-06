"""Offline, paired LoCoMo retrieval experiment; no gold labels enter ranking.

Frozen protocol: first six corpus dialogues are development, remaining four
are held out from new tuning. Existing v1 work has seen all dialogues, so this
is a retrospective split, not a claim of a pristine public test set.
Run: python eval/retrieval_experiment.py --out output/retrieval_v2.json
"""
from __future__ import annotations

import argparse
import hashlib
import json
import re
from collections import Counter
from pathlib import Path

import numpy as np

from locomo_qa import load_corpus

STOP = set("a an the and or is are was were be been do does did to of in on at for with from i me my you your he she his her it its we our they their what which who when where how why would could can has have had that this".split())
ARMS = ("static", "bm25_text", "bm25", "hybrid", "fact_hybrid")


def terms(text):
    return [t for t in re.findall(r"[a-z0-9]+", text.lower()) if t not in STOP]


class BM25:
    def __init__(self, texts):
        counts = [Counter(terms(t)) for t in texts]
        n = len(counts)
        self.lengths = np.array([sum(c.values()) for c in counts], dtype=float)
        self.avg = max(float(self.lengths.mean()), 1.0)
        self.postings = {}
        for i, c in enumerate(counts):
            for term, tf in c.items():
                self.postings.setdefault(term, []).append((i, tf))
        self.n = n

    def score(self, query):
        scores = np.zeros(self.n)
        for term in set(terms(query)):
            postings = self.postings.get(term, [])
            if not postings:
                continue
            idx, tf = np.array(postings).T
            idf = np.log(1 + (self.n - len(postings) + .5) / (len(postings) + .5))
            denom = tf + 1.2 * (1 - .75 + .75 * self.lengths[idx] / self.avg)
            scores[idx] += idf * tf * 2.2 / denom
        return scores


def fact_units(memories):
    """Extractive sentence units, not LLM-resolved facts or verified claims.

    Preserve exact source spans, negation, speaker and timestamp. No QA input.
    Questions remain in the utterance fallback; never promoted to fact units.
    """
    units = []
    for parent, memory in enumerate(memories):
        text = memory["content"]
        for match in re.finditer(r"[^.!?\n]+(?:[.!?]+|$)", text):
            sentence = match.group().strip()
            if sentence.endswith("?") or len(terms(sentence)) < 2:
                continue
            start = match.start() + len(match.group()) - len(match.group().lstrip())
            units.append({"parent": parent, "source_id": memory["id"],
                          "start": start, "end": start + len(sentence),
                          "text": sentence, "speaker": memory.get("speaker", ""),
                          "occurred_at": memory.get("occurred_at")})
    return units


def reciprocal_rank(scores, positive_only=False):
    order = np.argsort(-scores, kind="stable")
    values = np.zeros(len(scores))
    values[order] = 1.0 / (60 + np.arange(1, len(scores) + 1))
    if positive_only:
        values[scores <= 0] = 0
    return values


class Index:
    def __init__(self, case, vec):
        self.memories = case["memories"]
        self.vectors = vec[[m["row"] for m in self.memories]]
        self.bm25 = BM25([f"{m.get('speaker', '')}: {m['content']}" for m in self.memories])
        self.bm25_text = BM25([m["content"] for m in self.memories])
        self.units = fact_units(self.memories)
        self.facts = BM25([f"{u['speaker']}: {u['text']}" for u in self.units]) if self.units else None

    def rank(self, question, query_vector):
        sem = self.vectors @ query_vector
        lexical = self.bm25.score(question)
        text_only = self.bm25_text.score(question)
        hybrid = reciprocal_rank(sem) + reciprocal_rank(lexical, True)
        fact_scores = np.zeros(len(self.memories))
        if self.facts:
            scores = self.facts.score(question)
            for unit, score in zip(self.units, scores):
                parent = unit["parent"]
                fact_scores[parent] = max(fact_scores[parent], score)
        # Max pooling gives one vote per original source, regardless of sentence count.
        fact_hybrid = reciprocal_rank(sem) + reciprocal_rank(fact_scores, True)
        return {name: np.argsort(-score, kind="stable").tolist() for name, score in
                zip(ARMS, (sem, text_only, lexical, hybrid, fact_hybrid))}


def metrics(ids, gold, k):
    relevant = set(gold)
    top = set(ids[:k])
    hits = len(top & relevant)
    return {"precision": hits / k, "recall": hits / len(relevant),
            "hit": float(hits > 0), "accuracy": float(ids[0] in relevant),
            "mrr": next((1 / (i + 1) for i, x in enumerate(ids) if x in relevant), 0.0)}


def summarize(rows, arm):
    return {metric: float(np.mean([r[arm][metric] for r in rows]))
            for metric in ("precision", "recall", "hit", "accuracy", "mrr")} if rows else {}


def paired_interval(rows, arm, metric="recall", repeats=2000):
    """Resample dialogue clusters, preserving paired outcomes and their dependence."""
    groups = {}
    for row in rows:
        groups.setdefault(row["case_id"], []).append(row[arm][metric] - row["static"][metric])
    sums = np.array([sum(g) for g in groups.values()])
    counts = np.array([len(g) for g in groups.values()])
    if not len(counts):
        return None
    draws = np.random.default_rng(20260906).integers(len(counts), size=(repeats, len(counts)))
    deltas = sums[draws].sum(axis=1) / counts[draws].sum(axis=1)
    return {"delta": float(sums.sum() / counts.sum()),
            "cluster_bootstrap_95": np.quantile(deltas, [.025, .975]).tolist(),
            "clusters": len(counts), "repeats": repeats,
            "caution": "Few dialogue clusters; interval is exploratory."}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", default="output/retrieval_v2.json")
    parser.add_argument("--topk", type=int, default=5)
    args = parser.parse_args()
    if args.topk < 1:
        parser.error("--topk must be positive")
    corpus, vec = load_corpus()
    rows, units = [], []
    for ci, case in enumerate(corpus["cases"]):
        index = Index(case, vec)
        units.extend({"case_id": case["case_id"], **u} for u in index.units)
        for qi, query in enumerate(case["queries"]):
            rankings = index.rank(query["query"], vec[query["row"]])
            row = {"case_id": case["case_id"], "query_index": qi, "category": query["category"],
                   "split": "dev" if ci < 6 else "test"}
            for arm, order in rankings.items():
                ids = [case["memories"][i]["id"] for i in order]
                row[arm] = {**metrics(ids, query["expected"], args.topk), "ids": ids[:args.topk]}
            rows.append(row)
        print(f"{ci + 1}/{len(corpus['cases'])} indexed: {len(index.units)} extractive units", flush=True)
    summary = {}
    for split in ("dev", "test", "all"):
        subset = [r for r in rows if split == "all" or r["split"] == split]
        summary[split] = {"n": len(subset), "arms": {a: summarize(subset, a) for a in ARMS},
                          "paired_recall": {a: paired_interval(subset, a) for a in ARMS[1:]}}
        print(split, json.dumps(summary[split]), flush=True)
    result = {"protocol": {"version": 2, "k": args.topk, "rrf_constant": 60,
                           "bm25_k1": 1.2, "bm25_b": .75, "tuned_on_test": False,
                           "split": "retrospective dialogue split: first 6 dev, last 4 test",
                           "fact_method": "extractive sentence spans; parent vector reused; full source context returned",
                           "corpus_sha256": hashlib.sha256(json.dumps(corpus, sort_keys=True).encode()).hexdigest(),
                           "limitation": "Evidence-bearing subset only; category 5 recall is not answerability."},
              "summary": summary, "by_category": {str(c): {a: summarize([r for r in rows if r['category'] == c], a)
                                                          for a in ARMS} for c in range(1, 6)}, "rows": rows}
    path = Path(args.out)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(result, ensure_ascii=False, indent=2), encoding="utf-8")
    path.with_suffix(".facts.json").write_text(json.dumps(units, ensure_ascii=False), encoding="utf-8")


if __name__ == "__main__":
    main()
