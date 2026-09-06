import unittest

from auto_state import classification_metrics, cluster_input, observable_turns, parse_predictions
from audit_governance_v4 import audit, classify, fit_tree


class GovernanceTests(unittest.TestCase):
    def test_missing_is_not_active(self):
        predictions, errors = parse_predictions("m0000|stale", {"m0000": "a", "m0001": "b"})
        self.assertEqual(predictions, {"a": "stale", "b": None})
        metrics = classification_metrics({"a": "stale", "b": "active"}, predictions)
        self.assertEqual(metrics["accuracy"], .5)
        self.assertEqual(metrics["active_recall"], 0)
        self.assertEqual(metrics["prediction_coverage"], .5)
        self.assertEqual(metrics["unknown_active_count"], 1)
        self.assertEqual(metrics["false_elimination_count"], 0)
        self.assertEqual(metrics["active_nonrecognition_rate"], 1)
        self.assertTrue(errors)

    def test_duplicate_invalid_and_unknown_ids(self):
        pred, errors = parse_predictions("m0000|active\nm0000|stale\nm0001|ACTIVE\ninvented|active",
                                         {"m0000": "a", "m0001": "b"})
        self.assertEqual(pred, {"a": None, "b": None})
        self.assertEqual(len(errors), 3)

    def test_metadata_does_not_enter_prompt(self):
        turns = [{"id": "gen.corr.dup", "fact_key": "gen.unres.old", "content": "A plain statement.",
                  "age_days": 3, "state": "duplicate", "value": "gold", "split": "test"}]
        safe = observable_turns(turns)
        self.assertNotIn("state", safe[0])
        self.assertNotIn("value", safe[0])
        text, aliases = cluster_input(safe)
        self.assertNotIn("gen.", text)
        self.assertNotIn("duplicate", text)
        self.assertEqual(aliases, {"m0000": "gen.corr.dup"})

    def test_five_class_confusion_and_denominators(self):
        gold = {"a": "active", "b": "active", "c": "stale", "d": "unresolved"}
        pred = {"a": "active", "b": "superseded", "c": "active", "d": "unresolved"}
        m = classification_metrics(gold, pred)
        self.assertEqual(m["false_elimination_rate"], .5)
        self.assertEqual(m["false_retention_rate"], .5)
        self.assertEqual(m["prediction_coverage"], 1)
        self.assertEqual(m["confusion_matrix"]["active"]["superseded"], 1)

    def test_metadata_probe_does_not_read_test_labels(self):
        dev = [{"age_days": i, "state": "active" if i < 6 else "superseded"} for i in range(12)]
        model = fit_tree(dev, ("age_days",))
        self.assertEqual(classify(model, {"age_days": 2, "state": "active"}),
                         classify(model, {"age_days": 2, "state": "superseded"}))
        self.assertNotEqual(classify(model, {"age_days": 2}), classify(model, {"age_days": 10}))

    def test_review_flags_are_specific(self):
        turns = [{"id": "a", "fact_key": "x", "state": "duplicate", "value": "A", "age_days": 1, "content": "A"},
                 {"id": "b", "fact_key": "x", "state": "active", "value": "B", "age_days": 2, "content": "B"}]
        self.assertEqual(audit({"turns": turns})[0]["type"], "duplicate_without_equal_value")
        turns[0]["value"] = "B"
        self.assertEqual(audit({"turns": turns}), [])


if __name__ == "__main__":
    unittest.main()
