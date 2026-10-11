#!/usr/bin/env python3
"""Build the LoCoMo retrieval corpus for LingShu's long-horizon memory benchmark.

Why this file exists
--------------------
This experiment evaluates retrieval on a *public* long-horizon memory dataset.
LoCoMo (Snap Research, 10 multi-session dialogues / 1,986 QA pairs with gold
evidence ids) fits, but shipped as raw chat logs. This script turns it into the
deterministic, auditable corpus the Rust evaluation harness consumes.

Design rules (kept explicit so the numbers can be defended):
  * Memory unit = one utterance (`dia_id`), which is exactly the granularity of
    LoCoMo's gold evidence. No re-ranking shortcut, no leaked labels.
  * `semantic` embeddings come from a real encoder
    (`paraphrase-multilingual-MiniLM-L12-v2` ONNX, 384d), pre-computed offline
    so the Rust run is deterministic and needs no runtime model download.
  * `importance` / `trust` are derived from the text itself with documented
    heuristics. `access_count` is pinned to 0 because LoCoMo ships no access
    log -- claiming otherwise would be fabrication.

Outputs (datasets/public/):
  locomo_corpus.json   metadata, human readable, the audit surface
  locomo_emb.f32       float32 matrix [n_rows, 384] (memories then queries)
"""

from __future__ import annotations

import json
import math
import re
import struct
from datetime import datetime, timedelta
from pathlib import Path

import numpy as np
import onnxruntime as ort
from tokenizers import Tokenizer

ROOT = Path(__file__).resolve().parent.parent
PUBLIC = ROOT / "datasets" / "public"
SRC = PUBLIC / "locomo10.json"

# One encoder for every corpus, Chinese and English alike. Mixing backends
# across datasets would make the numbers incomparable.
MODEL_DIR = PUBLIC / "_model_multi"
MODEL_ID = "sentence-transformers/paraphrase-multilingual-MiniLM-L12-v2"

OUT_JSON = PUBLIC / "locomo_corpus.json"
OUT_BIN = PUBLIC / "locomo_emb.f32"

MAX_LEN = 256
DIM = 384

# Hedging language lowers asserted-truth confidence. First-person plain
# statements keep the base rate.
HEDGES = [
    "maybe", "might", "perhaps", "probably", "possibly", "i think", "i guess",
    "i wonder", "not sure", "i'm not sure", "kind of", "sort of", "somewhat",
    "could be", "seems like", "i believe", "i suppose", "hopefully", "wondering",
    "if i recall", "i vaguely", "or something", "i dunno", "i don't know",
]
FIRST_PERSON = [" i ", " my ", " me ", " we ", " our ", " myself "]

STOP = set(
    """a an the and or but if then than that this these those is are was were be been being
    am do does did doing have has had having i you he she it we they me him her them my your
    his its our their mine yours hers ours theirs of in on at to for with from by as into
    about over after before under above out up down off again once here there all any both
    each few more most other some such no nor not only own same so too very s t can will just
    don should now d ll m o re ve y ain aren couldn didn doesn hadn hasn haven isn ma mightn
    mustn needn shan shouldn wasn weren won wouldn what which who whom whose when where why how
    because while during until unless since though although get got go going went come came
    said say says like know think really well yeah yes okay ok oh hey hi thanks thank""".split()
)

TOKEN_RE = re.compile(r"[a-z0-9']+")


def tokenize_words(text: str) -> list[str]:
    return [t for t in TOKEN_RE.findall(text.lower()) if t not in STOP and len(t) > 1]


def parse_session_time(raw: str) -> datetime:
    """'1:56 pm on 8 May, 2023' -> datetime. LoCoMo uses one fixed format."""
    return datetime.strptime(raw.strip(), "%I:%M %p on %d %B, %Y")


def derive_importance(tokens: list[str], idf: dict[str, float]) -> float:
    """Information-density proxy: rare, content-bearing words carry more weight.

    Sum of idf over the utterance's rarest terms, normalised by length so long
    rambling turns do not automatically win.
    """
    if not tokens:
        return 0.30
    weights = sorted((idf.get(t, 0.0) for t in tokens), reverse=True)
    top = weights[:8]
    density = sum(top) / math.sqrt(len(tokens))
    return round(min(0.95, max(0.25, 0.30 + 0.42 * (1.0 - math.exp(-density)))), 4)


def derive_trust(text: str) -> float:
    """Assertion-confidence proxy: hedged speech is a weaker memory source."""
    low = " " + text.lower() + " "
    trust = 0.72
    if any(h in low for h in HEDGES):
        trust -= 0.18
    if any(p in low for p in FIRST_PERSON):
        trust += 0.10
    if "?" in text:
        trust -= 0.12  # a question is not an asserted fact
    if re.search(r"\b\d{1,2}(:\d{2})?\s?(am|pm)\b|\b(19|20)\d{2}\b|\b\d+\s?(years?|months?|weeks?|days?)\b", low):
        trust += 0.08  # concrete temporal/numeric anchors are more reliable
    return round(min(0.98, max(0.20, trust)), 4)


class Encoder:
    """MiniLM-L6-v2 ONNX encoder: mean pooling + L2 normalise, matching
    sentence-transformers' default pooling for this checkpoint."""

    def __init__(self) -> None:
        self.tokenizer = Tokenizer.from_file(str(MODEL_DIR / "tokenizer.json"))
        self.tokenizer.enable_truncation(max_length=MAX_LEN)
        self.tokenizer.enable_padding(length=MAX_LEN, pad_id=0, pad_token="[PAD]")
        self.session = ort.InferenceSession(
            str(MODEL_DIR / "model.onnx"), providers=["CPUExecutionProvider"]
        )
        # XLM-R style checkpoints ignore token_type_ids; feed only what the graph asks for.
        self.input_names = {i.name for i in self.session.get_inputs()}

    def encode(self, texts: list[str], batch: int = 32) -> np.ndarray:
        out = np.zeros((len(texts), DIM), dtype=np.float32)
        for start in range(0, len(texts), batch):
            chunk = texts[start : start + batch]
            encoded = self.tokenizer.encode_batch(chunk)
            ids = np.array([e.ids for e in encoded], dtype=np.int64)
            mask = np.array([e.attention_mask for e in encoded], dtype=np.int64)
            feed = {"input_ids": ids, "attention_mask": mask}
            if "token_type_ids" in self.input_names:
                feed["token_type_ids"] = np.zeros_like(ids, dtype=np.int64)
            hidden = self.session.run(["last_hidden_state"], feed)[0]
            m = mask[:, :, None].astype(np.float32)
            pooled = (hidden * m).sum(axis=1) / np.maximum(m.sum(axis=1), 1e-9)
            norm = np.linalg.norm(pooled, axis=1, keepdims=True)
            out[start : start + len(chunk)] = (pooled / np.maximum(norm, 1e-9)).astype(np.float32)
        return out


def session_keys(conv: dict) -> list[tuple[int, str]]:
    pairs = []
    for key in conv:
        m = re.fullmatch(r"session_(\d+)", key)
        if m and isinstance(conv[key], list):
            pairs.append((int(m.group(1)), key))
    return sorted(pairs)


def build() -> None:
    raw = json.loads(SRC.read_text(encoding="utf-8"))
    encoder = Encoder()

    cases: list[dict] = []
    memory_texts: list[str] = []
    memory_rows: list[tuple[str, int]] = []  # (case_id, index inside case)

    for sample in raw:
        conv = sample["conversation"]
        pairs = session_keys(conv)
        if not pairs:
            continue

        # Pass 1: collect utterances with resolved timestamps.
        utterances: list[dict] = []
        for session_no, key in pairs:
            stamp_raw = conv.get(f"{key}_date_time")
            base = parse_session_time(stamp_raw) if stamp_raw else None
            for offset, utt in enumerate(conv[key]):
                text = (utt.get("text") or "").strip()
                caption = (utt.get("blip_caption") or "").strip()
                if not text and not caption:
                    continue
                occurred = base + timedelta(minutes=offset) if base else None
                utterances.append(
                    {
                        "id": utt["dia_id"],
                        "text": text,
                        "caption": caption,
                        "speaker": utt.get("speaker", "unknown"),
                        "session": session_no,
                        "turn": offset + 1,
                        "occurred_at": occurred.isoformat() + "Z" if occurred else None,
                    }
                )

        if not utterances:
            continue

        # Pass 2: idf inside this case (each dialogue is one memory store).
        docs = [tokenize_words(u["text"] or u["caption"]) for u in utterances]
        n_docs = len(docs)
        df: dict[str, int] = {}
        for toks in docs:
            for term in set(toks):
                df[term] = df.get(term, 0) + 1
        idf = {t: math.log((n_docs + 1) / (c + 1)) + 1.0 for t, c in df.items()}

        memories = []
        for idx, (utt, toks) in enumerate(zip(utterances, docs)):
            content = utt["text"] or utt["caption"]
            if utt["text"] and utt["caption"]:
                content = f"{utt['text']} [image: {utt['caption']}]"
            importance = derive_importance(toks, idf)
            trust = derive_trust(utt["text"] or utt["caption"])
            # A question is never a storable fact, but LoCoMo evidence can point
            # at one, so we keep it and let trust carry the penalty.
            memories.append(
                {
                    "row": len(memory_texts),
                    "id": utt["id"],
                    "content": content,
                    "speaker": utt["speaker"],
                    "session": utt["session"],
                    "turn": utt["turn"],
                    "occurred_at": utt["occurred_at"],
                    "importance": importance,
                    "trust": trust,
                    "access_count": 0,  # LoCoMo ships no access log
                    "state": "active",
                }
            )
            memory_texts.append(content)
            memory_rows.append((sample["sample_id"], idx))

        # Pass 3: QA pairs. Skip items without gold evidence -- unscoreable.
        known = {m["id"] for m in memories}
        queries = []
        for qa in sample["qa"]:
            gold = [e for e in (qa.get("evidence") or []) if e in known]
            if not gold:
                continue
            queries.append(
                {
                    "row": None,  # filled after encoding
                    "query": qa["question"],
                    # LoCoMo stores adversarial answers under a separate key.
                    "answer": str(qa.get("answer") or qa.get("adversarial_answer") or ""),
                    "expected": gold,
                    "category": qa.get("category", 0),
                }
            )
        if not queries:
            continue

        cases.append(
            {
                "case_id": sample["sample_id"],
                "speakers": [conv.get("speaker_a"), conv.get("speaker_b")],
                "session_count": len(pairs),
                "turn_count": len(memories),
                "memories": memories,
                "queries": queries,
            }
        )

    # Encode memories then queries, row-major, so the Rust side can mmap once.
    query_texts: list[str] = []
    for case in cases:
        for q in case["queries"]:
            query_texts.append(q["query"])

    print(f"encoding {len(memory_texts)} memories + {len(query_texts)} queries ...")
    mem_vec = encoder.encode(memory_texts)
    qry_vec = encoder.encode(query_texts)

    cursor = len(memory_texts)
    for case in cases:
        for q in case["queries"]:
            q["row"] = cursor
            cursor += 1

    corpus = {
        "name": "LoCoMo-Retrieval-v1",
        "source": "snap-research/locomo:data/locomo10.json (CC BY-NC 4.0)",
        "source_url": "https://github.com/snap-research/locomo",
        "embedding_model": f"{MODEL_ID} (ONNX, mean-pooling, L2-normalized, 384d)",
        "feature_policy": {
            "semantic": "MiniLM cosine, computed offline, frozen in locomo_emb.f32",
            "lexical": "character bigram Jaccard (lingshu lexical_similarity)",
            "recency": "real session timestamps parsed from LoCoMo date_time fields",
            "trust": "assertion-confidence heuristic over hedging/first-person/temporal anchors",
            "importance": "idf information-density heuristic computed per dialogue",
            "frequency": "DISABLED on this dataset: LoCoMo ships no access log, access_count pinned to 0",
            "state": "all active: LoCoMo ships no conflict/expiry labels; governance is measured on the in-house set",
        },
        "category_legend": {
            "1": "multi-hop",
            "2": "temporal reasoning",
            "3": "open-domain knowledge",
            "4": "single-hop",
            "5": "adversarial",
        },
        "row_layout": {
            "dim": DIM,
            "memory_rows": f"[0, {len(memory_texts)})",
            "query_rows": f"[{len(memory_texts)}, {len(memory_texts) + len(query_texts)})",
            "total_rows": len(memory_texts) + len(query_texts),
        },
        "cases": cases,
    }

    out_json = PUBLIC / "locomo_corpus.json"
    out_bin = PUBLIC / "locomo_emb.f32"
    out_json.write_text(
        json.dumps(corpus, ensure_ascii=False, indent=1), encoding="utf-8"
    )
    with out_bin.open("wb") as fh:
        fh.write(struct.pack("<II", len(memory_texts) + len(query_texts), DIM))
        fh.write(mem_vec.astype("<f4").tobytes())
        fh.write(qry_vec.astype("<f4").tobytes())

    total_q = sum(len(c["queries"]) for c in cases)
    total_m = sum(len(c["memories"]) for c in cases)
    print(f"cases={len(cases)} memories={total_m} queries={total_q}")
    print(f"wrote {out_json} ({out_json.stat().st_size/1e6:.1f} MB)")
    print(f"wrote {out_bin} ({out_bin.stat().st_size/1e6:.1f} MB)")


if __name__ == "__main__":
    build()
