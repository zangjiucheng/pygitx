import histgit


def main() -> None:
    repo = histgit.open_repo("./test-git")
    head = repo.head()
    print(f"HEAD: {head.id if head else 'None'}")

    for commit in repo.list_commits(max=5):
        email = commit.email or ""
        summary = commit.summary or ""
        print(f"{commit.id[:7]} {commit.author} <{email}> {summary}")


if __name__ == "__main__":
    main()
