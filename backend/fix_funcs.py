import re

with open("/Users/abhiram/Documents/eraya/backend/src/modules/academic/repository.rs", "r") as f:
    text = f.read()

# Split by "pub async fn"
parts = text.split("    pub async fn")
if len(parts) > 1:
    new_text = parts[0]
    for i in range(1, len(parts)):
        # If it's not the first one, it means we are closing the previous function
        if i > 1:
            # Add } to the end of the previous part
            new_text = new_text.rstrip() + "\n    }\n\n"

        new_text += "    pub async fn" + parts[i]

    new_text = new_text.rstrip() + "\n    }\n}\n"

    with open("/Users/abhiram/Documents/eraya/backend/src/modules/academic/repository.rs", "w") as f:
        f.write(new_text)
