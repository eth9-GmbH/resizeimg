#![forbid(unsafe_code)]
use crate::config::{Config, Upstreams};
use crate::image::Image;
use axum::{
    body::Body,
    extract::{Request, State},
    http::header::{HeaderValue, ACCEPT},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use image::ImageFormat;
use regex::Regex;
use std::{
    net::{Ipv4Addr, SocketAddr},
    sync::Arc,
    time::Duration,
};
use tokio::net::TcpListener;

static APP_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"),);

type SharedConfig = Arc<Config>;

pub async fn http_server(config: Config) -> anyhow::Result<()> {
    let address =
        if let Some(addr) = config.listen_address.clone() { addr } else { "0.0.0.0".to_string() };
    let listen_address: Ipv4Addr = address.parse()?;
    let addr = SocketAddr::from((listen_address, config.port.unwrap_or(8080)));
    info!("Listening on {addr}");
    let listener = TcpListener::bind(addr).await?;
    let shared_config = Arc::new(config);
    let app =
        Router::new().route("/health", get(health)).fallback(get(handle)).with_state(shared_config);
    Ok(axum::serve(listener, app.into_make_service()).await?)
}

async fn health<'a>() -> Response {
    (StatusCode::OK, "Healthy\n").into_response()
}

async fn handle(State(shared_config): State<SharedConfig>, req: Request<Body>) -> Response {
    match process(shared_config, req).await {
        Ok(r) => r,
        Err(e) => {
            error!("Processing: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, "Error processing image\n").into_response()
        }
    }
}

async fn process(config: SharedConfig, req: Request<Body>) -> anyhow::Result<Response> {
    // Choose backend_url
    let upstream_url =
        if let Ok(Some(url)) = get_upstream(req.uri().path(), config.upstreams.clone()) {
            url
        } else {
            return Ok((StatusCode::NOT_FOUND, "Not found\n").into_response());
        };
    let (request_parts, _) = req.into_parts();
    let desired_geometry =
        extract_geometry(request_parts.uri.query().unwrap_or_default().to_string());
    debug!("Desired size: {:?}", desired_geometry);
    // Send request to the backend
    // ToDo: Propagate accept requests from downstream?
    let http_client = create_http_client()?;
    let upstream_answer = http_client.get(upstream_url).send().await?;
    if !upstream_answer.status().is_success() {
        error!("Backend responded with code {}", upstream_answer.status().as_str());
        return Ok(Response::builder().status(StatusCode::BAD_GATEWAY).body(Body::empty())?);
    }

    // Extract upstream headers
    let upstream_headers = convert_headers(upstream_answer.headers().clone())?;

    trace!("Upstream headers: {:?}", upstream_headers);
    // Extract response body (image data)
    let payload = upstream_answer.bytes().await?;

    debug!("Image download complete");
    // Create an image object with the response
    let mut image = Image::new(payload, upstream_headers, desired_geometry, config.engine.clone())?;
    // Resize/Convert image and send back to client
    if let Some(target_mime) = get_target_mime(request_parts.headers.get(ACCEPT).cloned()) {
        image.set_mime(target_mime);
    }
    let mut response = Response::new(Body::from(image.save()?));
    let response_headers = response.headers_mut();
    *response_headers = image.get_headers();
    Ok(response)
}

fn create_http_client() -> anyhow::Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .gzip(true)
        .brotli(true)
        .deflate(true)
        .user_agent(APP_USER_AGENT)
        .connect_timeout(Duration::new(5, 0))
        .timeout(Duration::new(10, 0))
        .build()?)
}

fn get_target_mime(accept: Option<HeaderValue>) -> Option<ImageFormat> {
    if let Some(accept_header) = accept {
        let accept_str = accept_header.to_str().unwrap_or_default();
        if accept_str.contains("image/avif") {
            debug!("The client accepts AVIF");
            return Some(ImageFormat::Avif);
        } else if accept_str.contains("image/webp") {
            debug!("The client accepts WebP");
            return Some(ImageFormat::WebP);
        }
    }
    None
}

fn extract_geometry(uri_string: String) -> Option<(u32, u32)> {
    let mut size = String::new();
    for pair in uri_string.split('&') {
        let mut it = pair.split("Resize=").take(2);
        match (it.next(), it.next()) {
            (Some("im="), Some(v)) => {
                size = v.to_string();
                break;
            }
            _ => continue,
        };
    }
    if size.is_empty() {
        return None;
    }
    debug!("Extracted size parameter: {}", size);
    // cut ( and )
    size = size[1..size.len() - 1].to_string();
    let v: Vec<&str> = size.split(',').collect();
    if v.len() != 2 {
        return None;
    }
    let width: u32 = v[0].to_string().trim().parse().unwrap_or_default();
    let height: u32 = v[1].to_string().trim().parse().unwrap_or_default();
    if width == 0 || height == 0 {
        return None;
    }
    debug!("Found geometry: {width}x{height}");
    Some((width, height))
}

fn get_upstream(uri: &str, map: Vec<Upstreams>) -> anyhow::Result<Option<String>> {
    for entry in map {
        debug!("Trying RE {} against {}", &entry.path, uri);
        let re = Regex::new(&entry.path)?;
        if let Some(captures) = re.captures(uri) {
            let mut results = Vec::new();
            for capture in captures.iter().flatten() {
                results.push(capture.as_str().to_string());
            }
            // Remove full match
            results.remove(0);
            let upstream_exp = entry.upstream.to_string();
            trace!("Captured: {:?}", captures);
            let upstream_vec: Vec<_> = upstream_exp.split("{}").map(|s| s.to_string()).collect();
            let zipped: Vec<_> = upstream_vec.into_iter().zip(results.into_iter()).collect();
            let url: Vec<String> =
                zipped.into_iter().flat_map(|(a, b)| vec![a, b.to_string()]).collect();
            debug!("Upstream URL: {}", url.join(""));
            return Ok(Some(url.join("")));
        }
    }
    Ok(None)
}

// Needed because reqwest and axum are using different http versions
fn convert_headers(headers: reqwest::header::HeaderMap) -> anyhow::Result<axum::http::HeaderMap> {
    let mut clean_headers = axum::http::HeaderMap::new();
    for (key, value) in headers.iter() {
        match *key {
            reqwest::header::CONTENT_TYPE => {
                _ = clean_headers.insert(
                    axum::http::header::CONTENT_TYPE,
                    axum::http::HeaderValue::from_str(value.to_str()?)?,
                )
            }
            reqwest::header::LAST_MODIFIED => {
                _ = clean_headers.insert(
                    axum::http::header::LAST_MODIFIED,
                    axum::http::HeaderValue::from_str(value.to_str()?)?,
                )
            }
            reqwest::header::ETAG => {
                _ = clean_headers.insert(
                    axum::http::header::ETAG,
                    axum::http::HeaderValue::from_str(value.to_str()?)?,
                )
            }
            reqwest::header::CACHE_CONTROL => {
                _ = clean_headers.append(
                    axum::http::header::CACHE_CONTROL,
                    axum::http::HeaderValue::from_str(value.to_str()?)?,
                );
            }
            _ => continue,
        };
    }
    Ok(clean_headers)
}
