import re

with open("/Users/abhiram/Documents/eraya/backend/src/modules/academic/repository.rs", "r") as f:
    text = f.read()

# Strip any existing function-ending `}` that are sitting alone, to normalize:
text = re.sub(r"^\s*}\s*\n", "", text, flags=re.MULTILINE)

# Now, we know exactly where functions and structs start.
# Let's split by line.
lines = text.split("\n")
new_lines = []

for i, line in enumerate(lines):
    # Before we append a new pub async fn or #[allow, we close the previous one
    stripped = line.strip()
    if stripped.startswith("pub async fn ") or stripped.startswith("#[allow"):
        if i > 0 and "impl " not in lines[i-1]:
            # Close the previous function
            new_lines.append("    }")

    if stripped.startswith("pub struct ") and not line.endswith(";"):
        if i > 0:
            # Structs might need closing if we were in one.
            pass

    new_lines.append(line)

new_lines.append("}") # Close the last function
new_lines.append("}") # Close impl

# Fix structs
text = "\n".join(new_lines)
text = re.sub(r"(pub struct [^{]+{[^}]+?\n)(\n)", r"\1}\n\n", text, flags=re.MULTILINE)
text = re.sub(r"(pub struct [^{]+{[^}]+?,\n)(\n)", r"\1}\n\n", text, flags=re.MULTILINE)

with open("/Users/abhiram/Documents/eraya/backend/src/modules/academic/repository.rs", "w") as f:
    f.write(text)
