#[macro_use] extern crate rocket;
use rocket::fs::{relative, FileServer};

use rocket::request::Outcome;
use rocket::http::Status;
use rocket::request::FromRequest;
use rocket::Request;
use rocket::http::Cookie;
use rocket::http::SameSite;

use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug)]
struct Room {
    id: String,
    name: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct User {
    id: String,
    name: String,
    room_id: Option<String>,
}

fn new_user() -> User {
    User {
        id: uuid::Uuid::new_v4().to_string(),
        name: "Username".to_string(),
        room_id: None,
    }
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for User {
    type Error = ();
    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let cookie = req.cookies().get("user_token").map(|c| c.value().to_string());
        match cookie {
            Some(cookie) => {
                match serde_json::from_str(&cookie) {
                    Ok(user) => Outcome::Success(user),
                    Err(_) => {
                        let user = new_user();
                        let new_cookie = Cookie::build(("user_token", serde_json::to_string(&user).unwrap()))
                            .path("/")
                            .secure(true)
                            .same_site(SameSite::Lax);
                        req.cookies().add(new_cookie);
                        Outcome::Success(user)
                    },
                }
            }

            None => {
                let user = new_user();
                let new_cookie = Cookie::build(("user_token", serde_json::to_string(&user).unwrap()))
                    .path("/")
                    .secure(true)
                    .same_site(SameSite::Lax);
                req.cookies().add(new_cookie);
                Outcome::Success(user)
            }
        }
    }
}

struct Question {
    text: String,
    correct: String,
    wrong1: String,
    wrong2: String,
}


#[get("/room/list")]
fn hello() -> String {
    format!("Hello")
}

#[get("/room/create")]
fn create_room(user: User) -> String {
    format!("Hello {}", user.name)
}

#[launch]
fn rocket() -> _ {


    let mut rooms: Vec<Room>  = Vec::new();

    rocket::build()
        .mount("/api", routes![hello, create_room])
        .mount("/", FileServer::from(relative!("html")))
        .manage(rooms)
}
