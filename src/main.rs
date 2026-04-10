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

// time fetching
use std::time::SystemTime;

// Read Questions file
use std::fs::File;


#[derive(Serialize, Deserialize, Debug, Clone)]
struct Room {
    state: RoomState,
    // Current question
    question: Option<Question>,
    // Submitted answers
    answers: HashMap<String, usize>,
    // All players that still are in the current round
    contestants: Vec<User>,
    // The player everyone is competing against
    player: Option<User>,
    // Everyone in the room
    users: Vec<User>,

    questions: Vec<JsonQuestion>,

    // All users that havnt been selected before
    selected: Vec<String>,
}

// Default values for room on creation
impl Default for Room {
    fn default() -> Self {
        Room {
            state: RoomState::Open,
            question: None,
            answers: HashMap::new(),
            contestants: Vec::new(),
            player:None,
            users: Vec::new(),
            questions: fetch_questions(),
            selected: Vec::new(),
        }
    }
}

/// Statemachine of the room. Flowchart is in the readme
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
enum RoomState {
    /// Joining and Leaving of Players
    Open,
    /// Select the challenger
    PlayerSelection,
    /// Show the Question & and wait for everyone to answer
    Question,
    /// Evaluate the contestants
    EvaluateContestants,
    /// Evaluate the player
    EvaluatePlayer,
    /// Round is over
    RoundEnd
}


/// Question wich is read from json
#[derive(Serialize, Deserialize, Debug, Clone)]
struct JsonQuestion {
    /// The question
    text: String,
    /// All possible answers
    answers: [String; 3],
    /// Index of the correct answer, using a 1 based index!!
    correct: usize,
}

/// Question struct for the server
#[derive(Serialize, Deserialize, Debug, Clone)]
struct Question {
    /// The question
    text: String,
    /// All possible answers
    answers: [String; 3],
    /// Index of the correct answer, using a 1 based index!!
    correct: usize,

    start_time: u64,
    end_time: u64,
    player_start_time: u64,
    player_end_time: u64,
}



// Return all questions
fn fetch_questions() -> Vec<JsonQuestion> {
    let questions_file = File::open("questions.json").expect("Failed to open questions.json");
    let questions: Vec<JsonQuestion> = serde_json::from_reader(questions_file).expect("Failed to parse questions.json");
    return questions;
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
                            .secure(false)
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
                    .secure(false)
                    .same_site(SameSite::Lax);
                req.cookies().add(new_cookie);
                Outcome::Success(user)
            }
        }
    }
}




#[get("/room/<room_id>/manager")]
async fn manage_room(state: &State<AppState>, room_id: String) -> EventStream![] {
    
    let room = Room::default();

    if !room_exists(state, &room_id) {
        state.rooms.write().unwrap().insert(room_id.to_string(), RwLock::new(room));
    }

    let mut rx = state.tx.subscribe();
    let mut room_events = state.room_events.subscribe();
    EventStream! {

        loop {

            let room_event = match room_events.recv().await {
                Ok(event) => event,
                Err(_) => break,
            };

            if room_event.room_id == room_id {
                yield Event::json(&room_event.kind);
            }
            
        }
    }
    
}

#[get("/room/<room_id>/exists")]
fn check_room(state: &State<AppState>, room_id: String) -> String {

    if room_exists(state, &room_id) {
        "true".to_string()
    } else {
        "false".to_string()
    }
}

// Check if a room exists
fn room_exists(state: &State<AppState>, room_id: &str) -> bool {
    // Check for room id in hash map
    state.rooms.read().unwrap().contains_key(room_id)
}

// Overwrite a room in the hash map
fn overwrite_room(state: &State<AppState>, room_id: String, room: Room) {
    let mut rooms = state.rooms.write().unwrap();
    rooms.insert(room_id, RwLock::new(room));
}

// Remove a room from the hash map
fn delete_room(state: &State<AppState>, room_id: &str) -> Option<Room> {
    let mut rooms = state.rooms.write().unwrap();
    rooms.remove(room_id).map(|lock| lock.into_inner().unwrap())
}

// Update fields of a room
// Example: update_room(&state, "room_1", |room| room.player_count += 1);
fn update_room_field<F>(state: &State<AppState>, room_id: &str, f: F) -> bool 
where F: FnOnce(&mut Room) 
{
    let rooms = state.rooms.read().unwrap();
    if let Some(room_lock) = rooms.get(room_id) {
        let mut room = room_lock.write().unwrap();
        f(&mut room);
        true
    } else {
        false
    }
}

// Read a room field
// Example: let is_full = read_room_field(state, "room_1", |r| r.players.len() >= r.max_capacity);
fn read_room_field<F, T>(state: &State<AppState>, room_id: &str, f: F) -> Option<T>
where
    F: FnOnce(&Room) -> T,
{
    let rooms = state.rooms.read().unwrap();
    rooms.get(room_id).map(|room_lock| {
        let room = room_lock.read().unwrap();
        f(&room)
    })
}



#[get("/room/<room_id>/start-round")]
fn start_round(state: &State<AppState>, room_id: String) -> String {

    match read_room_field(state, &room_id, |r| r.users.clone()) {
        Some(mut contestants) => {
            if contestants.is_empty() {
                return "not_enough_players".to_string();
            }

            let selection_successfull = update_room_field(state, &room_id, |room| {

                println!("Trying selection");

                // If everyone was selected once, refill the list
                if room.selected.is_empty() {
                    room.selected = room.users.iter().map(|u| u.id.clone()).collect();
                }

                // Select a random user
                let random_index = rand::random_range(0..room.selected.len());
                let selected_id = room.selected.swap_remove(random_index);

                // Fill the contestants
                room.contestants = room.users.clone();

                // Select a player
                if let Some(index) = room.contestants.iter().position(|u| u.id == selected_id) {

                    let player = room.contestants.swap_remove(index);

                    room.player = Some(player);
                    room.state = RoomState::PlayerSelection;
                }
            });

            let contestant_ids = read_room_field(state, &room_id, |r| r.contestants.iter().map(|c| c.id.clone()).collect()).unwrap();

            if let Some(Some(player)) = read_room_field(state, &room_id, |r| r.player.clone()) {

                // Display the selected Player upfront
                let _ = state.room_events.send(RoomEvent {
                    room_id: room_id.clone(),
                    kind: RoomEventKind::PlayerSelected { user: player.clone() },
                });

                // Display the selected Player that they got selected
                let _ = state.player_events.send(PlayerEvent {
                    player_ids: vec![player.id],
                    kind: PlayerEventKind::Screen { screen: PlayerScreens::YouGotSelected },
                });

                // Display the selected Player that they got selected
                let _ = state.player_events.send(PlayerEvent {
                    player_ids: contestant_ids,
                    kind: PlayerEventKind::Screen { screen: PlayerScreens::In },
                });

            } else {
                return "Player Selection Failed!".to_string();
            }

            
        },
        None => return "error".to_string(),
    }
    "ok".to_string()
}

#[get("/room/<room_id>/user-list")]
fn user_list(state: &State<AppState>, room_id: String) -> String {

    match read_room_field(state, &room_id, |r| r.users.clone()) {
        Some(users) => serde_json::to_string(&users).unwrap(),
        None => "error".to_string(),
    }
}


fn new_question(state: &State<AppState>, room_id: String) -> Option<JsonQuestion> {
    let mut questions = match read_room_field(state, &room_id, |r| r.questions.clone()) {
        Some(mut_questions) => mut_questions,
        None => return None,
    };

    let random_index = rand::random_range(0..questions.len());
    let question = questions.swap_remove(random_index);

    update_room_field(state, &room_id, |r| r.questions = questions);

    return Some(question);
}


#[get("/room/<room_id>/question")]
fn question(state: &State<AppState>, room_id: String) -> String {

    /*
    // Fetch the room state
    let room_state = match read_room_field(state, &room_id, |r| r.state.clone()) {
        Some(state) => state,
        None => return "error".to_string(),
    };
    // Check if room state is correct for transition to question
    if room_state != RoomState::PlayerSelection && room_state != RoomState::EvaluatePlayer {
        return "error".to_string();
    }
    */

    // Pseudeo random Question Fetching
    let question = new_question(state, room_id.clone()).unwrap();

    // Save the question
    //update_room_field(state, &room_id, |r| r.question = Some(question.clone()));
    // Change the room state
    update_room_field(state, &room_id, |r| r.state = RoomState::Question);

    // Calculate start and end time for question response
    let start_time = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs() + 4;
    
    let end_time = start_time + 12;

    let player_start_time = end_time;
    let player_end_time = end_time + 32;

    // Put together question object for the server
    let question = Question {
        text: question.text,
        answers: question.answers,
        correct: question.correct,

        // Adding seconds as leeway for wrong timed devices
        start_time: start_time - 1,
        end_time: end_time + 2,
        player_start_time: player_start_time - 1,
        player_end_time: player_end_time + 2
    };

    // Save the question
    update_room_field(state, &room_id, |r| r.question = Some(question.clone()));
    // Clear the hash map for the new question
    update_room_field(state, &room_id, |r| r.answers = HashMap::new());

    // Display the question
    let _ = state.room_events.send(RoomEvent {
        room_id: room_id.clone(),
        kind: RoomEventKind::Question {
            text: question.text.clone(),
            answers: question.answers.clone(),
            from: start_time,
            to: end_time
        },
    });

    // Fetch the contestants
    let contestants = match read_room_field(state, &room_id, |r| r.contestants.clone()) {
        Some(contestants) => contestants,
        None => return "error".to_string(),
    };

    // Display the question to the contestants
    let _ = state.player_events.send(PlayerEvent {
        player_ids: contestants.iter().map(|c| c.id.clone()).collect(),
        kind: PlayerEventKind::Question {
            text: question.text.clone(),
            answers: question.answers.clone(),
            from: start_time,
            to: end_time
        },
    });


    // Fetch the player
    let player = match read_room_field(state, &room_id, |r| r.player.clone()) {
        Some(player) => player,
        None => return "error".to_string(),
    };

    // Display the question to the player
    let _ = state.player_events.send(PlayerEvent {
        player_ids: vec![player.unwrap().id],
        kind: PlayerEventKind::Question {
            text: question.text.clone(),
            answers: question.answers.clone(),
            from: player_start_time,
            to: player_end_time
        }
    });

    "ok".to_string()
}

#[get("/player/<room_id>/answer-question/<answer>")]
fn answer_question(state: &State<AppState>, user: User, room_id: String, answer: usize) -> String {

    // Fetch the question
    let question = match read_room_field(state, &room_id, |r| r.question.clone()) {
        Some(question) => question,
        None => return "error".to_string(),
    };

    let current_time = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs();


    let player = match read_room_field(state, &room_id, |r| r.player.clone()) {
        Some(player) => player,
        None => return "error".to_string(),
    };
    
    // Detect if User was the main player
    if user.id == player.unwrap().id {

        if current_time > question.clone().unwrap().player_end_time {
            return "ended".to_string();
        }
        if current_time < question.clone().unwrap().player_start_time {
            return "not started".to_string();
        }

    } else {    

        if current_time > question.clone().unwrap().end_time {
            return "ended".to_string();
        }
        if current_time < question.clone().unwrap().start_time {
            return "not started".to_string();
        }

    }

    // Register Answer
    update_room_field(state, &room_id, |r| { r.answers.insert(user.id, answer); });

    "ok".to_string()
}

#[get("/room/<room_id>/evaluate-contestants")]
fn evaluate_contestants(state: &State<AppState>, room_id: String) -> String {
    
    let users = match read_room_field(state, &room_id, |r| r.users.clone()) {
        Some(users) => users,
        None => return "error".to_string(),
    };

    let mut evaluated_players: HashMap<String, Evaluation> = HashMap::new();

    // Mark all users as out
    for user in users {
        evaluated_players.insert(user.id, Evaluation::Out);
    }

    // Remove the player from that list
    let player = match read_room_field(state, &room_id, |r| r.player.clone()) {
        Some(player) => player,
        None => return "error".to_string(),
    };
    evaluated_players.remove(&player.unwrap().id);

    // Fetch all the contestants
    let mut contestants = match read_room_field(state, &room_id, |r| r.contestants.clone()) {
        Some(contestants) => contestants,
        None => return "error".to_string(),
    };

    // Fetch the question
    let question = match read_room_field(state, &room_id, |r| r.question.clone()) {
        Some(question) => question,
        None => return "error".to_string(),
    };

    // Evaluate the contestants
    for contestant in contestants.clone() {
    let maybe_answer = match read_room_field(state, &room_id, |r| r.answers.get(&contestant.id).cloned()) {
        Some(ans) => ans, // This is still an Option (e.g., Option<String>)
        None => return "error".to_string(),
    };

    // Use if let to safely check if the answer exists and matches
    if let Some(actual_answer) = maybe_answer {
        if actual_answer == question.clone().unwrap().correct {
            evaluated_players.insert(contestant.id, Evaluation::Correct);
        } else {
            evaluated_players.insert(contestant.id.clone(), Evaluation::Wrong);
            contestants.retain(|c| c.id != contestant.id);
        }
    } else {
        // Handle the case where answer is None (e.g., treat as Wrong)
        evaluated_players.insert(contestant.id.clone(), Evaluation::Wrong);
        contestants.retain(|c| c.id != contestant.id);
    }
}

    // Update the contestants
    update_room_field(state, &room_id, |r| r.contestants = contestants);

    // TODO: Points, and finish wehn no more contestants
    // Update Room state

    // Evaluation Complete


    // Display the results
    let _ = state.room_events.send(RoomEvent {
        room_id: room_id.clone(),
        kind: RoomEventKind::EvaluateContestants {
            evaluations: evaluated_players.clone()
        },
    });

    // Show all Correct signals
    let _ = state.player_events.send(PlayerEvent {
        player_ids: evaluated_players.iter().filter_map(|(id, evaluation)| if let Evaluation::Correct = evaluation { Some(id.clone()) } else { None }).collect(),
        kind: PlayerEventKind::Screen { screen: PlayerScreens::Correct },
    });

    // Show all Wrong signals
    let _ = state.player_events.send(PlayerEvent {
        player_ids: evaluated_players.iter().filter_map(|(id, evaluation)| if let Evaluation::Wrong = evaluation { Some(id.clone()) } else { None }).collect(),
        kind: PlayerEventKind::Screen { screen: PlayerScreens::Wrong },
    });

    // Show all Out signals
    let _ = state.player_events.send(PlayerEvent {
        player_ids: evaluated_players.iter().filter_map(|(id, evaluation)| if let Evaluation::Out = evaluation { Some(id.clone()) } else { None }).collect(),
        kind: PlayerEventKind::Screen { screen: PlayerScreens::Out },
    });
    
    "ok".to_string()
}

#[get("/room/<room_id>/evaluate-player")]
fn evaluate_player(state: &State<AppState>, room_id: String) -> String {
    

    // Fetch the question
    let question = match read_room_field(state, &room_id, |r| r.question.clone()) {
        Some(question) => question,
        None => return "error".to_string(),
    };

    // Fetch The Player
    let player = match read_room_field(state, &room_id, |r| r.player.clone()) {
        Some(player) => player,
        None => return "error".to_string(),
    };

    // fetch the players answer
    let answer = match read_room_field(state, &room_id, |r| r.answers.get(&player.clone().unwrap().id).cloned()) {
        Some(answers) => answers,
        None => return "error".to_string(),
    };


    let mut evaluated_answers = [
        EvaluatedAnswer {
            answer: question.clone().unwrap().answers.get(0).cloned().unwrap(),
            evaluation: AnswerSelection::Wrong,
        },
        EvaluatedAnswer {
            answer: question.clone().unwrap().answers.get(1).cloned().unwrap(),
            evaluation: AnswerSelection::Wrong,
        },
        EvaluatedAnswer {
            answer: question.clone().unwrap().answers.get(2).cloned().unwrap(),
            evaluation: AnswerSelection::Wrong,
        },
    ];

    let mut end_round = false;

    match answer {
        Some(answer) => {
            if answer ==  question.clone().unwrap().correct {
                // Mark correct Answer
                evaluated_answers[answer-1].evaluation = AnswerSelection::Correct;

                // Send correct screen
                let _ = state.player_events.send(PlayerEvent {
                    player_ids: vec![player.clone().unwrap().id],
                    kind: PlayerEventKind::Screen { screen: PlayerScreens::Correct },
                });

            } else {
                // Mark incorrect answer and solution
                evaluated_answers[answer-1].evaluation = AnswerSelection::WrongSelection;
                evaluated_answers[question.clone().unwrap().correct-1].evaluation = AnswerSelection::Correct;

                // Send incorrect screen
                let _ = state.player_events.send(PlayerEvent {
                    player_ids: vec![player.clone().unwrap().id],
                    kind: PlayerEventKind::Screen { screen: PlayerScreens::Wrong },
                });

                // Endround
                end_round = true;
            }
        }
        None => {

            evaluated_answers[question.clone().unwrap().correct-1].evaluation = AnswerSelection::Correct;

            // Send incorrect screen
            let _ = state.player_events.send(PlayerEvent {
                player_ids: vec![player.clone().unwrap().id],
                kind: PlayerEventKind::Screen { screen: PlayerScreens::ToSlow },
            });

            // Endround
                end_round = true;

        }
    }
    // Check if correct
    

    let _ = state.room_events.send(RoomEvent {
        room_id: room_id.clone(),
        kind: RoomEventKind::EvaluatePlayer {
            text: question.clone().unwrap().text,
            answers: evaluated_answers,
            end_round
        },
    });

    "ok".to_string()
}

#[get("/room/<room_id>/end-round")]
fn end_round(state: &State<AppState>, room_id: String) -> String {

    // ToDo Distribute Points

    
    // Clean states for new round. Reset to Lobby

    update_room_field(state, &room_id, |r| r.question = None);
    update_room_field(state, &room_id, |r| r.player = None);
    update_room_field(state, &room_id, |r| r.answers = HashMap::new());

    update_room_field(state, &room_id, |r| r.state = RoomState::Open);

    let _ = state.room_events.send(RoomEvent {
        room_id: room_id.clone(),
        kind: RoomEventKind::EndRound,
    });

    //let user_ids = read_room_field(state, &room_id, |r| r.users.clone());
    let users = read_room_field(state, &room_id, |r| r.users.clone()).unwrap();
    let user_ids = users.iter().map(|u| u.id.clone()).collect();

    let _ = state.player_events.send(PlayerEvent {
        player_ids: user_ids,
        kind: PlayerEventKind::Screen { screen: PlayerScreens::Room },
    });
    
    "ok".to_string()
}

#[get("/room/<room_id>/player")]
fn join_room(state: &State<AppState>, user: User, room_id: String) -> EventStream![] {

    let found = read_room_field(state, &room_id, |r| r.users.iter().any(|u| u.id == user.id));

    if !(found == Some(true)) {
        // On Join
        update_room_field(state, &room_id, |r| r.users.push(user.clone()));
        update_room_field(state, &room_id, |r| r.selected.push(user.id.clone()));
    } else {
        // On Rejoin
    }

    let _ = state.room_events.send(RoomEvent {
        room_id: room_id.clone(),
        kind: RoomEventKind::UserJoined { user: user.clone() },
    });

    let mut player_events = state.player_events.subscribe();

    // SSE Stream that sends events that select only the right user
    EventStream! {
        loop {

            // Fetch new event
            let player_event = match player_events.recv().await {
                Ok(event) => event,
                Err(_) => break,
            };

            // Only send events that are targeted to this user
            if player_event.player_ids.contains(&user.id) {
                yield Event::json(&player_event.kind);
            }
            
        }
    }
}

// Set username
#[get("/player/set-username/<name>?<room>")]
fn update_player_name(room: String, old_user: User, name: String, jar: &CookieJar<'_>, state: &State<AppState>) -> String {

    let mut user = old_user.clone();
    user.name = name;

    update_room_field(state, &room, |r| r.users = r.users.iter().map(|u| if u.id == old_user.id { user.clone() } else { u.clone() }).collect());

    let new_cookie = Cookie::build(("user_token", serde_json::to_string(&user).unwrap()))
        .path("/")
        .secure(false)
        .same_site(SameSite::Lax);

    jar.add(new_cookie);

    let _ = state.room_events.send(RoomEvent {
        room_id: room.clone(),
        kind: RoomEventKind::UserUpdated { user: user.clone() },
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

#[derive(Serialize, Deserialize, Clone, Debug)]
struct RoomEvent {
    room_id: String,
    kind: RoomEventKind,
}

// Events sent to the room manager
#[derive(Serialize, Deserialize, Clone, Debug)]
enum RoomEventKind {
    UserJoined { user: User },
    UserUpdated { user: User },
    UserLeft { user: User },
    PlayerSelected { user: User },
    Question {
        text: String,
        answers: [String; 3],
        from: u64,
        to: u64,
    },
    EvaluateContestants {
        evaluations: HashMap<String, Evaluation>,
    },
    EvaluatePlayer {
        text: String,
        answers: [EvaluatedAnswer; 3],
        end_round: bool
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
    #[serde(rename = "correct")]
    Correct,
    #[serde(rename = "wrong")]
    Wrong,
    Neutral,
    #[serde(rename = "wrong-selection")]
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
    #[serde(rename = "user-correct")]
    Correct,
    #[serde(rename = "user-wrong")]
    Wrong,
    #[serde(rename = "user-out")]
    Out,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct PlayerEvent {
    player_ids: Vec<String>,
    kind: PlayerEventKind,
}

// Events sent to the player
#[derive(Serialize, Deserialize, Clone, Debug)]
enum PlayerEventKind {
    Question {
        text: String,
        answers: [String; 3],
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
    Correct,
    In,
    Out,
    YouGotSelected,
    ToSlow,
    Wrong,
    Loading,
    Room,
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
        .mount("/api", routes![manage_room, join_room, check_room, update_player_name, start_round, question, evaluate_contestants, evaluate_player, user_list, end_round, answer_question])
        .mount("/", FileServer::from(relative!("html")))
        .manage(AppState {
            tx,
            player_events: player_tx,
            room_events: room_tx,
            rooms: RwLock::new(HashMap::new()),
        })
}
