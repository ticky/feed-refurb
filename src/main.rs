#![feature(plugin)]
#![plugin(rocket_codegen)]
#![feature(custom_derive)]

extern crate html5ever;
extern crate reqwest;
extern crate rocket;
extern crate rocket_contrib;
extern crate rss;
extern crate scraper;

use rocket::http::RawStr;
use rocket::request::FromFormValue;
use rocket::response::content::Xml;
use rocket::response::Failure;
use rocket::{Request, State};
use rocket_contrib::Template;
use rss::Channel;
use scraper::Selector;
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

struct CSSSelector(Selector);

impl<'v> FromFormValue<'v> for CSSSelector {
  type Error = &'v RawStr;

  fn from_form_value(form_value: &'v RawStr) -> Result<CSSSelector, &'v RawStr> {
    match form_value.url_decode() {
      Ok(decoded) => match Selector::parse(&decoded) {
        Ok(selector) => Ok(CSSSelector(selector)),
        _ => Err(form_value),
      },
      _ => Err(form_value),
    }
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
        },
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

  // TODO: Fearless Concurrency!
  for item in feed.items_mut().iter_mut() {
    let new_description = match item.link() {
      None => continue,
      Some(url) => {
        println!("Item has link: {}", url);
        match http_client.client.get(url).send() {
          Err(_error) => continue,
          Ok(mut response) => {
            println!("Got response");
            match response.text() {
              Err(_error) => continue,
              Ok(text) => {
                println!("Got response text");
                use html5ever::tendril::TendrilSink;
                use std::default::Default;

                let parser = html5ever::driver::parse_document(
                  scraper::Html::new_document(),
                  html5ever::driver::ParseOpts {
                    tree_builder: html5ever::tree_builder::TreeBuilderOpts {
                      scripting_enabled: false,
                      ..Default::default()
                    },
                    ..Default::default()
                  },
                );

                let document = parser.one(text);

                println!("Got document");

                let selected_items: Vec<String> = document
                  .select(&configuration.description_selector.0)
                  .map(|i| i.html())
                  .collect();

                // TODO: Make sure the URLs present in the document are reassociated

                println!("Got selections");

                selected_items.join("<br/>")
              }
            }
          }
        }
      }
    };

    item.set_description(new_description);
    println!("Description set!");
  }

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

fn main() {
  rocket::ignite()
    .attach(Template::fairing())
    .manage(shared_http_client())
    .mount("/", routes![index, refurb])
    .catch(catchers![not_found])
    .launch();
}
