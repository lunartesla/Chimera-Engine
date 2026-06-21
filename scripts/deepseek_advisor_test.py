"""
deepseek_advisor_test.py — exploratory test, NOT production.

Goal: see what DeepSeek-R1-Distill-Qwen-1.5B actually produces when given
read/write tool access to reason about one engine source file, before we
design any real JSON schema or wire it into the daemon.

Assumes GPT4All's local server is running with the model loaded, exposing
the OpenAI-compatible endpoint at http://localhost:4891/v1/chat/completions.

Run:  python deepseek_advisor_test.py
"""
import json
import re
import argparse
from pathlib import Path
import requests

ENDPOINT = "https://openrouter.ai/api/v1/chat/completions"
MODEL_NAME = "nex-agi/nex-n2-pro:free"
OPENROUTER_API_KEY = "PASTE_YOUR_KEY_HERE"
MAX_TURNS = 12

SYSTEM_PROMPT = """You are a code-reasoning assistant analyzing a Rust evolutionary \
optimization engine called Chimera. Your job: read the target file, reason about \
what it does in plain language, and produce hyper-detailed machine-usable hints \
for a NEAT neural network that mutates optimization pass pipelines.

You have exactly two tools. To use one, respond with ONLY a JSON object, nothing else:
  {"tool": "read_file", "path": "<relative path>"}
  {"tool": "write_file", "path": "<relative path>", "content": "<file content>"}

When you are completely done, call write_file with your final findings as a JSON
string containing at least: "reasoning_summary" (plain language), "recommended_pipeline"
(list of pass names), "pass_priority_weights" (object mapping pass name -> 0.0-1.0),
"new_ir_requests" (list of strings — primitives the IR is missing, if any), then
respond with exactly: {"tool": "done"}

Think step by step, but keep tool-call responses as pure JSON with no other text.
"""

def strip_think_blocks(text: str) -> str:
    """DeepSeek-R1 models emit <think>...</think> reasoning traces before the
    actual answer — strip them before trying to parse a tool call out of it."""
    return re.sub(r"<think>.*?</think>", "", text, flags=re.DOTALL).strip()

def extract_json_obj(text: str):
    """Model may wrap JSON in prose/code fences despite instructions — find the
    first {...} block and try to parse it."""
    text = strip_think_blocks(text)
    match = re.search(r"\{.*\}", text, re.DOTALL)
    if not match:
        return None, text
    try:
        return json.loads(match.group(0)), text
    except json.JSONDecodeError:
        return None, text

def call_model(messages):
    resp = requests.post(
        ENDPOINT,
        headers={"Authorization": f"Bearer {OPENROUTER_API_KEY}"},
        json={
            "model": MODEL_NAME,
            "messages": messages,
            "temperature": 0.3,
            "max_tokens": 4096,
        },
        timeout=600,
    )
    resp.raise_for_status()
    return resp.json()["choices"][0]["message"]["content"]

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--file", default="src/self_evolving_engine.rs")
    parser.add_argument("--output", default="scripts/deepseek_findings.json")
    parser.add_argument("--max-turns", type=int, default=MAX_TURNS)
    args = parser.parse_args()

    base = Path(__file__).resolve().parent.parent
    messages = [
        {"role": "system", "content": SYSTEM_PROMPT},
        {"role": "user", "content": f"Start by reading {args.file}"},
    ]
    nudge_count = 0

    for turn in range(args.max_turns):
        print(f"\n{'='*60}\nTURN {turn+1}\n{'='*60}")
        raw = call_model(messages)
        print(f"[RAW MODEL OUTPUT]\n{raw}\n")

        obj, cleaned = extract_json_obj(raw)
        messages.append({"role": "assistant", "content": cleaned})

        if obj is None:
            nudge_count += 1
            if nudge_count >= 2:
                print("[!] Model ignored the nudge twice. Stopping.")
                break
            print("[!] No tool call found — nudging model back toward write_file.")
            messages.append({"role": "user", "content": (
                "That's a good summary, but you haven't called write_file yet. "
                "Respond with ONLY this JSON now: "
                '{"tool": "write_file", "path": "scripts/deepseek_findings.json", '
                '"content": "<your findings as a JSON string with reasoning_summary, '
                'recommended_pipeline, pass_priority_weights, new_ir_requests>"}'
            )})
            continue

        tool = obj.get("tool")

        if tool == "read_file":
            target = base / obj["path"]
            try:
                content = target.read_text(encoding="utf-8")
                print(f"[TOOL] read_file({obj['path']}) -> {len(content)} chars")
                messages.append({"role": "user", "content": f"FILE CONTENT:\n{content}"})
            except Exception as e:
                print(f"[TOOL ERROR] {e}")
                messages.append({"role": "user", "content": f"ERROR reading file: {e}"})

        elif tool == "write_file":
            target = base / obj["path"]
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text(obj["content"], encoding="utf-8")
            print(f"[TOOL] write_file({obj['path']}) -> wrote {len(obj['content'])} chars")
            messages.append({"role": "user", "content": "File written successfully."})

        elif tool == "done":
            print("[DONE] Model signaled completion.")
            break

        else:
            print(f"[!] Unknown tool: {tool}. Stopping.")
            break

    print(f"\n{'='*60}\nCheck {args.output} for the model's findings (if it wrote one).\n{'='*60}")

if __name__ == "__main__":
    main()
