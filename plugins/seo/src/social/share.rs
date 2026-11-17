//! The share image of a page, fetched through the host's `http` capability (its address rule,
//! rate limit and size cap apply), its redirects followed as a page's are, its format and size
//! read from its header.

use rb_plugin_api::PluginHost;

use crate::fetch::{self, ACCEPT_IMAGE, MAX_HOPS, Stop};
use crate::social::image::{self, Image};
use crate::url::Url;

/// A share image as it answered.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Loaded {
    /// The URL that answered (after the redirects).
    pub url: String,
    /// The final status; `None` when no answer came.
    pub status: Option<u16>,
    /// Why no answer came: the host's failure code, `loop` or `location`.
    pub failure: Option<String>,
    pub content_type: Option<String>,
    /// The sniffed image; `None` for anything that is none.
    pub image: Option<Image>,
    pub body: Vec<u8>,
}

impl Loaded {
    /// Whether the image answered 200.
    #[must_use]
    pub fn ok(&self) -> bool {
        self.status == Some(200)
    }

    /// Whether the answer says and is an image: an `image/*` type (or none) and bytes that sniff
    /// as one.
    #[must_use]
    pub fn is_image(&self) -> bool {
        self.image.is_some()
            && self
                .content_type
                .as_deref()
                .is_none_or(|kind| kind.trim().to_ascii_lowercase().starts_with("image/"))
    }
}

/// `url` fetched.
pub fn load(host: &mut dyn PluginHost, url: &Url) -> Loaded {
    let fetched = fetch::follow_with(host, url, MAX_HOPS, ACCEPT_IMAGE);
    let failure = fetched.stop.map(|stop| match stop {
        Stop::Failed(code) => code,
        Stop::Loop => "loop".to_owned(),
        Stop::BadLocation(_) => "location".to_owned(),
    });
    let Some(response) = fetched.response else {
        return Loaded {
            url: url.to_string(),
            failure,
            ..Loaded::default()
        };
    };
    let content_type = response.header("content-type").next().map(str::to_owned);
    Loaded {
        status: Some(response.status),
        failure,
        content_type,
        image: image::sniff(&response.body),
        url: response.url,
        body: response.body,
    }
}

#[cfg(test)]
#[path = "share_tests.rs"]
mod tests;
