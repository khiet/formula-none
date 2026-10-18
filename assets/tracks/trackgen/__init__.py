"""Greybox track generation, pure Python so it runs inside Blender and outside it.

Coordinates are Blender's: X right, Y forward, Z up, metres. `export.py`
converts to the glTF frame (Y up, -Z forward) when it writes the sidecar.
"""
