#![forbid(unsafe_code)]
use anyhow::{anyhow, Result};
use image::ImageFormat;
use libvips::{ops::ForeignHeifCompression, VipsImage};

pub fn vips_decode(image: &VipsImage, format: ImageFormat) -> Result<Vec<u8>> {
    match format {
        ImageFormat::Jpeg => save_jpeg(image),
        ImageFormat::Png => save_png(image),
        ImageFormat::WebP => save_webp(image),
        ImageFormat::Avif => save_avif(image),
        _ => {
            error!("Got unsupported image format: {}", format.to_mime_type());
            Err(anyhow!("unsupported format"))
        }
    }
}

fn save_jpeg(image: &VipsImage) -> Result<Vec<u8>> {
    debug!("Saving as JPEG");
    let options = libvips::ops::JpegsaveBufferOptions {
        q: 90,
        background: vec![255.],
        optimize_coding: true,
        interlace: true,
        ..libvips::ops::JpegsaveBufferOptions::default()
    };

    Ok(libvips::ops::jpegsave_buffer_with_opts(image, &options)?)
}

fn save_png(image: &VipsImage) -> Result<Vec<u8>> {
    debug!("Saving as PNG");
    let options = libvips::ops::PngsaveBufferOptions {
        q: 90,
        background: vec![255.],
        interlace: true,
        bitdepth: 8,
        ..libvips::ops::PngsaveBufferOptions::default()
    };
    Ok(libvips::ops::pngsave_buffer_with_opts(image, &options)?)
}

fn save_webp(image: &VipsImage) -> Result<Vec<u8>> {
    debug!("Saving as WEBP");
    let options = libvips::ops::WebpsaveBufferOptions {
        q: 90,
        background: vec![255.],
        ..libvips::ops::WebpsaveBufferOptions::default()
    };
    Ok(libvips::ops::webpsave_buffer_with_opts(image, &options)?)
}

fn save_avif(image: &VipsImage) -> Result<Vec<u8>> {
    debug!("Saving as Avif");
    let options = libvips::ops::HeifsaveBufferOptions {
        background: vec![255.],
        compression: ForeignHeifCompression::Av1,
        ..Default::default()
    };
    Ok(libvips::ops::heifsave_buffer_with_opts(image, &options)?)
}
