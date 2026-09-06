"""Automatic memory-state governance (active / superseded / stale / duplicate /
unresolved).

S1 evidence gap: the v3 synthetic set carries ORACLE `state` labels, which the
current retrieval experiments consume directly. That inflates the "state filter
99%" result — the system never had to *decide* which fact is stale. This module
answers the judge's question "if the system doesn't know which fact is old, does
99% still hold?" by predicting state from observable signals only (never the
gold `state`), then measuring *governance* quality directly (matching the Rust
Experiment B "pure governance" protocol), NOT a retrieval Top-1 that would
couple the measurement to an unrelated ranking signal.

Two predictors, same contract:
  - `rule`:  unsupervised. Uses age_days + trust + literal change/stale/duplicate
             markers in content. Deliberately does NOT assume "newest == active".
  - `llm`:   llama3.1:8b, temperature 0. Classifies each fact_key's *cluster* of
             versions together (a governor sees the clustered candidate set, not
             isolated turns). Offline, needs Ollama.

Governance metrics (the acceptance surface):
  - per-class P/R/F1 + accuracy
  - macro-F1 (mean over the five classes)
  - active_recall            (share of current facts kept)
  - false_elimination_rate   (active facts explicitly labeled non-active / gold active)
  - false_retention_rate     (share of non-current facts wrongly kept as active)
  - prediction_coverage     (valid predictions / evaluated turns)

Missing/invalid predictions count as classification errors, but are not reported
as an explicit keep or filter decision. Active nonrecognition includes them.
`1 - accuracy` is classification_error_rate, NOT a retrieval Oracle gap.
The five-way label benchmark is not a production retention/deletion policy:
unresolved and invalid outputs must remain reviewable; this script writes no store.

Run:
  python eval/auto_state.py --dataset datasets/zte_memory_v4_all.json \
      [--llm] [--limit N] --out output/auto_state_v4.json
"""
from __future__ import annotations

import argparse
import hashlib
import json
import random
import urllib.request
from collections import defaultdict
from pathlib import Path

CLASSES = ("active", "superseded", "stale", "duplicate", "unresolved")


def observable_turns(turns):
    """Whitelist model inputs; fact_key is used only for oracle clustering."""
    fields = ("id", "fact_key", "content", "age_days", "trust", "importance", "access")
    return [{k: t[k] for k in fields if k in t} for t in turns]


def parse_predictions(raw, aliases):
    """Never silently convert missing, duplicated or invalid output to active."""
    predictions = {original: None for original in aliases.values()}
    errors, seen = [], set()
    for line in raw.splitlines():
        if not line.strip():
            continue
        parts = line.strip().split("|")
        if len(parts) != 2 or parts[0].strip() not in aliases:
            errors.append({"kind": "malformed_or_unknown_id", "line": line})
            continue
        alias, state = (part.strip() for part in parts)
        if alias in seen:
            predictions[aliases[alias]] = None
            errors.append({"kind": "duplicate_id", "id": alias})
            continue
        seen.add(alias)
        if state not in CLASSES:
            errors.append({"kind": "invalid_status", "id": alias, "status": state})
            continue
        predictions[aliases[alias]] = state
    for alias in aliases.keys() - seen:
        errors.append({"kind": "missing_id", "id": alias})
    return predictions, errors


def cluster_input(members):
    ordered = list(members)
    # Vary permutations by observable content, not gold-like IDs or cluster order.
    # A fixed permutation reused for every two-version cluster would preserve
    # the v0/v1 positional shortcut even after aliases replace source IDs.
    seed = hashlib.sha256(json.dumps(sorted((m["content"], m["age_days"]) for m in members),
                                     ensure_ascii=False).encode()).digest()
    random.Random(seed).shuffle(ordered)
    aliases = {f"m{i:04d}": m["id"] for i, m in enumerate(ordered)}
    text = "\n".join(f"{alias}|{m['age_days']}d ago|{m['content']}"
                     for alias, m in zip(aliases, ordered))
    return text, aliases


def load_dataset(path):
    return json.loads(Path(path).read_text(encoding="utf-8"))


def group_by_key(turns):
    groups = defaultdict(list)
    for t in turns:
        groups[t["fact_key"]].append(t)
    return groups


# --------------------------------------------------------------------------- #
# Rule predictor
# --------------------------------------------------------------------------- #
# Content cues that a memory announces change / current-ness vs lapsed-ness.
# A governor that has NO model can still read these literal markers.
_CHANGE_MARKERS = ("改", "换", "现在", "目前", "最近", "更正", "迁移", "搬", "更新", "已经", "改成", "换成")
_STALE_MARKERS = ("临时", "一次性", "过期", "结束", "取消了", "撤销", "作废", "废弃")
_DUP_MARKERS = ("也就是", "即", "重写", "重述", "同款", "一模一样")


def predict_rule(turns):
    """Predict state from single-memory observable signals + literal content
    cues, WITHOUT assuming 'newest version == active'.

    Signals used (all available to a real governor, none is the gold `state`):
      - age_days (recency prior, but NOT decisive by itself)
      - literal change/stale/duplicate markers in `content`
      - per-fact_key sibling count (does this subject have multiple versions?)

    Decision order (deliberately conservative):
      1. any literal stale marker                      -> stale
      2. literal change/correction marker AND recent    -> active
      3. literal duplicate marker                       -> duplicate
      4. recent (< 15d)                                  -> active
      5. old (>= 90d) and has a newer sibling           -> superseded
      6. old (>= 90d) and no sibling                    -> stale
      7. otherwise                                      -> active (stable)

    This is the existing experimental heuristic, not a validated production
    governor. Quality metadata is retained by the interface but unused here.
    """
    groups = group_by_key(turns)
    # sibling lookup: which fact_keys have multiple versions
    multi = {k for k, v in groups.items() if len(v) >= 2}

    pred = {}
    for t in turns:
        content = t["content"]
        age = t["age_days"]
        key = t["fact_key"]
        has_sibling = key in multi

        if any(m in content for m in _STALE_MARKERS):
            st = "stale"
        elif any(m in content for m in _CHANGE_MARKERS) and age < 30:
            st = "active"
        elif any(m in content for m in _DUP_MARKERS):
            st = "duplicate"
        elif age < 15:
            st = "active"
        elif age >= 90:
            st = "superseded" if has_sibling else "stale"
        else:
            st = "active"

        pred[t["id"]] = st
    return pred


# --------------------------------------------------------------------------- #
# LLM predictor
# --------------------------------------------------------------------------- #
def predict_llm(turns, model="llama3.1:8b", audit=None, checkpoint=None):
    """Classify each turn's state with a local LLM (temperature 0).

    Unlike the single-turn prompt (which underspecifies the task — a lone
    memory carries too little context to judge superseded/stale), this version
    feeds the LLM *all sibling versions of the same fact_key* together and asks
    it to label each. That mirrors a real governor, which sees the clustered
    candidate set for a subject before deciding what is current.
    """
    groups = group_by_key(turns)
    pred = {}
    items = list(groups.items())

    sys_ctx = (
        "You are classifying stored memories about the same subject. For each "
        "memory decide its status:\n"
        "- active: the current, correct fact now.\n"
        "- superseded: replaced by a newer fact about the same subject.\n"
        "- stale: faded / lapsed (a one-off event that already ended, or an old "
        "plan that no longer holds), not necessarily replaced.\n"
        "- duplicate: repeats another memory here with different wording.\n"
        "- unresolved: conflicts with another memory here, not yet decided.\n"
        "Use BOTH the content AND how recent each memory is.\n"
        "Output one line per memory: id|status\n"
        "Output ONLY those lines, nothing else."
    )

    for index, (key, members) in enumerate(items):
        lines, aliases = cluster_input(members)
        prompt = sys_ctx + "\n\nMemories about one subject:\n" + lines + "\n"
        body = json.dumps({
            "model": model, "stream": False,
            "prompt": prompt,
            "options": {"temperature": 0, "seed": 42, "num_predict": 256},
        }).encode()
        req = urllib.request.Request("http://localhost:11434/api/generate",
                                     data=body, headers={"Content-Type": "application/json"})
        with urllib.request.urlopen(req, timeout=120) as resp:
            response = json.load(resp)
        out = response["response"].strip()
        parsed, errors = parse_predictions(out, aliases)
        pred.update(parsed)
        if audit is not None:
            audit.append({"cluster_key": key, "aliases": aliases, "prompt": prompt,
                          "raw_output": out, "predictions": parsed, "parse_errors": errors,
                          "done_reason": response.get("done_reason"),
                          "prompt_tokens": response.get("prompt_eval_count"),
                          "output_tokens": response.get("eval_count")})
        if checkpoint:
            checkpoint()
        print(f"LLM clusters {index+1}/{len(items)} valid={sum(v is not None for v in parsed.values())}/{len(parsed)}", flush=True)
    return pred


# --------------------------------------------------------------------------- #
# Metrics
# --------------------------------------------------------------------------- #
def classification_metrics(gold, pred):
    classes = CLASSES
    tp = {c: 0 for c in classes}
    fp = {c: 0 for c in classes}
    fn = {c: 0 for c in classes}
    for tid, g in gold.items():
        p = pred.get(tid)
        if p == g:
            tp[g] += 1
        else:
            fn[g] += 1
            if p in classes:
                fp[p] += 1
    out = {}
    for c in classes:
        prec = tp[c] / (tp[c] + fp[c]) if tp[c] + fp[c] else 0.0
        rec = tp[c] / (tp[c] + fn[c]) if tp[c] + fn[c] else 0.0
        f1 = 2 * prec * rec / (prec + rec) if prec + rec else 0.0
        out[c] = {"precision": round(prec, 4), "recall": round(rec, 4), "f1": round(f1, 4)}
    correct = sum(tp.values())
    total = len(gold)
    # False elimination counts only explicit non-active predictions. Invalid
    # outputs remain a separate failure; they never silently become active.
    active_total = sum(1 for g in gold.values() if g == "active")
    false_elim = sum(1 for tid, g in gold.items()
                     if g == "active" and pred.get(tid) in classes and pred[tid] != "active")
    out["accuracy"] = round(correct / total, 4) if total else 0.0
    out["false_elimination_rate"] = round(false_elim / max(1, active_total), 4)
    # False retention: a NON-active fact wrongly kept as active (the symmetric
    # failure — governance that is too permissive and lets stale/superseded/
    # duplicate/unresolved facts through).
    non_active_total = sum(1 for g in gold.values() if g != "active")
    false_retain = sum(1 for tid, g in gold.items()
                       if g != "active" and pred.get(tid) == "active")
    out["false_retention_rate"] = round(false_retain / max(1, non_active_total), 4)
    # Macro-F1: mean over the five classes (classes with no gold are skipped
    # to avoid a zero dragging the average — but an absent gold class is still
    # meaningful, so we report both present-class and all-class macro).
    present_f1 = [out[c]["f1"] for c in classes if tp[c] + fn[c] > 0]
    out["macro_f1_present"] = round(sum(present_f1) / len(present_f1), 4) if present_f1 else 0.0
    out["macro_f1_all"] = round(sum(out[c]["f1"] for c in classes) / len(classes), 4)
    # Active recall is the single most important per-class signal for a
    # retrieval governor: how many current facts we keep.
    out["active_recall"] = out["active"]["recall"]
    valid = sum(pred.get(tid) in classes for tid in gold)
    unknown_active = sum(g == "active" and pred.get(tid) not in classes for tid, g in gold.items())
    out["prediction_coverage"] = valid / total if total else 0.0
    out["invalid_or_missing_count"] = total - valid
    out["unknown_active_count"] = unknown_active
    out["active_total"] = active_total
    out["non_active_total"] = non_active_total
    out["false_elimination_count"] = false_elim
    out["false_retention_count"] = false_retain
    out["active_nonrecognition_rate"] = (false_elim + unknown_active) / active_total if active_total else None
    out["confusion_matrix"] = {
        g: {p: sum(value == g and (pred.get(tid) if pred.get(tid) in classes else "invalid") == p
                   for tid, value in gold.items()) for p in (*classes, "invalid")} for g in classes}
    return out


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--dataset", default="datasets/zte_memory_v4_test.json")
    p.add_argument("--out", default="output/auto_state_v4_audited.json")
    p.add_argument("--llm", action="store_true", help="also run the LLM predictor")
    p.add_argument("--model", default="llama3.1:8b")
    p.add_argument("--limit", type=int, default=None, help="limit fact-key clusters for ALL arms; smoke only")
    args = p.parse_args()

    data = load_dataset(args.dataset)
    if args.limit is not None and args.limit < 1:
        p.error("--limit must be positive")
    turns, queries = data["turns"], data["queries"]
    if args.limit is not None:
        keys = set(list(group_by_key(turns))[:args.limit])
        turns = [t for t in turns if t["fact_key"] in keys]
        queries = [q for q in queries if q["fact_key"] in keys]
    gold = {t["id"]: t["state"] for t in turns}
    observable = observable_turns(turns)

    result = {"schema_version": 2, "dataset": args.dataset, "n_turns": len(turns), "n_queries": len(queries),
              "complete": False, "smoke_only": args.limit is not None,
              "protocol": {"dataset_sha256": hashlib.sha256(Path(args.dataset).read_bytes()).hexdigest(),
                           "code_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
                           "model": args.model, "seed": 42, "oracle_fact_key_clustering": True,
                           "invalid_policy": "Unknown stays unknown; scored incorrect, reported separately; no store mutations",
                           "scope": "Five-class diagnostics on unvalidated synthetic labels; not end-to-end retrieval"}}
    out = Path(args.out)
    if out.exists():
        p.error("Output already exists; choose a new path to preserve previous evidence")
    out.parent.mkdir(parents=True, exist_ok=True)

    def save():
        tmp = out.with_suffix(".tmp")
        tmp.write_text(json.dumps(result, ensure_ascii=False, indent=2), encoding="utf-8")
        tmp.replace(out)

    # Rule predictor
    rule_pred = predict_rule(observable)
    result["rule"] = {
        "classification": classification_metrics(gold, rule_pred),
        "predictions": rule_pred,
    }
    result["always_active"] = {"classification": classification_metrics(gold, {tid: "active" for tid in gold})}
    result["gold"] = gold
    save()

    # LLM predictor
    if args.llm:
        with urllib.request.urlopen("http://localhost:11434/api/tags", timeout=10) as resp:
            tags = json.load(resp)["models"]
        result["protocol"]["model_digest"] = next(m["digest"] for m in tags if m["name"] == args.model)
        result["llm_raw"] = []
        llm_pred = predict_llm(observable, model=args.model, audit=result["llm_raw"], checkpoint=save)
        result["llm"] = {
            "classification": classification_metrics(gold, llm_pred),
            "predictions": llm_pred,
        }

    # Unified acceptance summary (pure governance): the five numbers the
    # reviewer cares about, plus the oracle gap on classification accuracy.
    def _summary(tag, cls):
        return {
            "macro_f1": cls["macro_f1_all"],
            "active_recall": cls["active_recall"],
            "false_elimination_rate": cls["false_elimination_rate"],
            "false_retention_rate": cls["false_retention_rate"],
            "accuracy": cls["accuracy"],
            "classification_error_rate": round(1.0 - cls["accuracy"], 4),
            "prediction_coverage": cls["prediction_coverage"],
            "invalid_or_missing_count": cls["invalid_or_missing_count"],
            "false_elimination_count": cls["false_elimination_count"],
            "active_total": cls["active_total"],
            "gate_status": "not_evaluated: labels require review; no retrieval or production acceptance claim",
        }
    result["summary"] = {tag: _summary(tag, result[tag]["classification"]) for tag in ("rule", "always_active")}
    if args.llm:
        result["summary"]["llm"] = _summary("llm", result["llm"]["classification"])

    result["complete"] = True
    save()
    print(json.dumps(result["summary"], ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
