#![feature(plugin)]
#![plugin(rocket_codegen)]
#![feature(custom_derive)]

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
                use kuchiki::traits::TendrilSink;
                use std::default::Default;

                let source_document = kuchiki::parse_html_with_options(kuchiki::ParseOpts {
                  tree_builder: html5ever::tree_builder::TreeBuilderOpts {
                    scripting_enabled: false,
                    ..Default::default()
                  },
                  ..Default::default()
                }).one(text);

                println!("Parsed document");

                let selection: Vec<String> = configuration.description_selector.0.filter(source_document.inclusive_descendants())
                  .map(|element| element.html())
                  .collect();

                // TODO:
                //  1. Transplant selected elements to a new DOM context (Kuchiki?)
                //  2. Make sure the URLs present in the document are reassociated
                //     Rough plan:
                //      1. `new_dom.select("[href],[src]")`
                //      2. map over all of those merging their values with `url`
                //  3. Serialise that new DOM and return that value from this closure

                println!("Got selections");

                selection.join("<br/>")
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
  headers.set(header::UserAgent::new(format!("{}/{}", NAME, VERSION)));

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
  use super::application;
  use rocket::http::Status;
  use rocket::local::Client;

  #[test]
  fn index() {
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
}
