

// Fetch Room info
const urlParams = new URLSearchParams(window.location.search);
const room = urlParams.get('room');

document.getElementById("room").textContent = "Room: " + room;


// Enum of all screens
const Screens = Object.freeze({
    Loading: "loading",
    Room: "screen-room",
    PlayerSelection: "screen-player-selection",
    Question: "screen-question",
    EvaluateContestants: "screen-evaluate-contestants",
    EvaluatePlayer: "screen-evaluate-player",
});

function show_screen(screen) {
    for (const s of Object.values(Screens)) {
        document.getElementById(s).classList.remove("visible");
        document.getElementById(s).classList.add("invisible");
    }
    document.getElementById(screen).classList.remove("invisible");
    document.getElementById(screen).classList.add("visible");
}

show_screen(Screens.Room);


// Run Game Steps
function start_round() {
    fetch("/api/room/" + room + "/start-round");
}

function question() {
    fetch("/api/room/" + room + "/question");
}

function evaluate_contestants() {
    fetch("/api/room/" + room + "/evaluate-contestants");
}

function evaluate_player() {
    fetch("/api/room/" + room + "/evaluate-player");
}

function end_round() {
    fetch("/api/room/" + room + "/end-round");
}

function user_template(username, user_id, marker) {
    const userEntry = document.importNode(document.getElementById("user-entry").content, true);
    
    userEntry.querySelector(".user-entry").id = "user-" + user_id;
    userEntry.querySelector(".user-entry").classList.add(marker);
    userEntry.querySelector(".user").textContent = username;

    return userEntry;
}

function get_unix_time() {
    return new Date().getTime() / 1000;
}

let scores;

// Handle Room Events
async function handleMessage(event) {
    // Parse all the recieved Data
    const data = JSON.parse(event.data);

    let kind;

    if (typeof data === "string") {
        // Handle unit variants like "EndRound"
        kind = data;
    } else {
        // Handle variants with data like "UserJoined"
        kind = Object.keys(data)[0];
    }

    // Fetch the object name of the event
    //const kind = Object.keys(data)[0];

    switch (kind) {
        case "UserJoined":

            // Add the user to the list, if not already existing
            existingEntry = document.querySelector("#users #user-" + data.UserJoined.user.id);
            if (!existingEntry) {
                const userEntry = user_template(data.UserJoined.user.name, data.UserJoined.user.id, "user-entry");
                document.getElementById("users").appendChild(userEntry);
            } else {
                existingEntry.querySelector(".user").textContent = data.UserJoined.user.name;
            }

            break;

        case "UserUpdated":
            // Add the user to the list, if not already existing
            existingEntry = document.querySelector("#users #user-" + data.UserUpdated.user.id);
            if (!existingEntry) {
                const userEntry = user_template(data.UserJoined.user.name, data.UserJoined.user.id, "user-entry");
                document.getElementById("users").appendChild(userEntry);
            } else {
                existingEntry.querySelector(".user").textContent = data.UserUpdated.user.name;
            }
            break;

        case "PlayerSelected":
            console.log("Player selection: " + data.PlayerSelected.user.name);
            show_screen(Screens.PlayerSelection);
            document.getElementById("selected-player").textContent = data.PlayerSelected.user.name;
            break;

        case "Question":
            console.log("Recieved Question");
            

            document.getElementById("question").textContent = data.Question.text;
            
            for (let i = 0; i < 3; i++) {
                document.getElementById("answer-" + i).textContent = data.Question.answers[i];
            }
            show_screen(Screens.Loading);

            let startTime = get_unix_time();
            let timeoutId = setInterval(() => {
                let currentTime = get_unix_time();
                if (currentTime >= data.Question.from) {
                    clearInterval(timeoutId);
                    show_screen(Screens.Question);
                    let questionTimeoutId = setInterval(() => {
                        let currentTime = get_unix_time();
                        if (currentTime >= data.Question.to) {
                            clearInterval(questionTimeoutId);
                        } else {
                            document.getElementById("question-time").textContent = (data.Question.to - currentTime).toFixed(0);
                        }
                    }, 100);
                }
            }, 100);

            
            break;
        
        case "EvaluateContestants":

            const response = await fetch("/api/room/" + room + "/user-list");
            const userList = await response.json();

            console.log(userList);

            document.getElementById("contestants").innerHTML = "";

            for (const [user_id, evaluation] of Object.entries(data.EvaluateContestants.evaluations)) {
                
                // Look through the array for the object with the matching ID
                const user = userList.find(u => u.id === user_id);
    
                // Use a fallback name in case the ID isn't found
                const userName = user ? user.name : "Unknown User";

                const userEntry = user_template(userName, user_id, evaluation);
                document.getElementById("contestants").appendChild(userEntry);
            }

            show_screen(Screens.EvaluateContestants);
            
            break;

        case "EvaluatePlayer":
            console.log("Evaluate Player");

            document.getElementById("evaluated-question").textContent = data.EvaluatePlayer.text;

            const answers = document.querySelectorAll(".answer");
            answers.forEach(answer => answer.classList.remove("correct"));
            answers.forEach(answer => answer.classList.remove("wrong"));
            answers.forEach(answer => answer.classList.remove("wrong-selection"));

            for (let i = 0; i < 3; i++) {
                document.getElementById("evaluated-answer-" + i).textContent = data.EvaluatePlayer.answers[i].answer;
                document.getElementById("evaluated-answer-" + i).classList.add(data.EvaluatePlayer.answers[i].evaluation);
            }

            if (data.EvaluatePlayer.end_round) {
                document.getElementById("next-question").style.display = "none";
            } else {
                document.getElementById("next-question").style.display = "block";
            }

            show_screen(Screens.EvaluatePlayer);

            break;
        
        case "EndRound":
            console.log("End Round");

            const json_scores = await fetch("/api/room/" + room + "/user-scores");
            const scores = await json_scores.json();

            const usersContainer = document.querySelector("#users");

            // 1. Update the scores in the DOM first
            for (const [user, score] of Object.entries(scores)) {
                const userEntry = usersContainer.querySelector("#user-" + user);
                if (userEntry) {
                    userEntry.querySelector(".user-score").textContent = score;
                }
            }

            // 2. Sort the entries
            const userElements = Array.from(usersContainer.children);

            userElements.sort((a, b) => {
                const scoreA = parseInt(a.querySelector(".user-score").textContent) || 0;
                const scoreB = parseInt(b.querySelector(".user-score").textContent) || 0;
                return scoreB - scoreA; // Descending order (highest score first)
            });

            // 3. Re-append them to the container to reflect the new order
            userElements.forEach(el => usersContainer.appendChild(el));

            show_screen(Screens.Room);
            break;

        default:
            console.log("Unknown event kind: " + event.kind);
            break;
    }

}




// Setup Event source

let es;

function connect() {
    es = new EventSource("/api/room/" + room + "/manager");
    es.addEventListener("message", handleMessage);
}

// Start Streaming events
connect();


async function onlin_users() {
    const users = await fetch("/api/room/" + room + "/user-list");
    console.log(users.json());
}