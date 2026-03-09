#[macro_use] extern crate rocket;
use rocket::fs::{relative, FileServer};

use rocket::request::Outcome;
use rocket::http::Status;
use rocket::request::FromRequest;
use rocket::Request;
use rocket::http::Cookie;
use rocket::http::SameSite;

use rocket::fairing::AdHoc;
use rocket::response::stream::{EventStream, Event};
use rocket::serde::json::Json;
use rocket::tokio::select;
use rocket::tokio::sync::broadcast;
use rocket::{get, post, routes, Build, Rocket, State};

use serde::{Serialize, Deserialize};

// Mutable state
use std::sync::RwLock;

// Unique room id
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Debug)]
struct Room {
    users: Vec<User>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct User {
    id: String,
    name: String,
}

fn new_user() -> User {
    User {
        id: uuid::Uuid::new_v4().to_string(),
        name: "Username".to_string(),
    }
}

// Implement a Request guard for User struct
#[rocket::async_trait]
impl<'r> FromRequest<'r> for User {
    type Error = ();
    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {

        // Check if a user cookie exists
        let cookie = req.cookies().get("user_token").map(|c| c.value().to_string());
        match cookie {
            Some(cookie) => {
                match serde_json::from_str(&cookie) {

                    // If the cookie is valid, return the user
                    Ok(user) => Outcome::Success(user),

                    // If the cookie is invalid, create a new user
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

            // If there is no cookie, create a new user
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

fn room_exists(state: &State<AppState>, room_id: String) -> bool {
    // Check for room id in hash map
    state.rooms.read().unwrap().contains_key(&room_id)
}


#[get("/room/<room_id>/manager")]
async fn manage_room(state: &State<AppState>, room_id: String) -> EventStream![] {
    
    let room = Room {
        users: Vec::new(),
    };

    if !room_exists(state, room_id.clone()) {
        state.rooms.write().unwrap().insert(room_id.to_string(), RwLock::new(room));
    }

    let mut rx = state.tx.subscribe();
    EventStream! {

        loop {
            let event = match rx.recv().await {
                Ok(event) => event,
                Err(_) => break,
            };

            if event.room_id == room_id {
                yield Event::json(&event);
            }
            
        }
    }
    
}



#[get("/room/<room_id>/exists")]
fn check_room(state: &State<AppState>, room_id: String) -> String {

    if room_exists(state, room_id) {
        "true".to_string()
    } else {
        "false".to_string()
    }
}

#[get("/room/<room_id>/player")]
fn join_room(state: &State<AppState>, user: User, room_id: String) -> EventStream![] {
    
    let rooms = state.rooms.read().unwrap();
    let room = rooms.get(&room_id).unwrap();

    let mut found = false;
    for u in room.read().unwrap().users.iter() {
        if u.id == user.id {
            found = true;
            break;
        }
    }

    if !found {
        room.write().unwrap().users.push(user.clone());
    }

    let _ = state.tx.send(AppEvent {
        room_id: room_id.clone(),
        kind: EventKind::UserJoined { user: user.clone() },
    });

    let mut rx = state.tx.subscribe();
    EventStream! {
        loop {
            let event = match rx.recv().await {
                Ok(event) => event,
                Err(_) => break,
            };

            if event.room_id == room_id {
                yield Event::json(&event);
            }
            
        }
    }
}

#[get("/events")]
fn listen_events(state: &State<AppState>) -> EventStream![] {
    let mut rx = state.tx.subscribe();
    EventStream! {
        loop {
            let event = match rx.recv().await {
                Ok(event) => event,
                Err(_) => break,
            };

            yield Event::json(&event);
        }
    }
}

#[get("/send_event")]
fn test_event(state: &State<AppState>) {
    let _ = state.tx.send(AppEvent{ room_id: "test".to_string(), kind: EventKind::UserJoined { user: new_user() } });
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct AppEvent {
    room_id: String,
    kind: EventKind,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
enum EventKind {
    UserJoined { user: User },
    UserLeft { user: User },
}

struct AppState {
    tx: broadcast::Sender<AppEvent>,
    rooms: RwLock<HashMap<String, RwLock<Room>>>,
}

#[launch]
fn rocket() -> _ {

    let (tx, _rx) = broadcast::channel(1024);

    rocket::build()
        .mount("/api", routes![listen_events, test_event, manage_room, join_room, check_room])
        .mount("/", FileServer::from(relative!("html")))
        .manage(AppState {
            tx,
            rooms: RwLock::new(HashMap::new()),
        })
}
