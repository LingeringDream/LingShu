import copy
import unittest

import numpy as np

from build_zte_v3 import build
from locomo_scoring import score, token_f1
from retrieval_experiment import BM25, Index, fact_units, metrics, paired_interval


class ScoringTests(unittest.TestCase):
    def test_precision_is_not_hit_rate(self):
        result = metrics(["wrong", "gold", "x", "y", "z"], ["gold"], 5)
        self.assertEqual(result["precision"], .2)
        self.assertEqual(result["recall"], 1)
        self.assertEqual(result["mrr"], .5)

    def test_official_category_handling(self):
        self.assertEqual(token_f1("The cats", "cat"), 1)
        self.assertEqual(score("Paris", "Paris, London", 1), .5)
        self.assertEqual(score("Paris", "Paris; alternative", 3), 1)
        self.assertEqual(score("Paris", "Paris", 5), 0)
        self.assertEqual(score("Not mentioned", "false adversarial answer", 5), 1)
        self.assertEqual(token_f1("", ""), 0)

    def test_rare_term_bm25(self):
        scores = BM25(["common common common", "common telescope", "common banana"]).score("telescope")
        self.assertEqual(int(np.argmax(scores)), 1)
        self.assertEqual(scores[0], 0)

    def test_grounded_spans_and_no_gold_leakage(self):
        memories = [{"id": "a", "row": 0, "content": " I do not live in Paris. Where are you?",
                     "speaker": "Ann", "occurred_at": "2026-09-06T00:00:00Z"},
                    {"id": "b", "row": 1, "content": "I live in London.", "speaker": "Bob"}]
        units = fact_units(memories)
        self.assertEqual(len(units), 2)
        for unit in units:
            self.assertEqual(memories[unit["parent"]]["content"][unit["start"]:unit["end"]], unit["text"])
        self.assertIn("not live", units[0]["text"])
        vec = np.eye(2)
        case = {"memories": memories, "queries": [{"expected": ["a"], "answer": "London"}]}
        before = Index(case, vec).rank("Where does Ann live?", vec[0])
        other = copy.deepcopy(case)
        other["queries"][0] = {"expected": ["b"], "answer": "Paris"}
        self.assertEqual(before, Index(other, vec).rank("Where does Ann live?", vec[0]))

    def test_paired_cluster_interval(self):
        rows = [{"case_id": str(i), "static": {"recall": 0}, "bm25": {"recall": 1}} for i in range(4)]
        result = paired_interval(rows, "bm25")
        self.assertEqual(result["cluster_bootstrap_95"], [1, 1])

    def test_v3_disjoint_entities_and_changed_keys(self):
        data = build()
        self.assertEqual(len(data["queries"]), 300)
        self.assertEqual(sum(q["group"] == "fact-change" for q in data["queries"]), 150)
        keys = {split: {q["fact_key"] for q in data["queries"] if q["split"] == split}
                for split in ("dev", "test")}
        self.assertFalse(keys["dev"] & keys["test"])
        self.assertEqual(data, build())


if __name__ == "__main__":
    unittest.main()
