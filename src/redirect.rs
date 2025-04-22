use bytes::Bytes;
use http::{HeaderValue, Response, StatusCode, header};
use http_body_util::Full;

pub struct Redirect {
    status_code: StatusCode,
    location: HeaderValue,
}

impl Redirect {
    pub fn to<T: AsRef<str>>(url: T) -> Self {
        Self {
            status_code: StatusCode::SEE_OTHER,
            location: HeaderValue::try_from(url.as_ref())
                .expect("Url is not a valid  header value"),
        }
    }
}

impl From<Redirect> for Response<Full<Bytes>> {
    fn from(value: Redirect) -> Self {
        Response::builder()
            .status(value.status_code)
            .header(header::LOCATION, value.location)
            .body(Full::new(Bytes::new()))
            .unwrap_or_default()
    }
}
