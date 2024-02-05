#![forbid(unsafe_code)]
use crate::image::PngFilterType;
use anyhow::anyhow;
use anyhow::Result;
use image::{
    codecs::{
        avif::AvifEncoder,
        jpeg::JpegEncoder,
        png::{CompressionType, PngEncoder},
        webp::WebPEncoder,
    },
    DynamicImage, ImageFormat,
};

pub fn imagers_decode(image: &DynamicImage, format: ImageFormat) -> Result<Vec<u8>> {
    match format {
        ImageFormat::Jpeg => encode_jpeg(image),
        ImageFormat::Png => encode_png(image),
        ImageFormat::WebP => encode_webp(image),
        ImageFormat::Avif => encode_avif(image),
        _ => {
            error!("Got unsupported image format: {}", format.to_mime_type());
            Err(anyhow!("unsupported format"))
        }
    }
}

fn encode_jpeg(image: &DynamicImage) -> Result<Vec<u8>> {
    debug!("Saving as JPEG");
    let mut buffer: Vec<u8> = Vec::new();
    let encoder = JpegEncoder::new_with_quality(&mut buffer, 95);
    image.write_with_encoder(encoder)?;
    debug!("Saved");
    Ok(buffer)
}

fn encode_png(image: &DynamicImage) -> Result<Vec<u8>> {
    debug!("Saving as PNG");
    let mut buffer: Vec<u8> = Vec::new();
    let encoder = PngEncoder::new_with_quality(
        &mut buffer,
        CompressionType::Default,
        PngFilterType::NoFilter,
    );
    image.write_with_encoder(encoder)?;
    debug!("Saved");
    Ok(buffer)
}

fn encode_webp(image: &DynamicImage) -> Result<Vec<u8>> {
    debug!("Saving as WEBP");
    let mut buffer: Vec<u8> = Vec::new();
    let encoder = WebPEncoder::new_lossless(&mut buffer);
    image.write_with_encoder(encoder)?;
    debug!("Saved");
    Ok(buffer)
}

fn encode_avif(image: &DynamicImage) -> Result<Vec<u8>> {
    debug!("Saving as Avif");
    let mut buffer: Vec<u8> = Vec::new();
    let encoder = AvifEncoder::new_with_speed_quality(&mut buffer, 10, 95);
    image.write_with_encoder(encoder)?;
    debug!("Saved");
    Ok(buffer)
}
