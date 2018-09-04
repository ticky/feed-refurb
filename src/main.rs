#![feature(plugin)]
#![plugin(rocket_codegen)]
#![feature(custom_derive)]

extern crate feed_refurb;
extern crate kuchiki;
#[cfg(test)]
#[macro_use]
extern crate pretty_assertions;
extern crate reqwest;
extern crate rocket;
extern crate rocket_contrib;
extern crate rss;

use kuchiki::Selectors;
use rocket::http::RawStr;
use rocket::request::FromFormValue;
use rocket::response::content::Xml;
use rocket::response::Failure;
use rocket::{Request, State};
use rocket_contrib::Template;

const NAME: &'static str = env!("CARGO_PKG_NAME");
const VERSION: &'static str = env!("CARGO_PKG_VERSION");
const SOURCE_VERSION: &'static str = env!("SOURCE_VERSION");

#[get("/")]
fn index() -> Template {
  let mut map = std::collections::HashMap::new();
  map.insert("path", "/");
  Template::render("index", &map)
}

struct HTTPClient {
  client: reqwest::Client,
}

struct CSSSelector(Selectors);

impl<'v> FromFormValue<'v> for CSSSelector {
  type Error = &'v RawStr;

  fn from_form_value(form_value: &'v RawStr) -> Result<CSSSelector, &'v RawStr> {
    form_value
      .url_decode()
      .or_else(|_| Err(form_value))
      .and_then(|decoded| {
        Selectors::compile(&decoded)
          .or_else(|_| Err(form_value))
          .and_then(|selector| Ok(CSSSelector(selector)))
      })
  }
}

#[derive(FromForm)]
struct FeedConfiguration {
  feed: String,
  description_selector: CSSSelector,
}

#[get("/refurb?<configuration>")]
fn refurb(
  configuration: FeedConfiguration,
  http_client: State<HTTPClient>,
) -> Result<Xml<String>, Failure> {
  return match feed_refurb::refurb(
    configuration.feed,
    configuration.description_selector.0,
    &http_client.client,
  ) {
    Ok(feed) => Ok(Xml(feed.to_string())),
    Err(_error) => {
      // TODO: Handle specific errors
      return Err(Failure(rocket::http::Status::NotAcceptable));
    }
  };
}

#[catch(404)]
fn not_found(req: &Request) -> Template {
  let mut map = std::collections::HashMap::new();
  map.insert("path", req.uri().as_str());
  Template::render("error/404", &map)
}

fn shared_http_client() -> HTTPClient {
  use reqwest::header;
  use reqwest::Client;

  let mut headers = header::Headers::new();
  headers.set(header::UserAgent::new(format!(
    "{}/{} ({})",
    NAME, VERSION, SOURCE_VERSION
  )));

  HTTPClient {
    client: Client::builder().default_headers(headers).build().unwrap(),
  }
}

fn application() -> rocket::Rocket {
  rocket::ignite()
    .attach(Template::fairing())
    .manage(shared_http_client())
    .mount("/", routes![index, refurb])
    .catch(catchers![not_found])
}

fn main() {
  application().launch();
}

#[cfg(test)]
mod test {
  extern crate mockito;

  use application;
  use rocket::http::Status;
  use rocket::local::Client;

  #[test]
  fn index_page() {
    let client = Client::new(application()).expect("valid rocket instance");
    let response = client.get("/").dispatch();
    assert_eq!(response.status(), Status::Ok);
  }

  #[test]
  fn not_found() {
    let client = Client::new(application()).expect("valid rocket instance");
    let response = client
      .get("/not-a-valid-url-in-a-million-years-i-promise")
      .dispatch();
    assert_eq!(response.status(), Status::NotFound);
  }

  fn mock_get_request(path: &str, content_type: &str, body: &str) -> mockito::Mock {
    mockito::mock("GET", path)
      .with_header("content-type", content_type)
      .with_body(body)
      .create()
  }

  #[test]
  fn refurb() {
    let server_host = format!("http://{}", mockito::SERVER_ADDRESS);
    let feed_path = "/feed.rss";
    let article_path = "/articles/latest-cool-article-123";

    let feed_request = mock_get_request(
      feed_path,
      "application/xml",
      &format!(
        r#"<?xml version="1.0" encoding="UTF-8" ?>
  <rss version="2.0">
  <channel>
    <title>My Test Feed</title>
    <link>{host}</link>
    <description>Test feed for testing purposes!</description>
    <language>en-us</language>
    <item>
      <title><![CDATA[My Latest Cool Article]]></title>
      <description><![CDATA[bad and not good article summary, click for more]]></description>
      <link>{host}{article}</link>
      <author>webmaster@{hostname}</author>
      <pubDate>Fri, 31 Aug 2018 03:59:52 +0000</pubDate>
      <guid>{host}{article}</guid>
    </item>
  </channel>
</rss>
"#,
        hostname = mockito::SERVER_ADDRESS,
        host = server_host,
        article = article_path,
      ),
    );

    let article_request = mock_get_request(article_path, "text/html", &r#"<!DOCTYPE html>
<head>
  <title>My Latest Cool Article</title>
  <meta name="viewport" content="width=device-width, initial-scale=1">
</head>
<body>
  <nav>
    <a href="/">Home</a>
    <a href="/articles">Articles</a>
  </nav>
  <section>
    <h1>My Latest Cool Article</h1>
    <img class="main-image" src="/images/latest-cool-article-123-main.jpg" />
    <article>
      <p>Here is my latest cool article. It's very good and full of cool and useful information.</p>
      <p>My RSS feed is truncated, so you'd better click my links! I'd better see you in Google Analytics.</p>
    </article>
  </section>
</body>"#);

    let client = Client::new(application()).expect("valid rocket instance");

    let mut response = client
      .get(format!(
        "/refurb?feed={}{}&description_selector=.main-image,article",
        server_host, feed_path
      )).dispatch();

    assert_eq!(response.status(), Status::Ok);
    assert_eq!(
      response.body_string().expect("processed feed response body"),
      format!(
        r#"<rss version="2.0"><channel><title>My Test Feed</title><link>{host}</link><description>Test feed for testing purposes!</description><language>en-us</language><item><title>My Latest Cool Article</title><link>{host}{article}</link><description><![CDATA[<img class="main-image" src="/images/latest-cool-article-123-main.jpg"><br><article>
      <p>Here is my latest cool article.It's very good and full of cool and useful information.</p>
      <p>My RSS feed is truncated, so you'd better click my links! I'd better see you in Google Analytics.</p>
    </article>]]></description><author>webmaster@{hostname}</author><guid>{host}{article}</guid><pubDate>Fri, 31 Aug 2018 03:59:52 +0000</pubDate></item></channel></rss>"#,
        hostname = mockito::SERVER_ADDRESS,
        host = server_host,
        article = article_path,
      )
    );

    feed_request.assert();
    article_request.assert();
  }
}
