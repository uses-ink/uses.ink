## 0038 Path data in tenths, as a `T` chain; lengths by `sqrt`

Accepted, owner. Paths are written in whole tenths of a pixel (no leading zero, no trailing
`.0`), as one `Q` and then a `T` chain: mirroring a control through the midpoint to the next
gives exactly the next, so the controls after the first are implied. Each anchor is rounded so
the next implied control lands on its true point, which keeps every control and anchor within a
tenth of a pixel per axis with no drift. Lengths are `sqrt(x² + y²)` instead of `hypot`. Output
is no longer byte for byte the mock's JS (0033's parity claim ends); native and wasm still agree,
and the golden fixtures are the reference. **Why:** a 336 px line went from 96 us to 10 us (with
the formatting work of the same day) and brand files shrank 58%, with no visible change at 4x;
the ink is drawn without anti-aliasing, so hundredths never showed. **Reversal:** a theme drawn
large enough that a tenth shows.
