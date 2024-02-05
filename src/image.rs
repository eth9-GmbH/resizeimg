#![forbid(unsafe_code)]
use anyhow::{anyhow, Result};
use axum::http::header::{HeaderMap, HeaderValue, CONTENT_TYPE, VARY};
use bytes::Bytes;
use image::{
    codecs::png::FilterType as PngFilterType, imageops::FilterType, load_from_memory_with_format,
    DynamicImage, ImageFormat,
};
use libvips::{ops, VipsImage};
use serde::Deserialize;

const DEFAULT_GEOMETRY: (u32, u32) = (800, 800);

mod image_rs;
mod vips;

use image_rs::imagers_decode;
use vips::vips_decode;

trait ImageProcessing {
    fn resize(&self, width: u32, height: u32) -> Result<Engine>;
    fn encode(&self, format: ImageFormat) -> Result<Vec<u8>>;
    fn get_geometry(&self) -> (u32, u32);
}

#[derive(Debug, Clone, Deserialize, Default, PartialEq)]
pub enum EngineType {
    #[default]
    ImageRs,
    Vips,
}
#[derive(Debug, Clone)]
pub enum Engine {
    ImageRs(DynamicImage),
    Vips(VipsImage),
}
impl ImageProcessing for Engine {
    fn resize(&self, width: u32, height: u32) -> Result<Engine> {
        match self {
            Engine::ImageRs(d) => {
                Ok(Engine::ImageRs(d.resize(width, height, FilterType::Triangle)))
            }
            Engine::Vips(v) => {
                let width_ratio = width as f64 / v.get_width() as f64;
                let height_ratio = height as f64 / v.get_height() as f64;
                let ratio = match width_ratio > height_ratio {
                    true => width_ratio,
                    false => height_ratio,
                };
                debug!("Resizing with ratio {ratio}");
                let data = ops::resize(v, ratio)?;
                Ok(Engine::Vips(data))
            }
        }
    }
    fn encode(&self, format: ImageFormat) -> Result<Vec<u8>> {
        match self {
            Engine::ImageRs(d) => imagers_decode(d, format),
            Engine::Vips(v) => vips_decode(v, format),
        }
    }
    fn get_geometry(&self) -> (u32, u32) {
        match self {
            Engine::ImageRs(d) => (d.width(), d.height()),
            Engine::Vips(v) => (v.get_width() as u32, v.get_height() as u32),
        }
    }
}

pub struct Image {
    data: Engine,
    mime: ImageFormat,
    headers: HeaderMap,
}
impl Image {
    pub fn new(
        bytes: Bytes,
        mut upstream_headers: HeaderMap,
        geometry: Option<(u32, u32)>,
        engine: EngineType,
    ) -> Result<Self> {
        let mime = if let Some(content_type) = upstream_headers.get(CONTENT_TYPE) {
            match ImageFormat::from_mime_type(content_type.to_str().unwrap_or_default()) {
                Some(m) => m,
                None => return Err(anyhow!("Could not parse mime type")),
            }
        } else {
            error!("Could not parse mime type");
            return Err(anyhow!("Mime not parseable"));
        };
        debug!("Mime: {}", mime.to_mime_type());
        //headers.append(CACHE_CONTROL, HeaderValue::from_static("public"));
        upstream_headers.append(VARY, HeaderValue::from_static("Accept"));

        debug!("Loading image");
        let raw_data = import_image(bytes, engine, mime)?;
        let (nwidth, nheight) = if let Some(ngeometry) = geometry {
            debug!("Desired geometry: {}x{}", ngeometry.0, ngeometry.1);
            (ngeometry.0, ngeometry.1)
        } else {
            DEFAULT_GEOMETRY
        };
        let (w, h) = raw_data.get_geometry();
        debug!("Original size: {w}x{h}",);
        let data = if w < nwidth || h < nheight {
            raw_data
        } else {
            debug!("Resizing to {nwidth}x{nheight}");
            raw_data.resize(nwidth, nheight)?
        };
        Ok(Image { data, mime, headers: upstream_headers })
    }

    pub fn get_headers(&self) -> HeaderMap {
        self.headers.clone()
    }

    pub fn set_mime(&mut self, mime: ImageFormat) {
        self.mime = mime;
        self.headers
            .insert(CONTENT_TYPE, HeaderValue::from_static(ImageFormat::to_mime_type(&mime)));
    }

    pub fn save(&mut self) -> Result<Vec<u8>> {
        self.data.encode(self.mime)
    }
}

fn import_image(data: Bytes, engine: EngineType, format: ImageFormat) -> Result<Engine> {
    match engine {
        EngineType::ImageRs => {
            Ok(Engine::ImageRs(load_from_memory_with_format(data.as_ref(), format)?))
        }
        EngineType::Vips => Ok(Engine::Vips(VipsImage::new_from_buffer(&data[..], "")?)),
    }
}
