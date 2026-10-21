//! GIF encoder layer.
//!
//! A thin wrapper around [`gif::Encoder`] optimized for animations with very few changes between frames

use std::io::Write;

use gif::{DisposalMethod, Encoder, Frame};

/// A rectangle of the canvas, in pixels. `left`/`top` is the top-left corner.
pub struct Rect {
    pub left: usize,
    pub top: usize,
    pub width: usize,
    pub height: usize,
}

/// Converts a canvas coordinate to the 16 bits that GIF stores.
fn to_u16(value: usize) -> Result<u16, String> {
    u16::try_from(value).map_err(|_| format!("{value} does not fit in a GIF coordinate"))
}

/// Writes an animated GIF one frame at a time, merging unchanged ticks into the previous frame's delay.
pub struct GifWriter<W: Write> {
    encoder: Encoder<W>,
    /// Current frame, pending of being written, so if no new changes arrive the delay can just grow.
    pending: Option<Frame<'static>>,
    /// Duration of one tick in GIF units (1/100 s).
    tick: u16,
}

impl<W: Write> GifWriter<W> {
    /// Starts a GIF of `w` by `h` pixels with a global `palette` [R,G,B] bytes.
    pub fn new(writer: W, w: usize, h: usize, palette: &[u8], tick: u16) -> Result<Self, String> {
        let encoder = Encoder::new(writer, to_u16(w)?, to_u16(h)?, palette).map_err(|e| e.to_string())?;

        Ok(GifWriter {
            encoder,
            pending: None,
            tick,
        })
    }

    /// Queues a frame that covers `rect`. `pixels` are palette indices in row-major order, its len() must match
    /// `rect.width * rect.height`; if `transparent` is set, that index leaves the pixel as it was.
    pub fn push(
        &mut self, rect: Rect, pixels: impl IntoIterator<Item = u8>, transparent: Option<u8>,
    ) -> Result<(), String> {
        self.flush()?; // Write pending frame

        let pixels: Vec<u8> = pixels.into_iter().collect();
        let mut frame = Frame::from_indexed_pixels(to_u16(rect.width)?, to_u16(rect.height)?, pixels, transparent);

        frame.left = to_u16(rect.left)?;
        frame.top = to_u16(rect.top)?;
        frame.delay = self.tick;
        frame.dispose = DisposalMethod::Keep;
        self.pending = Some(frame);

        Ok(())
    }

    /// One tick passed with no visible change: show the last frame longer.
    pub fn hold(&mut self) {
        if let Some(frame) = &mut self.pending {
            frame.delay = frame.delay.saturating_add(self.tick);
        }
    }

    /// Writes the pending frame, if there is one.
    fn flush(&mut self) -> Result<(), String> {
        if let Some(frame) = self.pending.take() {
            self.encoder.write_frame(&frame).map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    /// Writes the last pending frame. The GIF trailer is written when the encoder is dropped.
    pub fn finish(&mut self) -> Result<(), String> {
        self.flush()
    }
}
