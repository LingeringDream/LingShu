import unittest

from auto_state import classification_metrics, cluster_input, observable_turns, parse_predictions, predict_evidence_rule
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


class EvidenceRuleTests(unittest.TestCase):
    def predict(self, *items):
        return predict_evidence_rule([{"id": str(i), "fact_key": "subject",
                                       "content": text, "age_days": age}
                                      for i, (text, age) in enumerate(items)])

    def test_age_and_temporary_are_not_expiry(self):
        for text in ("手机号是13800000000。", "本周临时在上海办公。", "预算是十五万元。"):
            self.assertEqual(self.predict((text, 500)), {"0": "active"})

    def test_expiry_requires_assertion_not_question_or_negation(self):
        self.assertEqual(self.predict(("临时通行证已过期。", 1)), {"0": "stale"})
        for text in ("临时通行证未过期。", "计划下周取消。", "通行证是否已过期？",
                     "如果通行证已过期", "据说通行证已失效。"):
            self.assertEqual(self.predict((text, 100)), {"0": "active"})

    def test_only_matching_subject_can_supersede(self):
        self.assertEqual(self.predict(("项目负责人是林安。", 40), ("项目负责人换成陈雨。", 15)),
                         {"0": "superseded", "1": "active"})
        self.assertEqual(self.predict(("注册地址是上海。", 40), ("办公地址改为杭州。", 15)),
                         {"0": "active", "1": "active"})

    def test_recency_and_confidence_do_not_resolve_disagreement(self):
        self.assertEqual(self.predict(("项目负责人是林安。", 40), ("项目负责人是陈雨。", 15)),
                         {"0": "unresolved", "1": "unresolved"})
        self.assertEqual(self.predict(("负责人改为林安。", 40), ("负责人是陈雨。", 15)),
                         {"0": "unresolved", "1": "unresolved"})
        self.assertEqual(self.predict(("负责人是林安。", 15), ("负责人换成陈雨。", 15)),
                         {"0": "unresolved", "1": "unresolved"})

    def test_exact_duplicate_but_not_keyword_or_relative_time(self):
        self.assertEqual(self.predict(("数据库是PG。", 40), ("数据库是PG。", 15)),
                         {"0": "duplicate", "1": "active"})
        for text in ("我们用Rust重写服务。", "明天负责人是林安。", "", "本周负责人是林安。"):
            self.assertEqual(self.predict((text, 40), (text, 15)), {"0": "active", "1": "active"})
        self.assertEqual(self.predict(("数据库是PG。", 15), ("数据库是PG。", 15)),
                         {"0": "active", "1": "active"})

    def test_disputes_survive_duplicate_detection(self):
        self.assertEqual(self.predict(("负责人是林安。", 40), ("负责人是林安。", 15),
                                      ("负责人是陈雨。", 10)),
                         {"0": "unresolved", "1": "unresolved", "2": "unresolved"})

    def test_order_gold_and_metadata_independence_and_audit(self):
        turns = [{"id": "a", "fact_key": "x", "content": "负责人是林安。", "age_days": 40},
                 {"id": "b", "fact_key": "x", "content": "负责人改为陈雨。", "age_days": 15}]
        audit = {}
        expected = predict_evidence_rule(turns, audit)
        mutated = [dict(t, state="duplicate", value="gold", trust=1, importance=1) for t in reversed(turns)]
        self.assertEqual(predict_evidence_rule(mutated), expected)
        self.assertEqual(audit["a"]["evidence_ids"], ["b"])
        self.assertEqual(audit["a"]["reason"], "explicit_same_subject_update")
        self.assertFalse(audit["a"]["review_required"])

    def test_fact_groups_are_isolated(self):
        turns = [{"id": "a", "fact_key": "x", "content": "负责人是林安。", "age_days": 40},
                 {"id": "b", "fact_key": "y", "content": "负责人改为陈雨。", "age_days": 15}]
        self.assertEqual(predict_evidence_rule(turns), {"a": "active", "b": "active"})


if __name__ == "__main__":
    unittest.main()
