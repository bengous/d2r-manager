pub mod failure;
mod handler;
pub mod payload;
mod query;
pub mod route;

use std::fmt;
use std::io::Read;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4};

use thiserror::Error;
use tiny_http::{Header, Request, Response};

use app::ports::{Clock, Files, Locks, Mirrors, Processes, Repository, Timers};

use failure::{Failure, Kind};
use route::{Match, Method, look_up};

pub const BODY_LIMIT: usize = 1 << 20;
pub const DEFAULT_PORT: u16 = 7105;

#[derive(Debug, Clone, Copy)]
pub struct Services<'a> {
    pub files: &'a dyn Files,
    pub locks: &'a dyn Locks,
    pub processes: &'a dyn Processes,
    pub mirrors: &'a dyn Mirrors,
    pub repository: &'a dyn Repository,
    pub timers: &'a dyn Timers,
    pub clock: &'a dyn Clock,
}

#[derive(Debug, Error)]
pub enum ServeError {
    #[error("the loopback address {address} takes no listener: {reason}")]
    Bind { address: SocketAddr, reason: String },

    #[error("the listening socket carries no address")]
    Unbound,

    #[error("the next request never arrives: {reason}")]
    Accept { reason: String },

    #[error("the answer never leaves: {reason}")]
    Answer { reason: String },
}

pub struct Server {
    http: tiny_http::Server,
    address: SocketAddr,
}

impl fmt::Debug for Server {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Server")
            .field("address", &self.address)
            .field("http", &format_args!("listening"))
            .finish()
    }
}

impl Server {
    pub fn bind(port: u16) -> Result<Self, ServeError> {
        let asked = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port));
        let http = tiny_http::Server::http(asked).map_err(|reason| ServeError::Bind {
            address: asked,
            reason: reason.to_string(),
        })?;
        let address = http.server_addr().to_ip().ok_or(ServeError::Unbound)?;
        Ok(Self { http, address })
    }

    #[must_use]
    pub const fn address(&self) -> SocketAddr {
        self.address
    }

    #[must_use]
    pub fn serve(&self, services: &Services<'_>) -> ServeError {
        loop {
            if let Err(error) = self.serve_once(services) {
                return error;
            }
        }
    }

    pub fn serve_once(&self, services: &Services<'_>) -> Result<(), ServeError> {
        let mut request = self.http.recv().map_err(|reason| ServeError::Accept {
            reason: reason.to_string(),
        })?;
        let (status, body) = match handle(&mut request, *services) {
            Ok(body) => (200, body),
            Err(failure) => (failure.status(), failure.body()),
        };
        request
            .respond(
                Response::from_string(body)
                    .with_status_code(status)
                    .with_header(json()),
            )
            .map_err(|reason| ServeError::Answer {
                reason: reason.to_string(),
            })
    }
}

fn handle(request: &mut Request, services: Services<'_>) -> Result<String, Failure> {
    from_this_machine(request)?;
    let url = request.url().to_owned();
    let (path, query) = url.split_once('?').unwrap_or((url.as_str(), ""));
    let route = match look_up(method(request), path) {
        Match::Found(route) => route,
        Match::OtherMethod => {
            return Err(Failure::new(
                Kind::MethodNotAllowed,
                format!("{path} answers no {} request", request.method()),
            ));
        }
        Match::Absent => {
            return Err(Failure::new(
                Kind::NoSuchRoute,
                format!("{path} is no route this API serves"),
            ));
        }
    };
    let body = body(request)?;
    handler::answer(services, route.target(), query, &body)
}

fn method(request: &Request) -> Option<Method> {
    match request.method() {
        tiny_http::Method::Get => Some(Method::Get),
        tiny_http::Method::Post => Some(Method::Post),
        _ => None,
    }
}

fn body(request: &mut Request) -> Result<Vec<u8>, Failure> {
    let too_large = || {
        Failure::new(
            Kind::PayloadTooLarge,
            format!("a request body holds at most {BODY_LIMIT} bytes"),
        )
    };
    if request
        .body_length()
        .is_some_and(|length| length > BODY_LIMIT)
    {
        return Err(too_large());
    }
    let mut body = Vec::new();
    request
        .as_reader()
        .take(BODY_LIMIT as u64 + 1)
        .read_to_end(&mut body)
        .map_err(|reason| {
            Failure::new(
                Kind::BadRequest,
                format!("the request body never arrives: {reason}"),
            )
        })?;
    if body.len() > BODY_LIMIT {
        return Err(too_large());
    }
    Ok(body)
}

fn from_this_machine(request: &Request) -> Result<(), Failure> {
    let header = |name: &'static str| {
        request
            .headers()
            .iter()
            .find(|header| header.field.equiv(name))
            .map(|header| header.value.as_str())
    };
    if let Some(value) = header("Origin")
        && !origin(value).is_some_and(loopback)
    {
        return Err(elsewhere("Origin", value));
    }
    if let Some(value) = header("Host")
        && !loopback(value)
    {
        return Err(elsewhere("Host", value));
    }
    Ok(())
}

fn elsewhere(field: &str, value: &str) -> Failure {
    Failure::new(
        Kind::ForbiddenOrigin,
        format!("the {field} {value:?} is no name of this machine"),
    )
}

fn origin(value: &str) -> Option<&str> {
    value
        .strip_prefix("http://")
        .or_else(|| value.strip_prefix("https://"))
}

fn loopback(authority: &str) -> bool {
    let host = match authority.strip_prefix('[') {
        Some(inside) => inside.split(']').next().unwrap_or_default(),
        None => authority.split(['/', ':']).next().unwrap_or_default(),
    };
    host == "localhost"
        || host
            .parse::<IpAddr>()
            .is_ok_and(|address| address.is_loopback())
}

fn json() -> Header {
    Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..])
        .expect("the content type of every answer is well formed")
}
