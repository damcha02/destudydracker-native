//! Requests leaving a controller and replies coming back (Stage 22a). Controllers never touch
//! the network or Slint: they return [`Outgoing`] values tagged with a token and receive
//! [`NetReply`] values for those tokens. The app glue (`app_net`) runs them on the network
//! worker; tests run them synchronously (or not at all) - so every controller is testable
//! headlessly and deterministically.

use crate::net::http::{ApiRequest, HttpResponse, NetError};
use crate::net::images::{DecodedImage, ImageKind};
use crate::net::worker::CancelToken;

/// Work done on the network thread after the response arrived (never on the UI thread).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Post {
    None,
    DecodeImage {
        kind: ImageKind,
        max_w: u32,
        max_h: u32,
    },
}

#[derive(Debug)]
pub struct Outgoing {
    pub token: u64,
    pub request: ApiRequest,
    pub cancel: CancelToken,
    pub post: Post,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetReply {
    Http(Result<HttpResponse, NetError>),
    Image(Result<DecodedImage, NetError>),
}

impl NetReply {
    /// Applies `post` to a transport result (runs on the network thread).
    pub fn from_result(result: Result<HttpResponse, NetError>, post: Post) -> Self {
        match post {
            Post::None => Self::Http(result),
            Post::DecodeImage { kind, max_w, max_h } => Self::Image(result.and_then(|r| {
                if !(200..300).contains(&r.status) {
                    return Err(NetError::from_status(r.status, &r.body));
                }
                crate::net::images::decode(kind, r.content_type.as_deref(), &r.body, max_w, max_h)
            })),
        }
    }

    pub fn http(self) -> Result<HttpResponse, NetError> {
        match self {
            Self::Http(r) => r,
            Self::Image(_) => Err(NetError::Malformed),
        }
    }

    pub fn image(self) -> Result<DecodedImage, NetError> {
        match self {
            Self::Image(r) => r,
            Self::Http(_) => Err(NetError::Malformed),
        }
    }
}

/// Token allocation shared by a controller's requests.
#[derive(Debug, Default)]
pub struct Tokens(u64);

impl Tokens {
    pub fn next(&mut self) -> u64 {
        self.0 += 1;
        self.0
    }
}
