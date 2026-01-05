Revision parsing
================

Resolve git revision strings.

rev_parse
---------
``Repo.rev_parse(spec: str) -> str`` / ``pygitx.rev_parse(repo, spec) -> str``

Resolve a revision spec to a hex object id. Accepts typical git syntax:

- ``HEAD``, ``HEAD~N``, ``HEAD^``
- Branch names
- Tags
- Short or full oids

.. code-block:: python

   print(repo.rev_parse("HEAD~1"))
   print(pygitx.rev_parse(repo, "main"))
