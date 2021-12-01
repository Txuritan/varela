#![feature(decl_macro)]

use enrgy::{
    dev::Extensions,
    http::{
        headers::{
            ACCEPT, ACCEPT_CHARSET, ACCEPT_ENCODING, ACCEPT_LANGUAGE, CONNECTION, HOST, KEEP_ALIVE,
            USER_AGENT,
        },
        *,
    },
};

macro http_test($name:ident, $exp:expr, $got:expr,) {
    #[test]
    fn $name() {
        assert_eq!($exp, $got);
    }
}

macro request {
    // end
    (@inner,
        ($m:ident, $u:ident, $v:ident, $h:ident),
        [ $( $building:tt )* ],
        $body:block,
    ) => {{
        $( $building )*

        HttpRequest2 {
            method: $m,
            uri: $u,
            version: $v,
            headers: $h,
            extensions: Extensions::new(),
            body: $body,
        }
    }},

    // header
    (@inner,
        ($m:ident, $u:ident, $v:ident, $h:ident),
        [ $( $building:tt )* ],
        $header:ident => $value:expr;
        $( $rest:tt )*
    ) => {
        request!(
            @inner,
            ($m, $u, $v, $h),
            [
                $( $building )*
                $h.insert($header, $value.to_string());
            ],
            $( $rest )*
        )
    },

    // entry point
    (
        ($m:ident, $u:ident, $v:ident, $h:ident),
        $method:ident, $uri:expr, $version:ident;
        $( $rest:tt )*
    ) => {
        request!(
            @inner,
            ($m, $u, $v, $h),
            [
                let $m = HttpMethod::$method;
                let $u = $uri.to_string();
                let $v = HttpVersion::$version;
                let mut $h = HttpHeaders::new();
            ],
            $( $rest )*
        )
    },
}

macro raw($( $raw:expr )*) {
    HttpRequest2::from_buf_reader(&mut std::io::Cursor::new(
        concat!($( $raw ),*).as_bytes(),
    )).unwrap()
}

http_test!(
    test_curl_get,
    request!((m, u, v, h),
        Get, "/test", Http11;
        USER_AGENT => "curl/7.18.0 (i486-pc-linux-gnu) libcurl/7.18.0 OpenSSL/0.9.8g zlib/1.2.3.3 libidn/1.1";
        HOST => "0.0.0.0=5000";
        ACCEPT => "*/*";
        { HttpBody::None },
    ),
    raw!(
        "GET /test HTTP/1.1\r\n"
        "User-Agent: curl/7.18.0 (i486-pc-linux-gnu) libcurl/7.18.0 OpenSSL/0.9.8g zlib/1.2.3.3 libidn/1.1\r\n"
        "Host: 0.0.0.0=5000\r\n"
        "Accept: */*\r\n"
        "\r\n"
    ),
);

http_test!(
    test_firefox_get,
    request!((m, u, v, h),
        Get, "/favicon.ico", Http11;
        HOST => "0.0.0.0=5000";
        USER_AGENT => "Mozilla/5.0 (X11; U; Linux i686; en-US; rv:1.9) Gecko/2008061015 Firefox/3.0";
        ACCEPT => "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8";
        ACCEPT_LANGUAGE => "en-us,en;q=0.5";
        ACCEPT_ENCODING => "gzip,deflate";
        ACCEPT_CHARSET => "ISO-8859-1,utf-8;q=0.7,*;q=0.7";
        KEEP_ALIVE => "300";
        CONNECTION => "keep-alive";
        { HttpBody::None },
    ),
    raw!(
        "GET /favicon.ico HTTP/1.1\r\n"
        "Host: 0.0.0.0=5000\r\n"
        "User-Agent: Mozilla/5.0 (X11; U; Linux i686; en-US; rv:1.9) Gecko/2008061015 Firefox/3.0\r\n"
        "Accept: text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8\r\n"
        "Accept-Language: en-us,en;q=0.5\r\n"
        "Accept-Encoding: gzip,deflate\r\n"
        "Accept-Charset: ISO-8859-1,utf-8;q=0.7,*;q=0.7\r\n"
        "Keep-Alive: 300\r\n"
        "Connection: keep-alive\r\n"
        "\r\n"
    ),
);
