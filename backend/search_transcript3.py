import json

with open("/Users/abhiram/.gemini/antigravity/brain/0df7cf86-fc9d-4681-9a79-f4ea3ff0dfa9/.system_generated/logs/transcript_full.jsonl", "r") as f:
    for line in f:
        data = json.loads(line)
        if "tool_calls" in data:
            for tc in data["tool_calls"]:
                if tc["name"] == "write_to_file":
                    args = tc.get("args", {})
                    if "academic/repository.rs" in str(args.get("TargetFile", "")):
                        last_content = args.get("CodeContent", "")
                        with open("recovered_academic_repository.rs", "w") as out:
                            out.write(last_content)
