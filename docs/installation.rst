Installation
============

PyPI
----
Install the published wheel:

.. code-block:: bash

   pip install pygitx

Editable / development
----------------------
Clone the repo and install in editable mode (requires a Python toolchain and Rust):

.. code-block:: bash

   python -m venv .venv
   source .venv/bin/activate
   pip install --upgrade pip setuptools wheel maturin
   pip install -e .

Makefile helpers are available:

.. code-block:: bash

   make venv
   source .venv/bin/activate
   make install       # build native extension
   make develop       # pip install -e .

Docs
----
Build the Sphinx docs locally:

.. code-block:: bash

   make docs

HTML will be emitted to ``docs/_build/html``.
