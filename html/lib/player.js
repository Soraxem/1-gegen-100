async function update_player() {
    let username = document.getElementById("username").value;
    if (username == "") {
        alert("Please enter a Nickname");
        return false;
    }

    let response = await fetch("/api/player/set-username/" + username + "?room=" + room);
}



const urlParams = new URLSearchParams(window.location.search);
const room = urlParams.get('room');

document.getElementById("room").textContent = "Room: " + room;

// Enum of all screens
const Screens = Object.freeze({
    Question: "screen-question",
    Correct: "screen-correct",
    In: "screen-in",
    Out: "screen-out",
    YouGotSelected: "screen-you-got-selected",
    ToSlow: "screen-to-slow",
    Wrong: "screen-wrong",
    Loading: "screen-loading",
    Room: "screen-room",
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

async function handleMessage(event) {
    const data = JSON.parse(event.data);

    let kind;

    if (typeof data === "string") {
        // Handle unit variants like "EndRound"
        kind = data;
    } else {
        // Handle variants with data like "UserJoined"
        kind = Object.keys(data)[0];
    }

    switch (kind) {
        case "Question":

            break;
        case "Screen":

            show_screen(Screens[data.Screen.screen]);

            break;
        default:
            console.log("Unknown event kind: " + event.kind);
            break;

    }
}

let es;

function connect() {
    es = new EventSource("/api/room/" + room + "/player");
    es.addEventListener("message", handleMessage);
}

connect();