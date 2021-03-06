// Copyright 2015-2017 Parity Technologies (UK) Ltd.
// This file is part of Parity.

// Parity is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Parity is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Parity.  If not, see <http://www.gnu.org/licenses/>.

//! Hyper handlers implementations.

mod content;
mod echo;
mod fetch;
mod redirect;
mod streaming;

pub use self::content::ContentHandler;
pub use self::echo::EchoHandler;
pub use self::fetch::{ContentFetcherHandler, ContentValidator, FetchControl, ValidatorResponse};
pub use self::redirect::Redirection;
pub use self::streaming::StreamingHandler;

use url::Url;
use hyper::{server, header, net, uri};
use address;

/// Adds security-related headers to the Response.
pub fn add_security_headers(headers: &mut header::Headers, embeddable_on: Option<(String, u16)>) {
	headers.set_raw("X-XSS-Protection", vec![b"1; mode=block".to_vec()]);
	headers.set_raw("X-Content-Type-Options", vec![b"nosniff".to_vec()]);

	// Embedding header:
	if let Some(ref embeddable_on) = embeddable_on {
		headers.set_raw("X-Frame-Options", vec![
			format!("ALLOW-FROM http://{}", address(embeddable_on)).into_bytes()
		]);
	} else {
		// TODO [ToDr] Should we be more strict here (DENY?)?
		headers.set_raw("X-Frame-Options",  vec![b"SAMEORIGIN".to_vec()]);
	}

	// Content Security Policy headers
	headers.set_raw("Content-Security-Policy", vec![
		// Allow connecting to WS servers and HTTP(S) servers.
		// We could be more restrictive and allow only RPC server URL.
		b"connect-src http: https: ws: wss:;".to_vec(),
		// Allow framing any content from HTTP(S).
		// Again we could only allow embedding from RPC server URL.
		// (deprecated)
		b"frame-src 'self' http: https:;".to_vec(),
		// Allow framing and web workers from HTTP(S).
		b"child-src 'self' http: https:;".to_vec(),
		// We allow data: blob: and HTTP(s) images.
		// We could get rid of wildcarding HTTP and only allow RPC server URL.
		// (http require for local dapps icons)
		b"img-src 'self' 'unsafe-inline' data: blob: http: https:;".to_vec(),
		// Allow style from data: blob: and HTTPS.
		b"style-src 'self' 'unsafe-inline' data: blob: https:;".to_vec(),
		// Allow fonts from data: and HTTPS.
		b"font-src 'self' data: https:;".to_vec(),
		// Allow inline scripts and scripts eval (webpack/jsconsole)
		b"script-src 'self' 'unsafe-inline' 'unsafe-eval';".to_vec(),
		// Restrict everything else to the same origin.
		b"default-src 'self';".to_vec(),
		// Run in sandbox mode (although it's not fully safe since we allow same-origin and script)
		b"sandbox allow-same-origin allow-forms allow-modals allow-popups allow-presentation allow-scripts;".to_vec(),
		// Disallow subitting forms from any dapps
		b"form-action 'none';".to_vec(),
		// Never allow mixed content
		b"block-all-mixed-content;".to_vec(),
		// Specify if the site can be embedded.
		match embeddable_on {
			Some((ref host, ref port)) if host == "127.0.0.1" => {
				format!("frame-ancestors {} {};", address(&(host.to_owned(), *port)), address(&("localhost".to_owned(), *port)))
			},
			Some(ref embed) => format!("frame-ancestors {};", address(embed)),
			None => format!("frame-ancestors 'self';"),
		}.into_bytes(),
	]);
}


/// Extracts URL part from the Request.
pub fn extract_url(req: &server::Request<net::HttpStream>) -> Option<Url> {
	convert_uri_to_url(req.uri(), req.headers().get::<header::Host>())
}

/// Extracts URL given URI and Host header.
pub fn convert_uri_to_url(uri: &uri::RequestUri, host: Option<&header::Host>) -> Option<Url> {
	match *uri {
		uri::RequestUri::AbsoluteUri(ref url) => {
			match Url::from_generic_url(url.clone()) {
				Ok(url) => Some(url),
				_ => None,
			}
		},
		uri::RequestUri::AbsolutePath { ref path, ref query } => {
			let query = match *query {
				Some(ref query) => format!("?{}", query),
				None => "".into(),
			};
			// Attempt to prepend the Host header (mandatory in HTTP/1.1)
			let url_string = match host {
				Some(ref host) => {
					format!("http://{}:{}{}{}", host.hostname, host.port.unwrap_or(80), path, query)
				},
				None => return None,
			};

			match Url::parse(&url_string) {
				Ok(url) => Some(url),
				_ => None,
			}
		},
		_ => None,
	}
}

