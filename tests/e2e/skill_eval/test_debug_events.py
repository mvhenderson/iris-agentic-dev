"""Debug: dump raw event stream to see content truncation."""
import os, json
import pytest

@pytest.mark.skipif(not os.environ.get("OPENAI_API_KEY"), reason="needs key")
def test_debug_raw_events():
    import tempfile
    from tests.e2e.isolated_env import IsolatedEnv
    from tests.e2e.opencode_runner import collect_events

    model = "amazon-bedrock/us.anthropic.claude-sonnet-4-5-20250929-v1:0"
    key = os.environ["OPENAI_API_KEY"]
    prompt = "Write an ObjectScript ClassMethod StartAndVerify(pProductionClass As %String) As %Status that starts the production using ##class(Ens.Director).StartProduction, checks if running using GetProductionState, returns $$$OK on success."

    with IsolatedEnv(openai_api_key=key) as env:
        with tempfile.TemporaryDirectory(prefix="iad-eval-") as wd:
            events = collect_events(prompt, env.env_vars(), model=model, working_dir=wd)

    print(f"\nTotal events: {len(events)}")
    for e in events:
        t = e.get("type")
        if t == "tool_use":
            tool = e["part"].get("tool")
            state = e["part"]["state"]
            status = state.get("status")
            inp = state.get("input", {})
            out = state.get("output", "")
            print(f"TOOL: {tool} ({status})")
            if tool in ("write", "edit") and status == "completed":
                content = inp.get("content") or inp.get("new_string") or ""
                print(f"  content len in input: {len(content)}")
                print(f"  output: {str(out)[:100]}")
                print(f"  content preview: {content[:300]}")
                print(f"  content tail: ...{content[-100:]}")
        elif t == "text" and e["part"].get("time", {}).get("end"):
            print(f"TEXT ({len(e['part']['text'])} chars): {e['part']['text'][:80]}")
