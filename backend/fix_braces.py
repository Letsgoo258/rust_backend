import re

with open("/Users/abhiram/Documents/eraya/backend/src/modules/academic/repository.rs", "r") as f:
    lines = f.readlines()

new_lines = []
in_struct = False
struct_indent = ""

for i, line in enumerate(lines):
    new_lines.append(line)

    if line.strip().startswith("pub struct ") and line.strip().endswith("{"):
        in_struct = True
        struct_indent = line[:line.find("pub")]

    if in_struct and not line.strip() and i + 1 < len(lines):
        next_line = lines[i+1].strip()
        if next_line.startswith("#[derive") or next_line.startswith("pub struct") or next_line.startswith("impl"):
            new_lines.insert(-1, struct_indent + "}\n")
            in_struct = False

with open("/Users/abhiram/Documents/eraya/backend/src/modules/academic/repository.rs", "w") as f:
    f.writelines(new_lines)
