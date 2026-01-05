Backups
=======

create_backup_ref
-----------------
``Repo.create_backup_ref(prefix: str | None = None) -> str`` / ``pygitx.create_backup_ref(repo, prefix=None) -> str``

Create backup references for HEAD (and its branch if attached) under ``refs/pygitx/backup/<timestamp>``. Returns the backup root ref path.
