use super::State;
use crate::bind::plugin::ink::host::text;

/// Measuring text needs fonts, and the host has none yet (the measurer of architecture.md
/// section 4 is not built): every font is unknown, said as the typed refusal.
impl text::Host for State {
    fn measure(&mut self, _font: String, _size: f64, _text: String) -> Result<text::Extent, text::Unmeasured> {
        Err(text::Unmeasured::FontUnknown)
    }
}
