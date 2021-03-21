use {
    crate::prelude::*,
    chrono::Duration,
    path_tree::PathTree,
    std::{collections::BTreeMap, io::Cursor, sync::Arc, time::Instant},
    tiny_http::{Header, Request},
};

pub static PAGE_404: &str = r#"<!DOCTYPE html><html><head><title>404 | local archive</title><style>*{transition:all .6s}html{height:100%}body{font-family:sans-serif;color:#888;margin:0}#main{display:table;width:100%;height:100vh;text-align:center}.fof{display:table-cell;vertical-align:middle}.fof h1{font-size:50px;display:inline-block;padding-right:12px;animation:type .5s alternate infinite}@keyframes type{from{box-shadow:inset -3px 0 0 #888}to{box-shadow:inset -3px 0 0 transparent}}</style></head><body><div id="main"><div class="fof"><h1>Error 404</h1></div></div></body></html>"#;
pub static PAGE_503: &str = r#"<!DOCTYPE html><html><head><title>503 | local archive</title><style>*{transition:all .6s}html{height:100%}body{font-family:sans-serif;color:#888;margin:0}#main{display:table;width:100%;height:100vh;text-align:center}.fof{display:table-cell;vertical-align:middle}.fof h1{font-size:50px;display:inline-block;padding-right:12px;animation:type .5s alternate infinite}@keyframes type{from{box-shadow:inset -3px 0 0 #888}to{box-shadow:inset -3px 0 0 transparent}}</style></head><body><div id="main"><div class="fof"><h1>Error 503</h1></div></div></body></html>"#;

macro_rules! res {
    (404) => {
        Response::from_string(PAGE_404)
            .with_header(
                Header::from_bytes(&b"Content-Type"[..], &b"text/html; charset=utf-8"[..]).unwrap(),
            )
            .with_status_code(404)
    };
    (503) => {
        Response::from_string(PAGE_503)
            .with_header(
                Header::from_bytes(&b"Content-Type"[..], &b"text/html; charset=utf-8"[..]).unwrap(),
            )
            .with_status_code(503)
    };
}

pub type Response = tiny_http::Response<Cursor<Vec<u8>>>;

pub struct Context<'s, S> {
    state: Arc<S>,
    query: Option<&'s str>,
    params: Vec<(&'s str, &'s str)>,
}

impl<'s, S> Context<'s, S> {
    pub fn state(&self) -> &S {
        &self.state
    }

    pub fn param(&self, key: &str) -> Option<&'s str> {
        self.params
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, value)| *value)
    }

    pub fn query(&self) -> Option<&'s str> {
        self.query
    }
}

pub trait Route<'r, S>: 'static {
    fn call(&'r self, ctx: Context<'r, S>) -> Result<Response>;
}

pub struct Boxed<S>(Box<dyn for<'r> Route<'r, S>>);

impl<'r, S, T> Route<'r, S> for T
where
    S: 'r,
    T: 'static + Fn(Context<'r, S>) -> Result<Response>,
{
    fn call(&'r self, ctx: Context<'r, S>) -> Result<Response> {
        (self)(ctx)
    }
}

impl<'r, S> Route<'r, S> for Boxed<S>
where
    S: 'static,
{
    fn call(&'r self, ctx: Context<'r, S>) -> Result<Response> {
        self.0.call(ctx)
    }
}

pub struct Handler<S> {
    method: Method,
    route: Boxed<S>,
}

pub fn get<R, S>(route: R) -> Handler<S>
where
    R: for<'r> Route<'r, S>,
{
    Handler {
        method: Method::Get,
        route: Boxed(Box::new(route)),
    }
}

pub fn post<R, S>(route: R) -> Handler<S>
where
    R: for<'r> Route<'r, S>,
{
    Handler {
        method: Method::Post,
        route: Boxed(Box::new(route)),
    }
}

pub struct Router<S> {
    tree: BTreeMap<Method, PathTree<Boxed<S>>>,
    state: Arc<S>,
}

impl<S> Router<S> {
    pub fn new(state: S) -> Self {
        Self {
            tree: BTreeMap::new(),
            state: Arc::new(state),
        }
    }

    pub fn on(mut self, path: &str, handler: Handler<S>) -> Self {
        let node = self
            .tree
            .entry(handler.method)
            .or_insert_with(PathTree::new);

        node.insert(path, handler.route);

        self
    }

    pub fn handle(&mut self, req: Request) -> Result<()>
    where
        S: 'static,
    {
        let earlier = Instant::now();

        let url = req.url().to_string();

        let method = Method::from(req.method());

        log::info!(
            "{} {} {}/{} {} {}",
            "+".bright_black(),
            "+".bright_black(),
            "HTTP".bright_yellow(),
            req.http_version(),
            method.to_colored_string(),
            url.bright_purple(),
        );

        let state = self.state.clone();

        let url = req.url();

        let (url, query) = match url.find('?').map(|i| url.split_at(i)) {
            Some((url, query)) => (url, Some(query)),
            None => (url, None),
        };

        let res = self
            .tree
            .get(&method)
            .and_then(|tree| tree.find(url))
            .map(|(payload, params)| {
                payload.call(Context {
                    state,
                    query,
                    params,
                })
            })
            .unwrap_or_else(|| Ok(res!(404)))
            .map_err(|err| {
                for cause in err.chain() {
                    log::error!("  {} {}", "|".bright_black(), cause,);
                }

                res!(503)
            })
            .ignore();

        let dur = Duration::from_std(Instant::now().duration_since(earlier))?;

        log::info!(
            "{} {} {} {}ms",
            "+".bright_black(),
            "+".bright_black(),
            match res.status_code().0 {
                200 => format!("{}", "200".green()),
                404 => format!("{}", "404".bright_yellow()),
                503 => format!("{}", "200".bright_red()),
                code => format!("{}", code.to_string().bright_blue()),
            },
            dur.num_milliseconds().bright_purple(),
        );

        req.respond(res)?;

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum Method {
    Get,
    Head,
    Post,
    Put,
    Delete,
    Connect,
    Options,
    Trace,
    Patch,
}

impl Method {
    fn to_colored_string(&self) -> String {
        match self {
            Method::Get => format!("{}", "GET".green()),
            Method::Post => format!("{}", "POST".bright_blue()),
            Method::Put => format!("{}", "PUT".bright_purple()),
            Method::Patch => format!("{}", "PATCH".bright_yellow()),
            Method::Delete => format!("{}", "DELETE".bright_red()),
            Method::Head => "HEAD".to_string(),
            Method::Connect => "CONNECT".to_string(),
            Method::Options => "OPTION".to_string(),
            Method::Trace => "TRACE".to_string(),
        }
    }
}

impl From<tiny_http::Method> for Method {
    fn from(method: tiny_http::Method) -> Self {
        Method::from(&method)
    }
}

impl From<&tiny_http::Method> for Method {
    fn from(method: &tiny_http::Method) -> Self {
        match method {
            tiny_http::Method::Get => Method::Get,
            tiny_http::Method::Head => Method::Head,
            tiny_http::Method::Post => Method::Post,
            tiny_http::Method::Put => Method::Put,
            tiny_http::Method::Delete => Method::Delete,
            tiny_http::Method::Connect => Method::Connect,
            tiny_http::Method::Options => Method::Options,
            tiny_http::Method::Trace => Method::Trace,
            tiny_http::Method::Patch => Method::Patch,
            tiny_http::Method::NonStandard(_) => Method::Get,
        }
    }
}

trait ResultExt<T> {
    fn ignore(self) -> T;
}

impl<T> ResultExt<T> for Result<T, T> {
    fn ignore(self) -> T {
        match self {
            Ok(t) => t,
            Err(t) => t,
        }
    }
}