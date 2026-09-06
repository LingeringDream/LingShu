"""Audit v4 construction and evaluate metadata-only baselines on dev/test.

No labels are repaired automatically. Flags identify review candidates, not
proof of the correct replacement label. Fixed depth-3 CART is fitted only on
development labels, with minimum leaf size 3 and no test hyperparameter search.
"""
import hashlib
import json
from collections import Counter
from pathlib import Path

from auto_state import classification_metrics, cluster_input, group_by_key

ROOT = Path(__file__).resolve().parent.parent
FEATURES = ("age_days", "trust", "importance", "access")


def fit_tree(turns, features, depth=3):
    counts = Counter(t["state"] for t in turns)
    leaf = {"label": sorted(counts, key=lambda c: (-counts[c], c))[0], "n": len(turns)}
    if depth == 0 or len(counts) == 1:
        return leaf
    def impurity(rows):
        c = Counter(t["state"] for t in rows)
        return len(rows) - sum(n * n for n in c.values()) / len(rows)
    best_loss, best = impurity(turns), None
    for feature in features:
        values = sorted({t[feature] for t in turns})
        for a, b in zip(values, values[1:]):
            threshold = (a + b) / 2
            left = [t for t in turns if t[feature] <= threshold]
            right = [t for t in turns if t[feature] > threshold]
            if min(len(left), len(right)) < 3:
                continue
            loss = impurity(left) + impurity(right)
            if loss < best_loss:
                best_loss, best = loss, (feature, threshold, left, right)
    if best is None:
        return leaf
    feature, threshold, left, right = best
    return {"feature": feature, "threshold": threshold,
            "left": fit_tree(left, features, depth - 1), "right": fit_tree(right, features, depth - 1)}


def classify(tree, turn):
    if "label" in tree:
        return tree["label"]
    return classify(tree["left"] if turn[tree["feature"]] <= tree["threshold"] else tree["right"], turn)


def audit(data):
    flags = []
    for key, members in group_by_key(data["turns"]).items():
        for t in members:
            if t["state"] == "duplicate" and not any(u["id"] != t["id"] and u["value"] == t["value"] for u in members):
                flags.append({"type": "duplicate_without_equal_value", "key": key, "id": t["id"]})
            if "更正" in t["content"]:
                younger = [u["id"] for u in members if u["value"] != t["value"] and u["age_days"] < t["age_days"]]
                if younger:
                    flags.append({"type": "correction_precedes_different_claim", "key": key, "id": t["id"], "younger": younger})
            if t["state"] == "stale" and "只限本周" in t["content"] and t["age_days"] <= 6:
                flags.append({"type": "expiry_needs_calendar_evidence", "key": key, "id": t["id"]})
            if "？" in t["content"]:
                flags.append({"type": "question_embedded_in_fact_statement", "key": key, "id": t["id"]})
        if any(t["state"] == "unresolved" for t in members) and any(t["state"] == "active" for t in members):
            flags.append({"type": "conflict_requires_resolution_policy", "key": key})
    return flags


def main():
    data = {s: json.loads((ROOT / "datasets" / f"zte_memory_v4_{s}.json").read_text(encoding="utf-8"))
            for s in ("all", "dev", "test")}
    dev, test = data["dev"]["turns"], data["test"]["turns"]
    assert not ({t["fact_key"] for t in dev} & {t["fact_key"] for t in test})
    baselines = {}
    for name, features in [("age_only_depth3", ("age_days",)), ("metadata_only_depth3", FEATURES)]:
        tree = fit_tree(dev, features)
        baselines[name] = {"tree": tree, "scores": {}}
        for split in ("dev", "test"):
            turns = data[split]["turns"]
            predictions = {t["id"]: classify(tree, t) for t in turns}
            baselines[name]["scores"][split] = classification_metrics({t["id"]: t["state"] for t in turns}, predictions)
            baselines[name].setdefault("predictions", {})[split] = predictions
    flags = audit(data["all"])
    result = {"protocol": {"split": "existing dev/test, disjoint fact_keys but shared archetypes",
                           "max_depth": 3, "min_leaf": 3, "fit_labels": "dev only",
                           "conclusion": "predictability probes, not proof of leakage absence or model limits"},
              "sizes": {s: {"turns": len(d["turns"]), "queries": len(d["queries"]),
                            "classes": dict(Counter(t["state"] for t in d["turns"])),
                            "query_groups": dict(Counter(q["group"] for q in d["queries"]))} for s, d in data.items()},
              "original_size_goal_met": len(data["all"]["turns"]) >= 200 and len(data["all"]["queries"]) >= 100,
              "flags": flags, "flag_counts": dict(Counter(f["type"] for f in flags)), "baselines": baselines,
              "dataset_sha256": hashlib.sha256((ROOT / "datasets/zte_memory_v4_all.json").read_bytes()).hexdigest()}
    out = ROOT / "output" / "governance_v4_audit.json"
    out.write_text(json.dumps(result, ensure_ascii=False, indent=2), encoding="utf-8")
    # A blind review packet: no gold, no original IDs, no archetype names.
    packet, mapping = [], {}
    for i, (key, members) in enumerate(group_by_key(data["all"]["turns"]).items()):
        text, aliases = cluster_input(members)
        rid = f"review-{i:04d}"
        packet.append({"review_id": rid, "snapshot": data["all"]["provenance"]["snapshot"],
                       "observations": text, "reviewed": False, "labels": {}, "notes": "",
                       "insufficient_evidence_ids": []})
        mapping[rid] = {"fact_key": key, "aliases": aliases}
    review_path = ROOT / "output/governance_v4_blind_review.jsonl"
    mapping_path = ROOT / "output/governance_v4_review_mapping.json"
    # Preserve any human annotation work on subsequent audit runs.
    if not review_path.exists() and not mapping_path.exists():
        review_path.write_text("\n".join(json.dumps(r, ensure_ascii=False) for r in packet) + "\n", encoding="utf-8")
        mapping_path.write_text(json.dumps(mapping, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps({"sizes": result["sizes"], "flag_counts": result["flag_counts"],
                      "test_baselines": {k: v["scores"]["test"] for k, v in baselines.items()}}, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
