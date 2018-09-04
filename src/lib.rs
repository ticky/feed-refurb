#[macro_use]
extern crate html5ever;
extern crate kuchiki;
#[cfg(test)]
#[macro_use]
extern crate pretty_assertions;
extern crate rayon;
extern crate reqwest;
extern crate rss;

use kuchiki::Selectors;
use rayon::prelude::*;
use rss::Channel;
use std::io::BufReader;
use std::result::Result;

fn create_br_element() -> kuchiki::NodeRef {
  kuchiki::NodeRef::new_element(
    html5ever::QualName::new(None, ns!(html), local_name!("br")),
    vec![],
  )
}

pub fn refurb(
  feed_url: String,
  description_selector: Selectors,
  http_client: &reqwest::Client,
) -> Result<Channel, String> {
  let mut feed = {
    let response_buffer = match http_client.get(feed_url.as_str()).send() {
      Err(error) => {
        return Err(error.to_string());
      }
      Ok(response) => BufReader::new(response),
    };

    println!("Fetched {}", feed_url);

    match Channel::read_from(response_buffer) {
      Err(error) => {
        return Err(error.to_string());
      }
      Ok(parsed) => parsed,
    }
  };

  println!("Parsed feed");

  feed
    .items_mut()
    .par_iter_mut()
    .enumerate()
    .for_each(|(index, item)| {
      let url = match item.link() {
        None => return,
        Some(url) => {
          println!("Item {}: has link {}", index, url);
          url.to_string()
        }
      };

      let source_document = {
        let text = {
          let mut response = match http_client.get(url.as_str()).send() {
            Err(_error) => return,
            Ok(response) => response,
          };

          println!("Item {}: Got response", index);

          match response.text() {
            Err(_error) => return,
            Ok(text) => text,
          }
        };

        println!("Item {}: Got response text", index);

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

      println!("Item {}: Parsed document", index);

      let new_description = {
        use kuchiki::iter::NodeIterator;

        let target_document = kuchiki::NodeRef::new_document();

        let selected = description_selector
          .filter(source_document.descendants().elements())
          .collect::<Vec<_>>();

        println!("Item {}: Got {} selection(s)", index, selected.len());

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

        println!("Item {}: Got selections", index);

        target_document.to_string()
      };

      item.set_description(new_description);

      println!("Item {}: Description set!", index);
    });

  println!("Processed entire feed!");

  Ok(feed)
}

#[cfg(test)]
mod test {
  extern crate mockito;
  extern crate reqwest;
  extern crate rss;

  use kuchiki::Selectors;

  use super::refurb;

  fn mock_get_request(path: &str, content_type: &str, body: &str) -> mockito::Mock {
    mockito::mock("GET", path)
      .with_header("content-type", content_type)
      .with_body(body)
      .create()
  }

  #[test]
  fn refurb_returns_valid_feed() {
    let server_host = format!("http://{}", mockito::SERVER_ADDRESS);
    let feed_path = "/feed.rss";
    let article_path = "/articles/latest-cool-article-123";

    let feed_request = mock_get_request(
      feed_path,
      "application/xml",
      &format!(
        include_str!("../test/fixtures/refurb_returns_valid_feed/source-feed.interpolated.xml"),
        hostname = mockito::SERVER_ADDRESS,
        host = server_host,
        article = article_path,
      ),
    );

    let article_request = mock_get_request(
      article_path,
      "text/html",
      &include_str!("../test/fixtures/refurb_returns_valid_feed/latest-cool-article-123.html"),
    );

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
}
