#![feature(plugin)]
#![plugin(rocket_codegen)]

extern crate rocket;

#[get("/")]
fn index() -> &'static str {
  "Oh wow, it works!"
}

fn main() {
  rocket::ignite()
    .mount("/", routes![index])
    .launch();
}
