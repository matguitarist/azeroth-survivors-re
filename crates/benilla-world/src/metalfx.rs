//! Apple's MetalFX spatial upscaler for a world rendered below the window's size (`renderScale`
//! under 1): the world's finished main texture is scaled to the backdrop's size before the
//! FFXGlow combine reads it, in place of the combine's bilinear stretch. The combine then reads
//! it 1:1; the glow's ¼-res blur still reads the world's own texture.
//!
//! MetalFX has no wgpu binding: the scaler is created and encoded through the raw Metal objects
//! wgpu-hal exposes (`as_hal`), on a command buffer of its own, queued between the world's
//! passes and the UI's so the order holds. wgpu-hal's textures are private and hazard-tracked,
//! so Metal orders the scaler's read and write against the passes around it.
//!
//! `WOW_METALFX=0` keeps the bilinear stretch, for comparison. Elsewhere than macOS, or on a GPU
//! MetalFX does not support, [`upscale`] returns `None` and the combine stretches as before.

use bevy::prelude::*;
use bevy::render::render_resource::{Texture, TextureView};
use bevy::render::renderer::RenderContext;

/// The world's main texture, scaled to `out`, or `None` to stretch it in the combine: no
/// MetalFX, switched off, or the world is not smaller than the backdrop.
pub fn upscale(
    render_context: &mut RenderContext,
    view: Entity,
    source: &Texture,
    out: UVec2,
) -> Option<TextureView> {
    #[cfg(target_os = "macos")]
    {
        imp::upscale(render_context, view, source, out)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (render_context, view, source, out);
        None
    }
}

#[cfg(target_os = "macos")]
mod imp {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};

    use bevy::prelude::*;
    use bevy::render::render_resource::{
        CommandEncoderDescriptor, Extent3d, Texture, TextureDescriptor, TextureDimension,
        TextureFormat, TextureUsages, TextureView, TextureViewDescriptor,
    };
    use bevy::render::renderer::RenderContext;
    use objc2::rc::Id;
    use objc2::runtime::{AnyClass, AnyObject};
    use objc2::{msg_send, msg_send_id};

    #[link(name = "MetalFX", kind = "framework")]
    extern "C" {}

    type Metal = wgpu::hal::api::Metal;

    /// `MTLFXSpatialScalerColorProcessingModePerceptual`: the world's lane holds gamma values,
    /// which is what the perceptual mode expects.
    const PERCEPTUAL: isize = 0;

    /// One view's scaler and its output, rebuilt when a size or the format changes.
    struct Scaler {
        key: (UVec2, UVec2, TextureFormat),
        scaler: Id<AnyObject>,
        // Kept alive with the scaler, which holds the raw texture.
        _output: Texture,
        view: TextureView,
    }

    // The Metal objects are used from whichever render thread runs the node, one at a time
    // (the map's lock); Metal's scaler and textures are not tied to a thread.
    unsafe impl Send for Scaler {}

    #[derive(Default)]
    struct State {
        scalers: HashMap<Entity, Scaler>,
        /// Set once MetalFX is found missing or refuses a texture: no retry every frame.
        unavailable: bool,
    }

    fn state() -> &'static Mutex<State> {
        static STATE: OnceLock<Mutex<State>> = OnceLock::new();
        STATE.get_or_init(|| {
            let off = std::env::var("WOW_METALFX").is_ok_and(|v| v == "0");
            if off {
                info!("metalfx: off (WOW_METALFX=0); a scaled world is stretched bilinearly");
            }
            Mutex::new(State {
                unavailable: off,
                ..default()
            })
        })
    }

    /// `MTLPixelFormat` for the formats a world view's main texture can have.
    fn mtl_format(format: TextureFormat) -> Option<usize> {
        Some(match format {
            TextureFormat::Rgba16Float => 115,
            TextureFormat::Rgba8Unorm => 70,
            TextureFormat::Rgba8UnormSrgb => 71,
            TextureFormat::Bgra8Unorm => 80,
            TextureFormat::Bgra8UnormSrgb => 81,
            TextureFormat::Rgb10a2Unorm => 90,
            TextureFormat::Rg11b10Ufloat => 92,
            _ => return None,
        })
    }

    /// The raw Objective-C object behind a `metal` crate handle: its ref type's address.
    fn objc<T>(r: &T) -> *mut AnyObject {
        r as *const T as *mut AnyObject
    }

    fn raw_texture(texture: &Texture) -> Option<*mut AnyObject> {
        // SAFETY: the handle is only read, and lives as long as `texture`.
        unsafe {
            let hal = texture.as_hal::<Metal>()?;
            Some(objc(&**hal.raw_handle()))
        }
    }

    fn build(
        render_context: &RenderContext,
        source: &Texture,
        key: (UVec2, UVec2, TextureFormat),
    ) -> Result<Scaler, String> {
        let (input, out, format) = key;
        let pixel =
            mtl_format(format).ok_or_else(|| format!("no MetalFX format for {format:?}"))?;
        // The combine clamps the world to 0..1 gamma, so a float world loses nothing in an 8-bit
        // output, and the output is the larger texture: half its bytes is most of the cost.
        let out_format = if format.is_srgb() {
            format
        } else {
            TextureFormat::Rgba8Unorm
        };
        let out_pixel = mtl_format(out_format).ok_or("no MetalFX output format")?;
        let device = render_context.render_device().wgpu_device();
        // SAFETY: the raw device is only read, under wgpu-hal's own lock.
        let hal = unsafe { device.as_hal::<Metal>() }.ok_or("not a Metal device")?;
        let raw_device = hal.raw_device().lock();
        let device_ptr = objc(&**raw_device);
        let class = AnyClass::get("MTLFXSpatialScalerDescriptor").ok_or("MetalFX is missing")?;
        // SAFETY: MTLFXSpatialScalerDescriptor's documented selectors, with their own types.
        let scaler = unsafe {
            let supported: bool = msg_send![class, supportsDevice: device_ptr];
            if !supported {
                return Err("this GPU has no MetalFX spatial scaler".into());
            }
            let desc: Id<AnyObject> = msg_send_id![class, new];
            let _: () = msg_send![&desc, setInputWidth: input.x as usize];
            let _: () = msg_send![&desc, setInputHeight: input.y as usize];
            let _: () = msg_send![&desc, setOutputWidth: out.x as usize];
            let _: () = msg_send![&desc, setOutputHeight: out.y as usize];
            let _: () = msg_send![&desc, setColorTextureFormat: pixel];
            let _: () = msg_send![&desc, setOutputTextureFormat: out_pixel];
            let _: () = msg_send![&desc, setColorProcessingMode: PERCEPTUAL];
            let scaler: Option<Id<AnyObject>> =
                msg_send_id![&desc, newSpatialScalerWithDevice: device_ptr];
            scaler.ok_or("MetalFX made no scaler")?
        };
        drop(raw_device);

        // Sampled by the combine, written by the scaler: it asks for render target and, on some
        // GPUs, shader write, so both are given where the format allows a storage binding.
        let mut usage = TextureUsages::TEXTURE_BINDING | TextureUsages::RENDER_ATTACHMENT;
        if !out_format.is_srgb() {
            usage |= TextureUsages::STORAGE_BINDING;
        }
        let output = render_context
            .render_device()
            .create_texture(&TextureDescriptor {
                label: Some("metalfx_world"),
                size: Extent3d {
                    width: out.x,
                    height: out.y,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: out_format,
                usage,
                view_formats: &[],
            });
        let raw_out = raw_texture(&output).ok_or("no raw output texture")?;
        let raw_in = raw_texture(source).ok_or("no raw world texture")?;
        // SAFETY: plain property reads.
        let (need_out, have_out, need_in, have_in) = unsafe {
            let need_out: usize = msg_send![&scaler, outputTextureUsage];
            let have_out: usize = msg_send![raw_out, usage];
            let need_in: usize = msg_send![&scaler, colorTextureUsage];
            let have_in: usize = msg_send![raw_in, usage];
            (need_out, have_out, need_in, have_in)
        };
        if need_out & !have_out != 0 || need_in & !have_in != 0 {
            return Err(format!(
                "texture usage: output has {have_out:#x} of {need_out:#x}, input {have_in:#x} of {need_in:#x}"
            ));
        }
        let view = output.create_view(&TextureViewDescriptor::default());
        info!(
            "metalfx: spatial upscale {}x{} -> {}x{} ({format:?} -> {out_format:?})",
            input.x, input.y, out.x, out.y
        );
        Ok(Scaler {
            key,
            scaler,
            _output: output,
            view,
        })
    }

    pub(super) fn upscale(
        render_context: &mut RenderContext,
        view: Entity,
        source: &Texture,
        out: UVec2,
    ) -> Option<TextureView> {
        let input = UVec2::new(source.width(), source.height());
        // Only a world smaller than its backdrop on both axes; supersampling keeps the combine's
        // box average.
        if input.x >= out.x || input.y >= out.y {
            return None;
        }
        let mut state = state().lock().ok()?;
        if state.unavailable {
            return None;
        }
        let key = (input, out, source.format());
        if state.scalers.get(&view).is_none_or(|s| s.key != key) {
            match build(render_context, source, key) {
                Ok(scaler) => {
                    state.scalers.insert(view, scaler);
                }
                Err(e) => {
                    warn!("metalfx: unavailable, the world is stretched bilinearly: {e}");
                    state.unavailable = true;
                    state.scalers.clear();
                    return None;
                }
            }
        }
        let scaler = state.scalers.get(&view)?;
        let raw_in = raw_texture(source)?;
        let raw_out = raw_texture(&scaler._output)?;

        let mut encoder =
            render_context
                .render_device()
                .create_command_encoder(&CommandEncoderDescriptor {
                    label: Some("metalfx_upscale"),
                });
        // SAFETY: a fresh encoder, so no wgpu-hal pass or blit encoder is open on its buffer; the
        // scaler encodes its own work and closes it, and wgpu ends the buffer at `finish`.
        let encoded = unsafe {
            encoder.as_hal_mut::<Metal, _, _>(|hal| {
                let buffer = objc(&**hal?.raw_command_buffer()?);
                let s = &scaler.scaler;
                let _: () = msg_send![s, setColorTexture: raw_in];
                let _: () = msg_send![s, setOutputTexture: raw_out];
                let _: () = msg_send![s, setInputContentWidth: input.x as usize];
                let _: () = msg_send![s, setInputContentHeight: input.y as usize];
                let _: () = msg_send![s, encodeToCommandBuffer: buffer];
                Some(())
            })
        };
        encoded?;
        render_context.add_command_buffer(encoder.finish());
        Some(scaler.view.clone())
    }
}
