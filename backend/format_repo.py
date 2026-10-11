import re

with open("/Users/abhiram/Documents/eraya/backend/src/modules/academic/repository.rs", "r") as f:
    lines = [l.rstrip() for l in f.readlines()]

# First, strip all lines that are just "}" (to start fresh)
clean_lines = []
for line in lines:
    if line.strip() == "}":
        continue
    if line.strip():
        clean_lines.append(line)

new_lines = []
in_struct = False
in_impl = False
in_fn = False

for i, line in enumerate(clean_lines):
    new_lines.append(line)

    # Check if we opened a struct
    if line.startswith("pub struct ") and line.endswith("{"):
        in_struct = True

    if in_struct:
        # If the next line is not a field (doesn't start with space/tab or is another struct/impl)
        if i + 1 < len(clean_lines):
            next_line = clean_lines[i+1]
            if next_line.startswith("#[") or next_line.startswith("pub struct") or next_line.startswith("impl ") or next_line.startswith("pub async fn"):
                new_lines.append("}")
                in_struct = False
        else:
            new_lines.append("}")
            in_struct = False

    # Check if we opened impl
    if line.startswith("impl "):
        in_impl = True

    # Check if we are inside a function
    if "pub async fn" in line:
        # if we were already in a function, we should have closed it before this line!
        # wait, we can't do it here because new_lines.append(line) already happened.
        pass

# Now handle functions. Functions end before another function starts, or before EOF.
final_lines = []
for i, line in enumerate(new_lines):
    if "pub async fn" in line and i > 0:
        # Check if we need to close the previous function
        # The previous function must be closed BEFORE the #[allow] or pub async fn
        # Let's find the last non-empty line before this fn or its attributes
        pass

# Actually, an easier way:
# We know functions end with Ok(...) or Ok(()) or Err(...)
# Let's just find the `    Ok(...)` that is indented with 4 or 8 spaces and is followed by either `    pub async fn` or `    #[allow`.
