import re

with open("/Users/abhiram/Documents/eraya/backend/src/modules/academic/repository.rs", "r") as f:
    text = f.read()

# Fix structs (if missing)
def repl_struct(m):
    return m.group(1) + "}\n"
text = re.sub(r"(pub struct [^{]+{[^}]+?\n)(\n)", repl_struct, text, flags=re.MULTILINE)

# If it's missing at EOF
if text.strip()[-1] != '}':
    text = text.rstrip() + "\n}\n"

with open("/Users/abhiram/Documents/eraya/backend/src/modules/academic/repository.rs", "w") as f:
    f.write(text)
