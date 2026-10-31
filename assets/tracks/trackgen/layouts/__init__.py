"""One module per track; each exposes `LAYOUT`."""

from __future__ import annotations

from importlib import import_module

from ..layout import Layout

NAMES = ("oval", "bell-deer", "goldstone", "monarch")


def load(name: str) -> Layout:
    if name not in NAMES:
        raise KeyError(f"no track named {name!r}; known tracks: {', '.join(NAMES)}")
    return import_module(f"{__name__}.{name.replace('-', '_')}").LAYOUT
