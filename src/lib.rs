extern crate failure;
#[macro_use]
extern crate failure_derive;
#[macro_use]
extern crate html5ever;
extern crate kuchiki;
#[macro_use]
extern crate log;
#[cfg(test)]
#[macro_use]
extern crate pretty_assertions;
extern crate rayon;
extern crate reqwest;
extern crate rss;

use kuchiki::Selectors;
use rayon::prelude::*;
use reqwest::Error as ReqwestError;
use rss::Channel;
use rss::Error as RSSError;
use std::io::BufReader;
use std::result::Result;

fn create_br_element() -> kuchiki::NodeRef {
  kuchiki::NodeRef::new_element(
    html5ever::QualName::new(None, ns!(html), local_name!("br")),
    vec![],
  )
}

#[derive(Fail, Debug)]
/// Errors that can occur while processing a feed.
pub enum Error {
  #[fail(display = "HTTP request error: {}", _0)]
  /// An error which occurred in downloading the feed.
  HTTP(#[cause] ReqwestError),

  #[fail(display = "RSS parser error: {}", _0)]
  /// An error which occurred while parsing the feed.
  RSS(#[cause] RSSError),
}

impl From<ReqwestError> for Error {
  /// Performs the conversion from a [`reqwest::Error`].
  fn from(err: ReqwestError) -> Error {
    Error::HTTP(err)
  }
}

impl From<RSSError> for Error {
  /// Performs the conversion from an [`rss::Error`].
  fn from(err: RSSError) -> Error {
    Error::RSS(err)
  }
}

pub fn refurb(
  feed_url: String,
  description_selector: Selectors,
  http_client: &reqwest::Client,
) -> Result<Channel, Error> {
  info!("Processing feed at {}", feed_url);

  let mut feed = {
    let response_buffer = http_client.get(feed_url.as_str()).send()?;

    debug!("{}: Fetched OK", feed_url);

    Channel::read_from(BufReader::new(response_buffer))?
  };

  debug!("{}: Parsed OK", feed_url);

  feed
    .items_mut()
    .par_iter_mut()
    .enumerate()
    .for_each(|(index, item)| {
      let url = match item.link() {
        None => return,
        Some(url) => {
          debug!("{}: Item {}: has link {}", feed_url, index, url);
          url.to_string()
        }
      };

      let source_document = {
        let text = {
          let mut response = match http_client.get(url.as_str()).send() {
            Err(_error) => return,
            Ok(response) => response,
          };

          debug!("{}: Item {}: Got response", feed_url, index);

          match response.text() {
            Err(_error) => return,
            Ok(text) => text,
          }
        };

        debug!("{}: Item {}: Got response text", feed_url, index);

        use html5ever::tree_builder::TreeBuilderOpts;
        use kuchiki::traits::TendrilSink;
        use kuchiki::ParseOpts;
        use std::default::Default;

        kuchiki::parse_html_with_options(ParseOpts {
          tree_builder: TreeBuilderOpts {
            scripting_enabled: false,
            ..Default::default()
          },
          ..Default::default()
        }).one(text)
      };

      debug!("{}: Item {}: Parsed document", feed_url, index);

      let new_description = {
        use kuchiki::iter::NodeIterator;

        let target_document = kuchiki::NodeRef::new_document();

        let selected = description_selector
          .filter(source_document.descendants().elements())
          .collect::<Vec<_>>();

        debug!(
          "{}: Item {}: Got {} selection(s)",
          feed_url,
          index,
          selected.len()
        );

        // TODO: Make this no-op if no selections are found

        selected.iter().for_each(|element| {
          // If we've already got siblings, separate with <br> elements
          if target_document.children().count() > 0 {
            target_document.append(create_br_element());
          }

          // Append the element!
          target_document.append(element.as_node().clone());
        });

        // TODO:
        // Reassociate the URLs present in the document. Rough plan:
        //  1. `new_dom.select("[href],[src]")`
        //  2. map over all of those merging their values with `url`

        debug!("{}: Item {}: Got selections", feed_url, index);

        target_document.to_string()
      };

      item.set_description(new_description);

      debug!("{}: Item {}: Description set!", feed_url, index);
    });

  info!("Processed feed at {}!", feed_url);

  Ok(feed)
}

#[cfg(test)]
mod test {
  extern crate mockito;
  extern crate reqwest;
  extern crate rss;

  use kuchiki::Selectors;

  use super::refurb;
  use super::Error;

  #[test]
  fn refurb_returns_valid_feed() {
    let server_host = format!("http://{}", mockito::SERVER_ADDRESS);
    let feed_path = "/feed.rss";
    let article_path = "/articles/latest-cool-article-123";

    let feed_request = mockito::mock("GET", feed_path)
      .with_header("content-type", "application/xml")
      .with_body(&format!(
        include_str!("../test/fixtures/refurb_returns_valid_feed/source-feed.interpolated.xml"),
        hostname = mockito::SERVER_ADDRESS,
        host = server_host,
        article = article_path,
      )).create();

    let article_request = mockito::mock("GET", article_path)
      .with_header("content-type", "text/html")
      .with_body(&include_str!(
        "../test/fixtures/refurb_returns_valid_feed/latest-cool-article-123.html"
      )).create();

    let client = reqwest::Client::new();

    let expected_feed = rss::Channel::read_from(
      format!(
        include_str!("../test/fixtures/refurb_returns_valid_feed/expected-feed.interpolated.xml"),
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
      .with_body(&include_str!("../test/fixtures/refurb_returns_rss_error/invalid-feed.xml"))
      .create();

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
}
