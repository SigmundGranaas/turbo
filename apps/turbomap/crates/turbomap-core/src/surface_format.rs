//! **Which surface format a host presents in** — one rule for every host.
//!
//! The renderer blends in linear and relies on its target to encode sRGB.
//! A surface either offers an sRGB format, or an 8-bit one whose sRGB form
//! is a permitted *view* (configure the surface with the base format, render
//! through the sRGB view — what WebGPU canvases need). A surface that offers
//! neither cannot show this renderer's colours correctly, and says so: it
//! used to take whatever came first and present linear values un-encoded,
//! which looks darker and is the kind of bug nobody files.

use wgpu::TextureFormat;

/// The format a surface is configured with, and the (sRGB) format frames
/// are rendered into through a view of it. Equal when the surface is sRGB.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SurfaceFormats {
    pub surface: TextureFormat,
    pub render: TextureFormat,
}

/// Choose from the formats a surface offers, in its preference order.
pub fn srgb_surface_formats(offered: &[TextureFormat]) -> Result<SurfaceFormats, String> {
    if let Some(&f) = offered.iter().find(|f| f.is_srgb()) {
        return Ok(SurfaceFormats {
            surface: f,
            render: f,
        });
    }
    if let Some(&f) = offered.iter().find(|f| f.add_srgb_suffix().is_srgb()) {
        return Ok(SurfaceFormats {
            surface: f,
            render: f.add_srgb_suffix(),
        });
    }
    Err(format!(
        "the surface offers no sRGB format and none with an sRGB view ({offered:?}); turbomap renders linear \
         colour that its target must encode, so presenting here would show the wrong colours"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use wgpu::TextureFormat as F;

    #[test]
    fn an_srgb_format_is_used_as_it_is() {
        let f = srgb_surface_formats(&[F::Rgba16Float, F::Bgra8UnormSrgb, F::Bgra8Unorm]).unwrap();
        assert_eq!(
            f,
            SurfaceFormats {
                surface: F::Bgra8UnormSrgb,
                render: F::Bgra8UnormSrgb
            }
        );
    }

    #[test]
    fn otherwise_an_8_bit_surface_is_rendered_through_its_srgb_view() {
        let f = srgb_surface_formats(&[F::Rgba16Float, F::Rgba8Unorm]).unwrap();
        assert_eq!(
            f,
            SurfaceFormats {
                surface: F::Rgba8Unorm,
                render: F::Rgba8UnormSrgb
            }
        );
    }

    #[test]
    fn a_surface_with_neither_is_refused_by_name() {
        let e = srgb_surface_formats(&[F::Rgba16Float, F::Rgb10a2Unorm]).unwrap_err();
        assert!(
            e.contains("Rgba16Float") && e.contains("no sRGB format"),
            "{e}"
        );
    }
}
