import histgit
from pathlib import Path

repo = histgit.open_repo(Path("./test-git"))
head = repo.head()
print("HEAD:", head.id if head else "None")

# Amending the latest commit message (rewrites history)
if head:
    updated = repo.rewrite_author(head.id, "Jiucheng", "hex4a5a@duck.com")
    print("Amended HEAD:", updated.id, updated.summary)