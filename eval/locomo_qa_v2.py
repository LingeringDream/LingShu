"""Paired local QA with checkpointing, official category-aware F1 and fixed budgets.

BM25 chosen using development retrieval results; all other arms remain reported
in retrieval_experiment.py. This is an offline experiment, not a production arm.
"""
import argparse
import hashlib
import json
import random
import urllib.request
from pathlib import Path

from locomo_qa import load_corpus
from locomo_scoring import score
from retrieval_experiment import Index, paired_interval


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--out", default="output/locomo_qa_v2.json")
    p.add_argument("--per-cat", type=int, default=20)
    p.add_argument("--model", default="llama3.1:8b")
    p.add_argument("--topk", type=int, default=5)
    p.add_argument("--max-context-words", type=int, default=600)
    args = p.parse_args()
    if min(args.per_cat, args.topk, args.max_context_words) < 1:
        p.error("sample count and context budgets must be positive")
    corpus, vec = load_corpus()
    with urllib.request.urlopen("http://localhost:11434/api/tags", timeout=10) as response:
        models = json.load(response)["models"]
    digest = next(m["digest"] for m in models if m["name"] == args.model)
    protocol = {"model": args.model, "model_digest": digest, "temperature": 0,
                "seed": 42, "num_predict": 128, "num_ctx": 4096,
                "topk": args.topk, "max_context_words": args.max_context_words,
                "per_cat": args.per_cat, "split": "last four dialogues (retrospective)",
                "arms": ["static", "bm25"], "metric": "LoCoMo category-aware token F1 / category-5 abstention",
                "corpus_sha256": hashlib.sha256(json.dumps(corpus, sort_keys=True).encode()).hexdigest(),
                "scope": "existing evidence-bearing corpus; not full official QA set"}
    path = Path(args.out)
    path.parent.mkdir(parents=True, exist_ok=True)
    result = {"protocol": protocol, "rows": []}
    if path.exists():
        result = json.loads(path.read_text(encoding="utf-8"))
        if result["protocol"] != protocol:
            raise ValueError("Existing checkpoint protocol differs; choose a new output path")
    rows = result["rows"]
    done = {(r["case_id"], r["query_index"]): r for r in rows}
    by_cat = {}
    for ci, case in enumerate(corpus["cases"]):
        if ci < 6:
            continue
        for qi, q in enumerate(case["queries"]):
            by_cat.setdefault(q["category"], []).append((ci, qi))
    rng = random.Random(42)
    selected = []
    for cat, items in sorted(by_cat.items()):
        rng.shuffle(items)
        selected.extend(items[:args.per_cat])
    rng.shuffle(selected)
    indices = {ci: Index(corpus["cases"][ci], vec) for ci in {ci for ci, _ in selected}}

    def save():
        tmp = path.with_suffix(".tmp")
        tmp.write_text(json.dumps(result, ensure_ascii=False, indent=2), encoding="utf-8")
        tmp.replace(path)

    for i, (ci, qi) in enumerate(selected):
        case, index = corpus["cases"][ci], indices[ci]
        q = case["queries"][qi]
        key = (case["case_id"], qi)
        row = done.get(key)
        if row is None:
            row = {"case_id": key[0], "query_index": qi, "question": q["query"],
                   "category": q["category"], "gold": q["answer"]}
            rows.append(row)
            done[key] = row
        rankings = index.rank(q["query"], vec[q["row"]])
        for arm in protocol["arms"]:
            if arm in row:
                continue
            memories = [case["memories"][j] for j in rankings[arm][:args.topk]]
            # Identical source formatting and truncation policy for both arms.
            context = "\n".join(f"[{m['occurred_at']}] {m.get('speaker', '')}: {m['content']}" for m in memories)
            context = " ".join(context.split()[:args.max_context_words])
            prompt = ("Answer using ONLY the conversation evidence. Give a concise phrase or short sentence. "
                      "If the answer is absent, reply 'No information available'. "
                      "Use the dated evidence to resolve relative dates.\n"
                      f"Evidence:\n{context}\nQuestion: {q['query']}\nAnswer:")
            body = json.dumps({"model": args.model, "prompt": prompt, "stream": False,
                               "options": {"temperature": 0, "seed": 42, "num_predict": 128, "num_ctx": 4096}}).encode()
            request = urllib.request.Request("http://localhost:11434/api/generate", data=body,
                                             headers={"Content-Type": "application/json"})
            with urllib.request.urlopen(request, timeout=300) as response:
                generated = json.load(response)
            answer = generated["response"].strip()
            row[arm] = {"answer": answer, "f1": score(answer, q["answer"], q["category"]),
                        "evidence_ids": [m["id"] for m in memories], "context": context,
                        "prompt_tokens": generated.get("prompt_eval_count"),
                        "generated_tokens": generated.get("eval_count")}
            save()
            print(f"{i+1}/{len(selected)} {arm} cat={q['category']} F1={row[arm]['f1']:.3f}", flush=True)
    summary = {}
    for cat in ["all", 1, 2, 3, 4, 5]:
        sub = [r for r in rows if cat == "all" or r["category"] == cat]
        summary[str(cat)] = {"n": len(sub), **{a: sum(r[a]["f1"] for r in sub) / len(sub)
                                               for a in protocol["arms"]}} if sub else {"n": 0}
    result["summary"] = summary
    result["paired_f1"] = paired_interval(rows, "bm25", "f1")
    result["complete"] = True
    save()
    print(json.dumps({"summary": summary, "paired": result["paired_f1"]}, indent=2))


if __name__ == "__main__":
    main()
