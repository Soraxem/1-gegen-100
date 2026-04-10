async function update_player() {
    let username = document.getElementById("username").value;
    if (username == "") {
        alert("Please enter a Nickname");
        return false;
    }

    if (username.length > 16) {
        alert("Max 16 Letters as Username");
        return false;
    }

    let response = await fetch("/api/player/set-username/" + username + "?room=" + room);

    show_screen(Screens.Room)

}



const urlParams = new URLSearchParams(window.location.search);
const room = urlParams.get('room');

document.getElementById("room").textContent = "Room: " + room;
document.getElementById("room-nick").textContent = "Joining Room: " + room;

// Enum of all screens
const Screens = Object.freeze({
    Question: "screen-question",
    Correct: "screen-correct",
    In: "screen-in",
    Out: "screen-out",
    YouGotSelected: "screen-you-got-selected",
    ToSlow: "screen-to-slow",
    Wrong: "screen-wrong",
    Loading: "loading",
    Room: "screen-room",
    Nickname: "screen-nickname",
});

function show_screen(screen) {
    for (const s of Object.values(Screens)) {
        document.getElementById(s).classList.remove("visible");
        document.getElementById(s).classList.add("invisible");
    }
    document.getElementById(screen).classList.remove("invisible");
    document.getElementById(screen).classList.add("visible");
}

function is_screen_visible(screen) {
    return document.getElementById(screen).classList.contains("visible");
}

show_screen(Screens.Nickname);

function get_unix_time() {
    return new Date().getTime() / 1000;
}

function answer(answer) {
    fetch("/api/player/" + room + "/answer-question/" + answer);

    const answers = document.querySelectorAll(".answer");
    answers.forEach(answer => answer.classList.remove("active"));

    document.getElementById("answer-" + answer).classList.add("active");
}

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

            show_screen(Screens.Loading);


            document.getElementById("question").textContent = data.Question.text;
            
            for (let i = 0; i < 3; i++) {
                document.getElementById("answer-" + (i + 1)).textContent = data.Question.answers[i];
            }

            const answers = document.querySelectorAll(".answer");
            answers.forEach(answer => answer.classList.remove("active"));

            let startTime = get_unix_time();
            let timeoutId = setInterval(() => {
                let currentTime = get_unix_time();
                if (currentTime >= data.Question.from) {
                    clearInterval(timeoutId);
                    show_screen(Screens.Question);
                    let questionTimeoutId = setInterval(() => {
                        let currentTime = get_unix_time();
                        if (currentTime >= data.Question.to || !is_screen_visible(Screens.Question)) {
                            clearInterval(questionTimeoutId);
                            show_screen(Screens.Loading);
                        } else {
                            document.getElementById("question-time").textContent = (data.Question.to - currentTime).toFixed(0);
                        }
                    }, 100);
                }
            }, 100);

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