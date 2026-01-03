import os
import sys
from datetime import datetime

# Make the repository root importable for custom modules if needed.
ROOT = os.path.abspath(os.path.join(__file__, "..", ".."))
if ROOT not in sys.path:
    sys.path.insert(0, ROOT)

project = "histgit"
author = "histgit contributors"
copyright = f"{datetime.now():%Y}, {author}"
release = "0.1.0"

extensions = [
    "sphinx.ext.autodoc",
    "sphinx.ext.napoleon",
]

templates_path = ["_templates"]
exclude_patterns = ["_build", "Thumbs.db", ".DS_Store"]

html_theme = "sphinx_rtd_theme"
html_static_path = ["_static"]

# We describe the Python surface manually (not relying on importing the extension at build time).
autodoc_mock_imports = ["histgit"]
autodoc_member_order = "bysource"
