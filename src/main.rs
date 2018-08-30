#![feature(plugin)]
#![plugin(rocket_codegen)]
#![feature(custom_derive)]

extern crate reqwest;
extern crate rocket;
extern crate rocket_contrib;
extern crate rss;
extern crate scraper;

use rocket::http::RawStr;
use rocket::{Request, State};
use rocket::request::FromFormValue;
use rocket::response::Failure;
use rocket::response::content::Xml;
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
  client: reqwest::Client
}

struct CSSSelector(Selector);

impl<'v> FromFormValue<'v> for CSSSelector {
  type Error = &'v RawStr;

  fn from_form_value(form_value: &'v RawStr) -> Result<CSSSelector, &'v RawStr> {
    match form_value.url_decode() {
      Ok(decoded) => {
        match Selector::parse(&decoded) {
          Ok(selector) => Ok(CSSSelector(selector)),
          _ => Err(form_value),
        }
      },
      _ => Err(form_value)
    }
  }
}

#[derive(FromForm)]
struct FeedConfiguration {
  feed: String,
  description_selector: CSSSelector
}

#[get("/refurb?<configuration>")]
fn refurb(configuration: FeedConfiguration, http_client: State<HTTPClient>) -> Result<Xml<String>, Failure> {
  let mut feed = match http_client.client.get(configuration.feed.as_str()).send() {
    Ok(response) => {
      match Channel::read_from(BufReader::new(response)) {
        Ok(parsed) => parsed,
        Err(_error) => {
          // TODO: Handle specific errors
          return Err(Failure(rocket::http::Status::NotAcceptable))
        }
      }
    },
    Err(_error) => {
      // TODO: Handle specific errors
      return Err(Failure(rocket::http::Status::NotAcceptable))
    }
  };

  for item in feed.items_mut().iter_mut() {
    let new_description = match item.link() {
      None => continue,
      Some(url) => {
        match http_client.client.get(url).send() {
          Err(_error) => continue,
          Ok(mut response) => {
            match response.text() {
              Err(_error) => continue,
              Ok(text) => {
                let document = scraper::Html::parse_document(&text);

                let selected_items: Vec<String> = document.select(&configuration.description_selector.0).map(|i| { i.html() }).collect();

                // TODO: Make sure the URLs present in the document are reassociated

                selected_items.join("<br/>")
              }
            }
          }
        }
      }
    };

    item.set_description(new_description);
  }

  Ok(Xml(feed.to_string()))
}

#[catch(404)]
fn not_found(req: &Request) -> Template {
  let mut map = std::collections::HashMap::new();
  map.insert("path", req.uri().as_str());
  Template::render("error/404", &map)
}

fn shared_http_client() -> HTTPClient {
  use reqwest::Client;
  use reqwest::header;

  let mut headers = header::Headers::new();
  headers.set(header::UserAgent::new(format!("{}/{}", NAME, VERSION)));

  HTTPClient {
    client: Client::builder()
                   .default_headers(headers)
                   .build()
                   .unwrap()
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
