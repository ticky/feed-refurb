#![feature(plugin)]
#![plugin(rocket_codegen)]

extern crate rocket;
extern crate rocket_contrib;

use rocket::Request;
use rocket_contrib::Template;

#[get("/")]
fn index() -> Template {
  let mut map = std::collections::HashMap::new();
  map.insert("path", "/");
  Template::render("index", &map)
}

#[error(404)]
fn not_found(req: &Request) -> Template {
  let mut map = std::collections::HashMap::new();
  map.insert("path", req.uri().as_str());
  Template::render("error/404", &map)
}

fn main() {
  rocket::ignite()
    .attach(Template::fairing())
    .catch(errors![not_found])
    .mount("/", routes![index])
    .launch();
}
