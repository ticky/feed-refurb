#![feature(plugin)]
#![plugin(rocket_codegen)]
#![feature(custom_derive)]

#[macro_use]
extern crate html5ever;
extern crate kuchiki;
extern crate rayon;
extern crate reqwest;
extern crate rocket;
extern crate rocket_contrib;
extern crate rss;

use kuchiki::Selectors;
use rayon::prelude::*;
use rocket::http::RawStr;
use rocket::request::FromFormValue;
use rocket::response::content::Xml;
use rocket::response::Failure;
use rocket::{Request, State};
use rocket_contrib::Template;
use rss::Channel;
use std::io::BufReader;

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

fn create_br_element() -> kuchiki::NodeRef {
  kuchiki::NodeRef::new_element(
    html5ever::QualName::new(None, ns!(html), local_name!("br")),
    vec![],
  )
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
  let mut feed = match http_client.client.get(configuration.feed.as_str()).send() {
    Ok(response) => {
      println!("Fetched {}", configuration.feed);
      match Channel::read_from(BufReader::new(response)) {
        Ok(parsed) => {
          println!("Parsed feed");
          parsed
        }
        Err(_error) => {
          // TODO: Handle specific errors
          return Err(Failure(rocket::http::Status::NotAcceptable));
        }
      }
    }
    Err(_error) => {
      // TODO: Handle specific errors
      return Err(Failure(rocket::http::Status::NotAcceptable));
    }
  };

  feed.items_mut().par_iter_mut().for_each(|item| {
    let new_description = match item.link() {
      None => return,
      Some(url) => {
        println!("Item has link: {}", url);
        match http_client.client.get(url).send() {
          Err(_error) => return,
          Ok(mut response) => {
            println!("Got response");
            match response.text() {
              Err(_error) => return,
              Ok(text) => {
                println!("Got response text");
                use html5ever::tree_builder::TreeBuilderOpts;
                use kuchiki::iter::NodeIterator;
                use kuchiki::traits::TendrilSink;
                use kuchiki::ParseOpts;
                use std::default::Default;

                let target_document = kuchiki::NodeRef::new_document();

                let source_document = kuchiki::parse_html_with_options(ParseOpts {
                  tree_builder: TreeBuilderOpts {
                    scripting_enabled: false,
                    ..Default::default()
                  },
                  ..Default::default()
                }).one(text);

                println!("Parsed document");

                let selected = configuration
                  .description_selector
                  .0
                  .filter(source_document.descendants().elements())
                  .collect::<Vec<_>>();

                println!("Got {} selection(s)", selected.len());

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

                println!("Got selections");

                target_document.to_string()
              }
            }
          }
        }
      }
    };

    item.set_description(new_description);

    println!("Description set!");
  });

  println!("Processed entire feed!");

  Ok(Xml(feed.to_string()))
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

pub fn application() -> rocket::Rocket {
  rocket::ignite()
    .attach(Template::fairing())
    .manage(shared_http_client())
    .mount("/", routes![index, refurb])
    .catch(catchers![not_found])
}
