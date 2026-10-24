"""A closed centreline built turtle-style from straights and arcs, sampled by arc length."""

from __future__ import annotations

import math
from bisect import bisect_left
from dataclasses import dataclass, field

# Metres between centreline samples. Every ribbon, wall and placement is
# derived from these samples, so this is the greybox's resolution.
SAMPLE_STEP_M = 1.0


@dataclass(frozen=True)
class Sample:
    x: float
    y: float
    heading: float
    """Direction of travel in radians, counter-clockwise from +X."""
    s: float
    """Arc length from the start of the path."""
    curvature: float
    """1 / radius, positive turning left, zero on a straight."""

    @property
    def left_normal(self) -> tuple[float, float]:
        return (-math.sin(self.heading), math.cos(self.heading))

    def offset(self, d: float) -> tuple[float, float]:
        """Point `d` metres to the left of the sample (negative is right).

        On the inside of a turn the offset is capped short of the turn's
        centre so a wide ribbon never folds back on itself.
        """
        if self.curvature != 0.0 and d * self.curvature > 0.0:
            d = math.copysign(min(abs(d), 1.0 / abs(self.curvature) - 0.5), d)
        nx, ny = self.left_normal
        return (self.x + d * nx, self.y + d * ny)


@dataclass(frozen=True)
class Segment:
    name: str
    s_start: float
    s_end: float
    turn: int
    """+1 turning left, -1 turning right, 0 straight."""

    @property
    def length(self) -> float:
        return self.s_end - self.s_start

    @property
    def s_mid(self) -> float:
        return (self.s_start + self.s_end) / 2.0


@dataclass
class Path:
    """A closed loop of samples. Built with `Turtle`, never by hand."""

    samples: list[Sample]
    segments: list[Segment]

    @property
    def length(self) -> float:
        return self.segments[-1].s_end

    def segment(self, name: str) -> Segment:
        for segment in self.segments:
            if segment.name == name:
                return segment
        raise KeyError(name)

    def index_at(self, s: float) -> int:
        """Index of the sample nearest to arc length `s`, which may wrap around the loop."""
        s %= self.length
        distances = [sample.s for sample in self.samples]
        after = bisect_left(distances, s)
        before = after - 1
        if after == len(self.samples):
            # Past the last sample the loop continues to the start point.
            return 0 if self.length - s < s - distances[before] else before
        if before < 0 or distances[after] - s <= s - distances[before]:
            return after
        return before

    def at(self, s: float) -> Sample:
        return self.samples[self.index_at(s)]

    def interpolate(self, s: float) -> Sample:
        """The point at arc length `s` exactly, between the samples either
        side of it; the heading and curvature are the earlier sample's, so
        on a straight the point is exact and on an arc it sits on the chord."""
        s %= self.length
        distances = [sample.s for sample in self.samples]
        after = bisect_left(distances, s)
        if after < len(self.samples) and distances[after] == s:
            return self.samples[after]
        before = self.samples[after - 1]
        # Past the last sample the loop continues to the start point.
        next_sample = self.samples[after % len(self.samples)]
        s_next = next_sample.s if after < len(self.samples) else self.length
        t = (s - before.s) / (s_next - before.s)
        return Sample(
            before.x + t * (next_sample.x - before.x),
            before.y + t * (next_sample.y - before.y),
            before.heading,
            s,
            before.curvature,
        )

    def indices_between(self, s_start: float, s_end: float) -> list[int]:
        """Sample indices from `s_start` to `s_end` inclusive, wrapping past the start line."""
        first = self.index_at(s_start)
        last = self.index_at(s_end)
        count = len(self.samples)
        span = (last - first) % count
        return [(first + i) % count for i in range(span + 1)]

    def bounds(self) -> tuple[float, float, float, float]:
        xs = [sample.x for sample in self.samples]
        ys = [sample.y for sample in self.samples]
        return (min(xs), min(ys), max(xs), max(ys))


@dataclass
class Turtle:
    """Lays a path segment by segment from a start point and heading.

    Each primitive appends samples every `SAMPLE_STEP_M` along its length,
    excluding its own start point (that is the previous primitive's end).
    `close()` finishes the loop with a straight back to the start, so the
    author must have turned a net 360 degrees and lined up with the start.
    """

    x: float
    y: float
    heading_deg: float
    _samples: list[Sample] = field(default_factory=list)
    _segments: list[Segment] = field(default_factory=list)
    _s: float = 0.0

    def __post_init__(self) -> None:
        self._start = (self.x, self.y, math.radians(self.heading_deg))
        self._heading = math.radians(self.heading_deg)
        self._samples.append(Sample(self.x, self.y, self._heading % math.tau, 0.0, 0.0))

    def straight(self, name: str, length: float) -> Turtle:
        if length <= 0.0:
            raise ValueError(f"{name}: a straight needs a positive length, got {length}")
        steps = max(1, round(length / SAMPLE_STEP_M))
        step = length / steps
        s_start = self._s
        for _ in range(steps):
            self.x += step * math.cos(self._heading)
            self.y += step * math.sin(self._heading)
            self._s += step
            self._samples.append(Sample(self.x, self.y, self._heading % math.tau, self._s, 0.0))
        self._segments.append(Segment(name, s_start, self._s, 0))
        return self

    def straight_until_x(self, name: str, x: float) -> Turtle:
        """A straight that ends where the path crosses `x`; the heading must not be parallel to Y."""
        return self.straight(name, (x - self.x) / math.cos(self._heading))

    def arc(self, name: str, radius: float, degrees: float) -> Turtle:
        """A circular arc; positive degrees turn left (counter-clockwise from above)."""
        if radius <= 0.0 or degrees == 0.0:
            raise ValueError(f"{name}: an arc needs a positive radius and a non-zero angle")
        turn = 1 if degrees > 0 else -1
        length = radius * math.radians(abs(degrees))
        steps = max(1, round(length / SAMPLE_STEP_M))
        step_angle = math.radians(degrees) / steps
        step = length / steps
        s_start = self._s
        cx = self.x - turn * radius * math.sin(self._heading)
        cy = self.y + turn * radius * math.cos(self._heading)
        for _ in range(steps):
            self._heading += step_angle
            self.x = cx + turn * radius * math.sin(self._heading)
            self.y = cy - turn * radius * math.cos(self._heading)
            self._s += step
            self._samples.append(
                Sample(self.x, self.y, self._heading % math.tau, self._s, turn / radius)
            )
        self._segments.append(Segment(name, s_start, self._s, turn))
        return self

    def close(self, name: str) -> Path:
        x0, y0, heading0 = self._start
        dx, dy = x0 - self.x, y0 - self.y
        gap = math.hypot(dx, dy)
        if gap < SAMPLE_STEP_M / 2.0:
            raise ValueError("the path already ends at its start; drop the closing straight")
        along = dx * math.cos(self._heading) + dy * math.sin(self._heading)
        across = -dx * math.sin(self._heading) + dy * math.cos(self._heading)
        turned = (self._heading - heading0 + math.pi) % math.tau - math.pi
        if abs(across) > 0.01 or abs(turned) > 1e-6 or along <= 0.0:
            raise ValueError(
                f"the path does not line up with its start: {across:.3f} m across, "
                f"{math.degrees(turned):.3f} degrees off, {along:.3f} m to go"
            )
        self.straight(name, along)
        # The closing straight's last sample is the start point again; the
        # loop is implicit, so drop it.
        samples = self._samples[:-1]
        return Path(samples, list(self._segments))


def elevation(features: list[Feature], s: float) -> float:
    return sum(feature.height_at(s) for feature in features)


class Feature:
    def height_at(self, s: float) -> float:
        raise NotImplementedError


@dataclass(frozen=True)
class Hill(Feature):
    """A smooth rise and fall between `s_start` and `s_end`, `height` metres at the crest."""

    s_start: float
    s_end: float
    height: float

    def height_at(self, s: float) -> float:
        if not self.s_start <= s <= self.s_end:
            return 0.0
        t = (s - self.s_start) / (self.s_end - self.s_start)
        return self.height * (1.0 - math.cos(math.tau * t)) / 2.0


@dataclass(frozen=True)
class Ramp(Feature):
    """A straight climb from `s_start` to `s_end` that drops away at the lip, so karts fly off it."""

    s_start: float
    s_end: float
    height: float

    def height_at(self, s: float) -> float:
        if not self.s_start <= s <= self.s_end:
            return 0.0
        return self.height * (s - self.s_start) / (self.s_end - self.s_start)
