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
  headers.set(header::UserAgent::new(concat!(
    env!("CARGO_PKG_NAME"),
    "/",
    env!("CARGO_PKG_VERSION"),
    " (",
    env!("SOURCE_VERSION"),
    ")"
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

  #[test]
  fn refurb_returns_valid_feed() {
    let server_host = format!("http://{}", mockito::SERVER_ADDRESS);
    let feed_path = "/feed.rss";
    let article_path = "/articles/latest-cool-article-123";

    let feed_request = mockito::mock("GET", feed_path)
      .with_header("content-type", "application/xml")
      .with_body(&format!(
        include_str!("../tests/fixtures/refurb_returns_valid_feed/source-feed.interpolated.xml"),
        hostname = mockito::SERVER_ADDRESS,
        host = server_host,
        article = article_path,
      )).create();

    let article_request = mockito::mock("GET", article_path)
      .with_header("content-type", "text/html")
      .with_body(&include_str!(
        "../tests/fixtures/refurb_returns_valid_feed/latest-cool-article-123.html"
      )).create();

    let client = Client::new(application()).expect("valid rocket instance");

    let mut response = client
      .get(format!(
        "/refurb?feed={}{}&description_selector=.main-image,article",
        server_host, feed_path
      )).dispatch();

    assert_eq!(response.status(), Status::Ok);

    assert_eq!(
      response
        .body_string()
        .expect("processed feed response body"),
      format!(
        include_str!("../tests/fixtures/refurb_returns_valid_feed/expected-feed.interpolated.xml"),
        hostname = mockito::SERVER_ADDRESS,
        host = server_host,
        article = article_path,
      )
    );

    feed_request.assert();
    article_request.assert();
  }
}
