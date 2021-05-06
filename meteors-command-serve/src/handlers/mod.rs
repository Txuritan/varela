mod index;
mod search;
mod story;

pub use {
    crate::{
        handlers::{index::index, search::search, story::story},
        router::{Context, Header, HeaderField, Response, StatusCode},
    },
    common::{database::Database, prelude::*},
    std::io::Cursor,
};

// use {crate::views::Layout, sailfish::TemplateOnce};

#[macro_export]
macro_rules! res {
    (200; $body:expr) => {
        $crate::router::Response::from_string(::opal::Template::render_into_string($body)?)
            .with_header(
                ::tiny_http::Header::from_bytes(
                    &b"Content-Type"[..],
                    &b"text/html; charset=utf-8"[..],
                )
                .unwrap(),
            )
            .with_status_code(200)
    };
}

pub fn style(ctx: &Context<'_, Database>) -> Result<Response> {
    static CSS: &str = include_str!("../../assets/style.css");
    // RELEASE: change anytime theres a release and the style gets updated
    static CSS_TAG: &str = "f621e1d55cbee8397c906c7d72d0fb9a4520a06be6218abeccff1ffcf75f00b3";

    let mut headers = Vec::with_capacity(16);

    headers
        .push(Header::from_bytes(&b"Content-Type"[..], &b"text/css; charset=utf-8"[..]).unwrap());

    if !cfg!(debug_assertions) {
        headers.push(
            Header::from_bytes(&b"Cache-Control"[..], &b"public; max-age=31536000"[..]).unwrap(),
        );

        headers.push(Header::from_bytes(&b"ETag"[..], CSS_TAG).unwrap());
    }

    let target_header = HeaderField::from_bytes(&b"If-None-Match"[..])?;
    let header = ctx
        .headers
        .iter()
        .find(|header| header.field == target_header);

    if let Some(header) = header {
        if header.value == CSS_TAG {
            return Ok(Response::new(
                StatusCode(304),
                headers,
                Cursor::new(vec![]),
                None,
                None,
            ));
        }
    }

    Ok(Response::new(
        StatusCode(200),
        headers,
        Cursor::new(CSS.as_bytes().to_vec()),
        Some(CSS.len()),
        None,
    ))
}

// pub trait Res {
//     fn response(self) -> String;
// }

// impl<'s> Res for &'s str {
//     fn response(self) -> String {
//         self.to_string()
//     }
// }

// impl Res for String {
//     fn response(self) -> String {
//         self
//     }
// }

// impl<I> Res for Layout<I>
// where
//     I: TemplateOnce,
// {
//     fn response(self) -> String {
//         self.render_once().expect("unable to render template")
//     }
// }
