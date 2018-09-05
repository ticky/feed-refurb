extern crate kuchiki;
extern crate mockito;
#[macro_use]
extern crate pretty_assertions;
extern crate reqwest;
extern crate rss;

use kuchiki::Selectors;

extern crate feed_refurb;
use feed_refurb::refurb;
use feed_refurb::Error;

#[test]
fn refurb_returns_valid_feed() {
  let server_host = format!("http://{}", mockito::SERVER_ADDRESS);
  let feed_path = "/feed.rss";
  let article_path = "/articles/latest-cool-article-123";

  let feed_request = mockito::mock("GET", feed_path)
    .with_header("content-type", "application/xml")
    .with_body(&format!(
      include_str!("fixtures/refurb_returns_valid_feed/source-feed.interpolated.xml"),
      hostname = mockito::SERVER_ADDRESS,
      host = server_host,
      article = article_path,
    )).create();

  let article_request = mockito::mock("GET", article_path)
    .with_header("content-type", "text/html")
    .with_body(&include_str!(
      "fixtures/refurb_returns_valid_feed/latest-cool-article-123.html"
    )).create();

  let client = reqwest::Client::new();

  let expected_feed = rss::Channel::read_from(
    format!(
      include_str!("fixtures/refurb_returns_valid_feed/expected-feed.interpolated.xml"),
      hostname = mockito::SERVER_ADDRESS,
      host = server_host,
      article = article_path,
    ).as_bytes(),
  );

  let processed_feed = refurb(
    format!("{}{}", server_host, feed_path),
    Selectors::compile(".main-image,article").expect("compiled selectors"),
    &client,
  );

  assert!(
    processed_feed.is_ok(),
    "should return success processing feed"
  );

  assert_eq!(
    processed_feed.expect("processed feed"),
    expected_feed.expect("expected feed"),
    "should return the expected feed"
  );

  feed_request.assert();
  article_request.assert();
}

#[test]
fn refurb_returns_http_error() {
  let client = reqwest::Client::new();

  let refurbished = refurb(
    "http://127.0.0.1:1".to_string(),
    Selectors::compile("*").expect("compiled selectors"),
    &client,
  );

  assert!(
    refurbished.is_err(),
    "should return failure processing feed"
  );

  match refurbished.unwrap_err() {
    Error::HTTP(_) => (),
    _ => panic!("expected an HTTP error!"),
  };
}

#[test]
fn refurb_returns_rss_error() {
  let server_host = format!("http://{}", mockito::SERVER_ADDRESS);
  let feed_path = "/feed.rss";

  let feed_request = mockito::mock("GET", feed_path)
    .with_header("content-type", "application/xml")
    .with_body(&include_str!(
      "fixtures/refurb_returns_rss_error/invalid-feed.xml"
    )).create();

  let client = reqwest::Client::new();

  let refurbished = refurb(
    format!("{}{}", server_host, feed_path),
    Selectors::compile("*").expect("compiled selectors"),
    &client,
  );

  assert!(
    refurbished.is_err(),
    "should return failure processing feed"
  );

  match refurbished.unwrap_err() {
    Error::RSS(_) => (),
    _ => panic!("expected an RSS error!"),
  };

  feed_request.assert();
}
