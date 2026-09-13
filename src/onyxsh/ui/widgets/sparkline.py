# onyxsh/ui/widgets/sparkline.py
"""
High-performance Cairo-based sparkline chart widget for GTK 4.

Renders real-time continuous series with anti-aliased Bézier/linear curves,
soft vertical gradients, and zero animation overhead.
"""

from collections import deque
from typing import Deque, List, Optional, Tuple

import cairo
import gi

gi.require_version("Gtk", "4.0")
from gi.repository import Gdk, Gtk


class SparklineCanvas(Gtk.DrawingArea):
    """
    Compact, hardware-friendly drawing area for live time-series sparklines.
    Renders with gradient fill under curve and optional secondary series (for dual I/O).
    """

    def __init__(
        self,
        max_points: int = 30,
        line_color: Tuple[float, float, float] = (0.2, 0.6, 1.0),
        fill_alpha: float = 0.25,
        secondary_color: Optional[Tuple[float, float, float]] = None,
        height_request: int = 48,
    ) -> None:
        super().__init__()
        self._max_points = max_points
        self._primary_data: Deque[float] = deque(maxlen=max_points)
        self._secondary_data: Optional[Deque[float]] = (
            deque(maxlen=max_points) if secondary_color else None
        )
        self._line_color = line_color
        self._fill_alpha = fill_alpha
        self._secondary_color = secondary_color
        self._max_val: float = 100.0
        self._auto_scale: bool = False

        self.set_content_height(height_request)
        self.set_hexpand(True)
        self.add_css_class("sparkline-canvas")

        # Connect GTK 4 draw function
        self.set_draw_func(self._on_draw)

    def set_max_value(self, max_val: float) -> None:
        """Fixes the upper bound scale (e.g. 100.0 for percentages)."""
        self._max_val = max(1.0, float(max_val))
        self._auto_scale = False

    def enable_auto_scale(self) -> None:
        """Enables dynamic scaling to the highest data point (useful for network speeds)."""
        self._auto_scale = True

    def clear(self) -> None:
        """Clears all historical data points."""
        self._primary_data.clear()
        if self._secondary_data is not None:
            self._secondary_data.clear()
        self.queue_draw()

    def push_value(self, primary_val: float, secondary_val: Optional[float] = None) -> None:
        """Appends new sample point(s) and requests a single redraw."""
        self._primary_data.append(float(primary_val))
        if self._secondary_data is not None and secondary_val is not None:
            self._secondary_data.append(float(secondary_val))
        self.queue_draw()

    def _on_draw(
        self,
        area: Gtk.DrawingArea,
        cr: cairo.Context,
        width: int,
        height: int,
    ) -> None:
        """Draws sparkline curve(s) using native Cairo path operations."""
        if not self._primary_data or width <= 0 or height <= 0:
            return

        cr.set_antialias(cairo.ANTIALIAS_SUBPIXEL)

        # Determine scale ceiling
        if self._auto_scale:
            all_vals = list(self._primary_data)
            if self._secondary_data:
                all_vals.extend(self._secondary_data)
            max_v = max(all_vals) if all_vals else 1.0
            scale_max = max(1.0, max_v * 1.15)
        else:
            scale_max = self._max_val

        # Draw primary series
        self._render_series(
            cr,
            list(self._primary_data),
            width,
            height,
            scale_max,
            self._line_color,
            self._fill_alpha,
        )

        # Draw secondary series (if configured)
        if self._secondary_data and self._secondary_color:
            self._render_series(
                cr,
                list(self._secondary_data),
                width,
                height,
                scale_max,
                self._secondary_color,
                self._fill_alpha * 0.7,
            )

    @staticmethod
    def _render_series(
        cr: cairo.Context,
        points: List[float],
        width: int,
        height: int,
        max_val: float,
        color: Tuple[float, float, float],
        fill_alpha: float,
    ) -> None:
        n = len(points)
        if n < 1:
            return

        padding_bottom = 2.0
        padding_top = 4.0
        usable_height = max(1.0, height - padding_top - padding_bottom)

        dx = width / max(1, n - 1) if n > 1 else width

        # Calculate pixel coordinates
        coords: List[Tuple[float, float]] = []
        for i, val in enumerate(points):
            ratio = min(1.0, max(0.0, val / max_val))
            x = i * dx
            y = height - padding_bottom - (ratio * usable_height)
            coords.append((x, y))

        # 1. Fill Area with Linear Gradient
        cr.save()
        cr.new_path()
        cr.move_to(coords[0][0], height)
        for x, y in coords:
            cr.line_to(x, y)
        cr.line_to(coords[-1][0], height)
        cr.close_path()

        r, g, b = color
        pat = cairo.LinearGradient(0, 0, 0, height)
        pat.add_color_stop_rgba(0.0, r, g, b, fill_alpha)
        pat.add_color_stop_rgba(1.0, r, g, b, 0.02)
        cr.set_source(pat)
        cr.fill()
        cr.restore()

        # 2. Draw Stroke Line
        cr.save()
        cr.new_path()
        cr.move_to(coords[0][0], coords[0][1])
        for x, y in coords[1:]:
            cr.line_to(x, y)

        cr.set_source_rgba(r, g, b, 0.95)
        cr.set_line_width(2.0)
        cr.set_line_cap(cairo.LINE_CAP_ROUND)
        cr.set_line_join(cairo.LINE_JOIN_ROUND)
        cr.stroke()

        # 3. Highlight Latest Point
        last_x, last_y = coords[-1]
        cr.arc(last_x, last_y, 3.0, 0, 2 * 3.14159)
        cr.set_source_rgba(r, g, b, 1.0)
        cr.fill()
        cr.restore()
