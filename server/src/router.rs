use hyper::{Body, Request, Response, StatusCode, header};
use std::convert::Infallible;
use std::collections::HashMap;
use crate::assets::{Asset, get_routes};

lazy_static::lazy_static! {
    static ref ROUTES: HashMap<&'static str, &'static Asset> = get_routes();
}

pub fn route_count() -> usize {
    ROUTES.len()
}

pub async fn route(req: Request<Body>) -> Result<Response<Body>, Infallible> {
    let path = req.uri().path();

    // Try exact match first
    if let Some(asset) = ROUTES.get(path) {
        return Ok(serve_asset(asset, &req, path));
    }

    // Try with /index.html appended for directory routes
    let with_index = if path.ends_with('/') {
        format!("{}index.html", path)
    } else {
        format!("{}/index.html", path)
    };

    if let Some(asset) = ROUTES.get(with_index.as_str()) {
        return Ok(serve_asset(asset, &req, &with_index));
    }

    // Try removing trailing slash and adding index.html
    if path.ends_with('/') && path.len() > 1 {
        let without_slash = &path[..path.len() - 1];
        if let Some(asset) = ROUTES.get(without_slash) {
            return Ok(serve_asset(asset, &req, without_slash));
        }
    }

    // 404 - check if we have a custom 404.html
    if let Some(not_found_asset) = ROUTES.get("/404.html") {
        return Ok(Response::builder()
            .status(StatusCode::NOT_FOUND)
            .header(header::CONTENT_TYPE, not_found_asset.content_type)
            .body(Body::from(not_found_asset.content_raw))
            .unwrap());
    }

    // Default 404
    Ok(Response::builder()
        .status(StatusCode::NOT_FOUND)
        .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .body(Body::from("404 Not Found"))
        .unwrap())
}

fn serve_asset(asset: &Asset, req: &Request<Body>, path: &str) -> Response<Body> {
    // Format ETag with quotes (HTTP spec requires it)
    let etag_value = format!("\"{}\"", asset.etag);

    // Check If-None-Match for 304 Not Modified
    if let Some(client_etag) = req.headers().get(header::IF_NONE_MATCH) {
        if let Ok(etag_str) = client_etag.to_str() {
            if etag_str == etag_value {
                return Response::builder()
                    .status(StatusCode::NOT_MODIFIED)
                    .header(header::ETAG, &etag_value)
                    .body(Body::empty())
                    .unwrap();
            }
        }
    }

    // Content negotiation based on Accept-Encoding
    let accept_encoding = req.headers()
        .get(header::ACCEPT_ENCODING)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let (content, encoding) = if asset.is_compressible {
        // Only negotiate compression for compressible content
        if accept_encoding.contains("br") {
            (asset.content_brotli, "br")
        } else if accept_encoding.contains("gzip") {
            (asset.content_gzip, "gzip")
        } else {
            (asset.content_raw, "identity")
        }
    } else {
        // Serve raw for non-compressible content (images, etc.)
        (asset.content_raw, "identity")
    };

    // Determine cache-control header
    // Hugo fingerprints assets with hashes (e.g., style.min.39e30de...css)
    // These can be cached forever since content changes = new hash = new URL
    let cache_control = if is_fingerprinted(path) {
        "public, max-age=31536000, immutable"
    } else {
        // HTML and other non-fingerprinted content
        "public, max-age=3600"
    };

    let mut response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, asset.content_type)
        .header(header::CACHE_CONTROL, cache_control)
        .header(header::ETAG, &etag_value);

    // Only set content-encoding if we're actually compressing
    if encoding != "identity" {
        response = response.header(header::CONTENT_ENCODING, encoding);
    }

    response
        .body(Body::from(content))
        .unwrap()
}

fn is_fingerprinted(path: &str) -> bool {
    // Hugo fingerprints look like: file.min.HASH.ext
    // Check if path contains ".min." followed by a long hex-like string
    if let Some(min_pos) = path.find(".min.") {
        let after_min = &path[min_pos + 5..];
        // If there's a long alphanumeric segment after .min., it's likely fingerprinted
        if let Some(dot_pos) = after_min.find('.') {
            let hash_part = &after_min[..dot_pos];
            return hash_part.len() > 20 && hash_part.chars().all(|c| c.is_ascii_hexdigit());
        }
    }
    false
}
