import re

with open("/Users/abhiram/Documents/eraya/backend/src/modules/academic/repository.rs", "r") as f:
    text = f.read()

# All structs that don't have } before the next #[derive or impl
text = re.sub(r"(\n    pub [^:\n]+: [^,\n]+,\n)\n", r"\1}\n\n", text)
# Some structs end without comma:
text = re.sub(r"(\n    pub [^:\n]+: [^,\n]+)\n\n", r"\1\n}\n\n", text)

with open("/Users/abhiram/Documents/eraya/backend/src/modules/academic/repository.rs", "w") as f:
    f.write(text)
