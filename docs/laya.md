# Laya boundary and continuous learning

The Rust engine uses Laya for one bounded decision: `search` or `graph` context.
It does not ask Laya to generate answers, invent graph edges, determine source
truth, or produce its own correctness labels. The fallback is always available.

## Implemented protocol

`context --laya-port PORT --confidence 0.8` posts a structured choice request to
`http://127.0.0.1:PORT/v1/systemone`. The request contains the query as `state` and
one `questions.strategy` choice with criteria for `search` and `graph`.
The response must contain:

```json
{"answers":{"strategy":{"type":"choice","choice":"graph","answer_confidence":0.93}}}
```

The client uses Laya's `answer_confidence`, not its distinct entropy-derived
`confidence` field. The threshold is policy, not proof that a new domain's model
has been calibrated. The timeout is two seconds, the response cap is 64 KiB,
redirects and environment proxies are disabled, and the destination is loopback.
Only the explicit flag makes this network request. No API-key authentication is
implemented yet; use a separately managed loopback-only server on a trusted host.

This wire shape has been exercised against local HTTP fixtures. It has **not**
been exercised against a running Laya model in this project, so neither model
accuracy nor inference latency is established here.

## Feedback and export

```sh
printf '%s\n' '{"task_id":"example-task-1","query":"who calls parse_record","correct_strategy":"graph","label_source":"operator","allow_training":true}' |
  foundry --store /tmp/foundry-demo feedback
foundry --store /tmp/foundry-demo export-training > /tmp/foundry-training.jsonl
```

The accepted label sources are `operator` and `task_checker`. They describe the
caller's assertion; the CLI cannot certify the checker. Keep examples local until
the owner has explicitly authorized their use. Never treat a selected strategy,
click, or model output as a correct-strategy label automatically.

The key is the task/query pair. Submitting the same pair with a corrected label
replaces it. Submitting it with `allow_training:false` excludes it from subsequent
exports. This does not erase previously exported datasets or untrain old weights.

Exports use recipe `retrieval-strategy-v1` and include `id`, `task_id`, `state`,
`correct_strategy`, `label_source`, and `split`. A deterministic task-id hash assigns
roughly 80/10/10 train/calibration/evaluation buckets. All examples of one task stay
in one bucket. Callers must use the same task id for related interactions; this is
not a guarantee against duplicate tasks given different ids or repository leakage.
Small datasets can have empty partitions and must not train/promote under that split.

## Next integration: bounded repeated training rounds

This is the handoff contract, **not an implemented training scheduler**:

1. Freeze opted-in rows in a dataset directory outside the source checkout. Write
   hashes, recipe, task groups, split counts, source rights/consent and Laya revision
   into a manifest. Record previously exported rows whose consent has changed.
2. Convert each row into Laya's existing choice-question training representation
   using its tokenizer and `build_sequence`. Do not feed this JSONL directly to a
   notebook and claim it is already tokenized training data.
3. Train outside serving using the existing Laya/PyTorch workflow, with a fixed
   time/resource budget and replay samples drawn only from earlier training splits.
   Keep repeated tasks/repositories out of calibration and evaluation as appropriate
   to the claimed generalization. No source execution or model download occurs in
   the engine's indexing path.
4. Calibrate on the calibration split. On held-out tasks, compare the candidate
   against deterministic routing and the incumbent: task correctness, action
   accuracy, abstention, routing time, complete agent input and actual provider
   usage when available. Reject an empty/contaminated evaluation or a quality
   regression. Token reduction alone does not authorize promotion.
5. Load an explicitly selected, hash-identified local candidate in a separate
   process, health-check its recipe and identity, then change the serving selection.
   Retain the incumbent and make rollback a single selection change. New training
   never modifies serving weights in place. Subsequent rounds repeat this flow.

The upstream `laya-serve` resolver supports named packaged checkpoints; an arbitrary
local path in a request's `model` field does **not** select a trained checkpoint.
Candidate serving therefore needs a small external adapter that explicitly constructs
`Agent(model_id_or_path=<local checkpoint>)`, verifies the artifact identity, and
exposes the same protocol plus model/recipe health metadata. This adapter and
candidate promotion are not included in the first slice.

Laya remains its existing Python/PyTorch runtime. All Context Foundry application
code is Rust; this integration does not attempt to rewrite Laya in Rust. There are
no GPU jobs, new trained weights, auto-promotion or live-model claims in this release.

References: upstream [Agent](https://github.com/NandhaKishorM/laya/blob/main/laya/agent.py),
[server](https://github.com/NandhaKishorM/laya/blob/main/laya/serve.py), and
[training guide](https://github.com/NandhaKishorM/laya#fine-tuning).
