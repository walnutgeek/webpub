use crate::server::storage::Storage;
use crate::Node;
use axum::{
    body::Body,
    extract::{Host, Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use std::sync::Arc;

pub struct AppState {
    pub storage: Arc<Storage>,
}

pub fn create_router(storage: Arc<Storage>) -> Router {
    let state = AppState { storage };

    Router::new()
        .route("/", get(handle_request))
        .route("/*path", get(handle_request))
        .with_state(Arc::new(state))
}

async fn handle_request(
    State(state): State<Arc<AppState>>,
    Host(host): Host,
    path: Option<Path<String>>,
) -> Response {
    let path_str = path
        .map(|p| format!("/{}", p.0))
        .unwrap_or_else(|| "/".to_string());

    // Strip port from host if present
    let hostname = host.split(':').next().unwrap_or(&host);

    // Get current snapshot for this host
    let snapshot = match state.storage.get_current_snapshot(hostname) {
        Ok(Some((_, tree))) => tree,
        Ok(None) => return (StatusCode::NOT_FOUND, "Site not found").into_response(),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };

    // Find the node for this path
    let node = match find_node(&snapshot, &path_str) {
        Some(n) => n,
        None => return (StatusCode::NOT_FOUND, "Not found").into_response(),
    };

    // Must be a file
    let (chunks, name) = match node {
        Node::File { chunks, name, .. } => (chunks, name),
        Node::Directory { .. } => {
            // Try index.html
            let index_path = if path_str.ends_with('/') {
                format!("{}index.html", path_str)
            } else {
                format!("{}/index.html", path_str)
            };
            if let Some(Node::File { chunks, name, .. }) = find_node(&snapshot, &index_path) {
                (chunks, name)
            } else {
                return (StatusCode::NOT_FOUND, "Not found").into_response();
            }
        }
    };

    // Reassemble file from chunks
    let mut data = Vec::new();
    for hash in chunks {
        match state.storage.get_chunk(hash) {
            Ok(Some(chunk_data)) => data.extend(chunk_data),
            Ok(None) => {
                return (StatusCode::INTERNAL_SERVER_ERROR, "Missing chunk").into_response()
            }
            Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
        }
    }

    // Guess content type from extension
    let content_type = mime_guess::from_path(name)
        .first_or_octet_stream()
        .to_string();

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .body(Body::from(data))
        .unwrap()
}

pub fn find_node<'a>(tree: &'a Node, path: &str) -> Option<&'a Node> {
    let path = path.trim_start_matches('/');

    if path.is_empty() || path == "/" {
        // Root directory - look for index.html
        if let Node::Directory { children, .. } = tree {
            return children.iter().find(|c| c.name() == "index.html");
        }
        return None;
    }

    let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    find_node_recursive(tree, &parts)
}

fn find_node_recursive<'a>(node: &'a Node, parts: &[&str]) -> Option<&'a Node> {
    if parts.is_empty() {
        return Some(node);
    }

    match node {
        Node::Directory { children, .. } => {
            for child in children {
                if child.name() == parts[0] {
                    return find_node_recursive(child, &parts[1..]);
                }
            }
            None
        }
        Node::File { .. } => None,
    }
}
