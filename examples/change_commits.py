import histgit
from pathlib import Path

repo = histgit.open_repo(Path("~/Dev/test-git"))
head = repo.head()
print("HEAD:", head.id if head else "None")

# Amending the latest commit message (rewrites history)
if head:
    updated = repo.change_commit_message(head.id, "new message for HEAD")
    print("Amended HEAD:", updated.id, updated.summary)