import os
import sys
from datetime import datetime

try:
    import tomllib  # Python 3.11+
except ModuleNotFoundError:  # pragma: no cover
    import tomli as tomllib  # type: ignore

# Make the repository root importable for custom modules if needed.
ROOT = os.path.abspath(os.path.join(__file__, "..", ".."))
if ROOT not in sys.path:
    sys.path.insert(0, ROOT)

project = "PyGitX"
author = "pygitx contributors"
copyright = f"{datetime.now():%Y}, {author}"


def _read_version() -> str:
    pyproject = os.path.join(ROOT, "pyproject.toml")
    cargo = os.path.join(ROOT, "Cargo.toml")
    for path in (pyproject, cargo):
        if not os.path.exists(path):
            continue
        try:
            with open(path, "rb") as f:
                data = tomllib.load(f)
            if "project" in data and "version" in data["project"]:
                return str(data["project"]["version"])
            if "package" in data and "version" in data["package"]:
                return str(data["package"]["version"])
        except Exception:
            continue
    raise RuntimeError("Could not determine version from pyproject.toml or Cargo.toml")


release = _read_version()
version = release

extensions = [
    "sphinx.ext.autodoc",
    "sphinx.ext.napoleon",
]

templates_path = ["_templates"]
exclude_patterns = ["_build", "Thumbs.db", ".DS_Store"]

html_theme = "sphinx_rtd_theme"
html_static_path = ["_static"]
html_logo = os.path.abspath(os.path.join(__file__, "..", "..", ".github", "icon.png"))
html_css_files = ["custom.css"]

# We describe the Python surface manually (not relying on importing the extension at build time).
autodoc_mock_imports = ["pygitx"]
autodoc_member_order = "bysource"
