"""LoCoMo category-aware scoring, matched to upstream task_eval/evaluation.py.

Source: https://github.com/snap-research/locomo/blob/main/task_eval/evaluation.py
Checked 2026-09-06. Category 5 scores abstention, never adversarial_answer.
"""
import re
import string
from collections import Counter
from nltk.stem import PorterStemmer

STEMMER = PorterStemmer()


def normalize(text):
    text = str(text).lower().replace(",", "")
    text = "".join(c for c in text if c not in string.punctuation)
    return " ".join(re.sub(r"\b(a|an|the|and)\b", " ", text).split())


def token_f1(prediction, reference):
    pred = [STEMMER.stem(w) for w in normalize(prediction).split()]
    gold = [STEMMER.stem(w) for w in normalize(reference).split()]
    overlap = sum((Counter(pred) & Counter(gold)).values())
    return 2 * overlap / (len(pred) + len(gold)) if overlap else 0.0


def score(prediction, reference, category):
    if category == 5:
        return float(any(s in prediction.lower() for s in ("no information available", "not mentioned")))
    if category == 1:
        gold = str(reference).split(",")
        return sum(max(token_f1(p, g) for p in prediction.split(",")) for g in gold) / len(gold)
    if category == 3:
        reference = str(reference).split(";")[0].strip()
    if category not in (2, 3, 4):
        raise ValueError(f"Unknown category: {category}")
    return token_f1(prediction, reference)
