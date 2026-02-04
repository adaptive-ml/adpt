# adpt CLI - Guide for Claude

This document helps Claude instances assist users with the Adaptive platform CLI.

## Overview

`adpt` is a Rust CLI for interacting with the Adaptive ML platform. It handles:
- Dataset uploads
- Recipe publishing (single and multi-file)
- Job execution and monitoring
- Model listing

## Installation

```bash
cd /path/to/adpt
cargo install --path .
```

The binary installs to `~/.cargo/bin/adpt`.

## Configuration

API keys are stored in the OS keyring:
```bash
adpt set-api-key <your-key>
```

Config file location: `~/.adpt/config.toml`
```toml
adaptive_base_url = "https://prod.tail19cfc3.ts.net"
default_use_case = "your-use-case"
```

Environment variables (override config):
- `ADAPTIVE_BASE_URL`
- `ADAPTIVE_API_KEY`

## Commands Reference

### Dataset Management

**Upload a dataset:**
```bash
adpt upload /path/to/data.jsonl -n dataset-name
# Returns: Dataset ID and key
```

### Recipe Management

**List recipes:**
```bash
adpt recipes
```

**Publish a recipe (reads from pyproject.toml):**
```bash
adpt publish <recipe-name>      # Single recipe by name
adpt publish --all              # All recipes in pyproject.toml
adpt publish --list             # Show available recipes
```

The publish command:
1. Reads `[tool.adaptive.recipes.<name>]` from pyproject.toml
2. Copies the recipe file to `main.py`
3. Filters files based on `ignore-files` and `ignore-extensions`
4. Removes `[tool.uv]` section (confuses server-side uv)
5. Zips and uploads

**View recipe schema:**
```bash
adpt schema <recipe-key>
```

### Job Execution

**Run a recipe:**
```bash
# With CLI arguments
adpt run <recipe-key> -g <num-gpus> -c <cluster> --arg1 value1 --arg2 value2

# With params file (recommended for complex configs)
adpt run <recipe-key> -g 8 -c default -p /path/to/params.json
```

**Monitor a job:**
```bash
adpt job <job-id>
```

**List running jobs:**
```bash
adpt jobs
```

**Cancel a job:**
```bash
adpt cancel <job-id>
```

### Models

**List available models:**
```bash
adpt models
```

## pyproject.toml Configuration

Recipes are configured under `[tool.adaptive]`:

```toml
[tool.adaptive]
use-case = "my-use-case"
ignore-files = [
    "__pycache__",
    ".venv",
    ".git",
    "data",        # Important: exclude large data directories!
]
ignore-extensions = [".pyc", ".ipynb"]

[tool.adaptive.recipes.eval]
recipe-path = "src/myproject/recipes/eval.py"
recipe-key = "my-eval"
use-case = "my-use-case"

[tool.adaptive.recipes.train]
recipe-path = "src/myproject/recipes/train.py"
recipe-key = "my-train"
use-case = "my-use-case"
```

## Case Study: Seoul AI Safety Evaluation

This walkthrough shows a complete workflow from dataset upload to evaluation.

### 1. Upload Datasets

```bash
# Upload test dataset
adpt upload data/handcrafted_data/test.jsonl -n seoul-test-data
# Output: Dataset uploaded successfully with ID: ..., key: seoul-test-data

# Upload training dataset
adpt upload data/handcrafted_data/train.jsonl -n seoul-train-data
# Output: Dataset uploaded successfully with ID: ..., key: seoul-train-data
```

### 2. Publish Recipe

```bash
# Check available recipes
adpt publish --list
# Output: eval, sft, rl

# Publish the eval recipe
adpt publish eval
# Output: Recipe published successfully with ID: ..., key: seoul-eval-1
```

### 3. Create Parameters File

For complex configurations, use a JSON params file:

```json
{
  "model": "Qwen3-30B-A3B-Instruct-2507",
  "run_name": "seoul-eval-test",
  "dataset": "seoul-test-data"
}
```

Note: The `Dataset` parameter type accepts just the dataset key as a string.

### 4. Run the Recipe

```bash
adpt run seoul-eval-1 -g 8 -c default -p /path/to/params.json
# Output: Recipe run successfully with ID: 019c2a54-c1a2-7531-bc7c-86a09603ae9d
```

### 5. Monitor Job Progress

```bash
adpt job 019c2a54-c1a2-7531-bc7c-86a09603ae9d
```

Output shows stage progress:
```
┌ Custom Recipe Job - 2026-02-04
│
◆ Loading Model
│ 1/1
│
◆ Testing
│ 120/120
│
└ Completed
```

### 6. Access Logs (via kubectl)

For debugging, access server logs:

```bash
# Get pod names
kubectl get pods | grep -E "sandkasten|controlplane"

# Check sandkasten logs (recipe execution)
kubectl logs <sandkasten-pod> --tail=100 | grep <job-id>

# Check controlplane logs (job orchestration)
kubectl logs <controlplane-pod> --tail=100 | grep <job-id>
```

### 7. Access Artifacts

Job artifacts are stored on the server:

```bash
# List artifacts
kubectl exec <sandkasten-pod> -- ls /workdir/job/<job-id>/artifacts/

# Read artifact content
kubectl exec <sandkasten-pod> -- cat /workdir/job/<job-id>/artifacts/<file>.jsonl
```

## Common Issues & Solutions


### Recipe Upload Too Large

**Cause:** Data directories or large files being included.

**Solution:** Add directories to `ignore-files` in pyproject.toml:
```toml
ignore-files = ["data", ".venv", "__pycache__"]
```

### "NotEnoughMemory" During Training

**Cause:** Batch size too large for available GPU memory.

**Solution:** Reduce `batch_size_tokens`, or increase TP

### Dataset Parameter Type Mismatch

**Cause:** Passing JSON object `{"dataset_key": "..."}` instead of string.

**Solution:** Pass just the dataset key as a string: `"dataset": "seoul-test-data"`

## Recipe Development Tips

### Model Spawning

Use `ctx.client.model()` with the model registry prefix:

```python
model = await (
    ctx.client.model(
        f"model_registry://{config.model.model_key}",
        kv_cache_len=config.kv_cache,
        tokens_to_generate=config.max_seq_len,
    )
    .tp(config.tp)
    .spawn_inference("model_name")
)
```

### Loading Server-side Datasets

```python
from adaptive_harmony.parameters import Dataset

class MyConfig(InputConfig):
    dataset: Annotated[Dataset | None, Field(description="Dataset")] = None

# In main():
if config.dataset is not None:
    data = await config.dataset.load(ctx)
```
