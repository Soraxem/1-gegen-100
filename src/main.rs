#[macro_use] extern crate rocket;

// Static files for App
use rocket::fs::{relative, FileServer};


use rocket::request::Outcome;
use rocket::http::Status;
use rocket::request::FromRequest;
use rocket::Request;
use rocket::http::Cookie;
use rocket::http::CookieJar;
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

// Random number generators
use rand::prelude::*;


#[derive(Serialize, Deserialize, Debug, Clone)]
struct Room {
    state: RoomState,
    // Current question
    question: Option<Question>,
    // All players that still ar in the game
    contestants: Vec<User>,
    // The player everyone is playing against
    player: User,
    // Everyone in the room
    users: Vec<User>,
}

// Statemachine of the room. Flowchart is in the readme
#[derive(Serialize, Deserialize, Debug, Clone)]
enum RoomState {
    // Joining an Leaving of Players
    Open,
    // Select the challenger
    PlayerSelection,
    // Show the Question & and wait for everyone to answer
    Question,
    // Evaluate the contestants
    EvaluateContestants,
    // Evaluate the player
    EvaluatePlayer,
    // Round is over
    RoundEnd
}


// Question wich is read from json
#[derive(Serialize, Deserialize, Debug, Clone)]
struct JsonQuestion {
    text: String,
    correct: String,
    wrong1: String,
    wrong2: String,
}

// Question struct for the server
#[derive(Serialize, Deserialize, Debug, Clone)]
struct Question {
    text: String,
    answers: [String; 3],
    correct: usize,
}

fn test_question() -> Question {
    Question {
        text: "Was ergibt diese Rechnung? 1 + 1".to_string(),
        answers: ["2".to_string(), "3".to_string(), "4".to_string()],
        correct: 0
    }
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


fn room_exists(state: &State<AppState>, room_id: String) -> bool {
    // Check for room id in hash map
    state.rooms.read().unwrap().contains_key(&room_id)
}


#[get("/room/<room_id>/manager")]
async fn manage_room(state: &State<AppState>, room_id: String) -> EventStream![] {
    
    let room = Room {
        users: Vec::new(),
        round: None,
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
                yield Event::json(&event.kind);
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

#[get("/room/<room_id>/start-round")]
fn start_round(state: &State<AppState>, room_id: String) -> String {

    if room_exists(state, room_id.clone()) {

        let rooms = state.rooms.read().unwrap();
        let room = rooms.get(&room_id).unwrap();

        let mut contestants = room.read().unwrap().users.clone();
        print!("{:#?}", contestants);
        let random_index = rand::random_range(0..contestants.len());
        let challenger = contestants.swap_remove(random_index);

        

        // Create a new round
        let mut round = Round {
            // Start with PlayerSelection
            state: RoundState::PlayerSelection,
            question: test_question(),
            contestants: contestants,
            challenger: challenger.clone(),
        };

        room.write().unwrap().round = Some(round);

        // Send event
        let _ = state.tx.send(AppEvent {
            room_id: room_id.clone(),
            kind: EventKind::PlayerSelection { user: challenger.clone() },
        });

        "ok".to_string()
    } else {
        "error".to_string()
    }
}

#[get("/room/<room_id>/question")]
fn question(state: &State<AppState>, room_id: String) -> String {
    "error".to_string()
}

#[get("/room/<room_id>/evaluate-contestants")]
fn evaluate_contestants(state: &State<AppState>, room_id: String) -> String {
"ok".to_string()
}

#[get("/room/<room_id>/evaluate-player")]
fn evaluate_player(state: &State<AppState>, room_id: String) -> String {
"ok".to_string()
}

#[get("/room/<room_id>/end-round")]
fn end_round(state: &State<AppState>, room_id: String) -> String {
    "ok".to_string()
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
                yield Event::json(&event.kind);
            }
            
        }
    }
}

// Set username
#[get("/player/set-username/<name>?<room>")]
fn update_player_name(room: String, old_user: User, name: String, jar: &CookieJar<'_>, state: &State<AppState>) -> String {

    let mut user = old_user.clone();
    user.name = name;

    let new_cookie = Cookie::build(("user_token", serde_json::to_string(&user).unwrap()))
        .path("/")
        .secure(true)
        .same_site(SameSite::Lax);

    jar.add(new_cookie);

    let _ = state.tx.send(AppEvent {
        room_id: room.clone(),
        kind: EventKind::UserUpdated { user: user.clone() },
    });

    "ok".to_string()
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct AppEvent {
    room_id: String,
    kind: EventKind,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
enum EventKind {
    UserJoined { user: User },
    UserUpdated { user: User },
    UserLeft { user: User },
    PlayerSelection { user: User },
    Question { question: Question },
}

// Events sent to the room manager
#[derive(Serialize, Deserialize, Clone, Debug)]
enum RoomEvent {
    UserJoined { user: User },
    UserLeft { user: User },
    Question {
        text: String,
        answers: [String; 3],
        from: u64,
        to: u64,
    },
    EvaluateContestants {
        evaluations: Vec<EvaluatedUser>,
    },
    EvaluatePlayer {
        text: String,
        answers: [EvaluatedAnswer; 3],
    },
    EndRound,
}

// Visually marked Answer on the screen
#[derive(Serialize, Deserialize, Clone, Debug)]
struct EvaluatedAnswer {
    answer: String,
    evaluation: AnswerSelection,
}

// Answer marking options
#[derive(Serialize, Deserialize, Clone, Debug)]
enum AnswerSelection {
    Correct,
    Wrong,
    Neutral,
    WrongSelection,
}

// Visually marked user for all to see
#[derive(Serialize, Deserialize, Clone, Debug)]
struct EvaluatedUser {
    user: User,
    evaluation: Evaluation,
}

// User marking options
#[derive(Serialize, Deserialize, Clone, Debug)]
enum Evaluation {
    Correct,
    Wrong,
    Out,
}

// Events sent to the player
#[derive(Serialize, Deserialize, Clone, Debug)]
enum PlayerEvent {
    Question {
        from: u64,
        to: u64,
    },
    Screen {
        screen: PlayerScreens
    }
}

// All Screens that can be triggered
#[derive(Serialize, Deserialize, Clone, Debug)]
enum PlayerScreens {
    In,
    Out,
    YouGotSelected,
    ToSlow,
    Wrong,
    Empty,
    Loading,
}

struct AppState {
    tx: broadcast::Sender<AppEvent>,
    player_events: broadcast::Sender<PlayerEvent>,
    room_events: broadcast::Sender<RoomEvent>,
    rooms: RwLock<HashMap<String, RwLock<Room>>>,
}

#[launch]
fn rocket() -> _ {

    let (tx, _rx) = broadcast::channel(1024);
    let (player_tx, _player_rx) = broadcast::channel(1024);
    let (room_tx, _room_rx) = broadcast::channel(1024);

    rocket::build()
        .mount("/api", routes![manage_room, join_room, check_room, update_player_name, start_round, question])
        .mount("/", FileServer::from(relative!("html")))
        .manage(AppState {
            tx,
            player_events: player_tx,
            room_events: room_tx,
            rooms: RwLock::new(HashMap::new()),
        })
}
