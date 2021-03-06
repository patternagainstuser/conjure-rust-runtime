// Copyright 2020 Palantir Technologies, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
use crate::blocking::runtime;
use bytes::Bytes;
use futures::executor;
use hyper::{HeaderMap, StatusCode};
use std::io::{self, Read};
use tokio::io::AsyncReadExt;

/// A blocking HTTP response.
pub struct Response(crate::Response);

impl Response {
    pub(crate) fn new(inner: crate::Response) -> Response {
        Response(inner)
    }

    /// Returns the response's status.
    pub fn status(&self) -> StatusCode {
        self.0.status()
    }

    /// Returns the response's headers.
    pub fn headers(&self) -> &HeaderMap {
        self.0.headers()
    }

    /// Consumes the response, returning its body.
    pub fn into_body(self) -> ResponseBody {
        ResponseBody(self.0.into_body())
    }
}

/// A blocking streaming response body.
pub struct ResponseBody(crate::ResponseBody);

impl ResponseBody {
    /// Reads the next chunk of bytes from the response.
    ///
    /// Compared to the `Read` implementation, this method avoids some copies of the body data when working with an API
    /// that already consumes `Bytes` objects.
    pub fn read_bytes(&mut self) -> io::Result<Option<Bytes>> {
        runtime()?.enter(|| executor::block_on(self.0.read_bytes()))
    }
}

impl Read for ResponseBody {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        runtime()?.enter(|| executor::block_on(self.0.read(buf)))
    }
}

// FIXME implement BufRead
