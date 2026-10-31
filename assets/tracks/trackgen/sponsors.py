"""The sponsors on Bell Deer's hoardings: cartoon Formula 1 jokes, every one invented here.

Each is a name and the colour of its lettering, so a board reads as a brand
at a glance. Nothing here names a real company or borrows a real logo. A
prop that wears a sponsor picks one from here by name (ADR 0038).
"""

from __future__ import annotations

SPONSORS: tuple[tuple[str, str], ...] = (
    ("OVERSTEER INSURANCE", "ink-blue"),
    ("APEX PREDATOR ENERGY", "ink-green"),
    ("CHICANE COLA", "ink-red"),
    ("GRIP & CO. TYRES", "ink-black"),
    ("SLIPSTREAM AIRWAYS", "ink-blue"),
    ("PIT STOP ESPRESSO", "ink-brown"),
    ("UNDERSTEER BROS. BRAKES", "ink-orange"),
    ("LATE BRAKING BANK", "ink-purple"),
    ("BOX BOX BOX LOGISTICS", "ink-red"),
    ("MARBLES OFF-LINE DETERGENT", "ink-green"),
    ("DIRTY AIR CONDITIONING", "ink-blue"),
    ("LOLLIPOP MAN LOLLIPOPS", "ink-orange"),
)


def sponsor(name: str) -> tuple[str, str]:
    """The sponsor called `name` with its ink, for scenery that wears one in particular."""
    return next(s for s in SPONSORS if s[0] == name)


# The arch over Dunlop: a giant tyre wears the tyre sponsor on both faces.
ARCH_SPONSOR = sponsor("GRIP & CO. TYRES")
# The airship over the infield: the airline, on both flanks.
AIRSHIP_SPONSOR = sponsor("SLIPSTREAM AIRWAYS")
# The helicopter over Monarch's harbour: red ink, for a red-and-white helicopter.
HELICOPTER_SPONSOR = sponsor("BOX BOX BOX LOGISTICS")
