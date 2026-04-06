

// Fetch Room info
const urlParams = new URLSearchParams(window.location.search);
const room = urlParams.get('room');

document.getElementById("room").textContent = "Room: " + room;


// Run Game Steps
function start_round() {
    fetch("/api/room/" + room + "/start-round");
}

function question() {
    fetch("/api/room/" + room + "/question");
}


// Handle Room Events
function handleMessage(event) {
    // Parse all the recieved Data
    const data = JSON.parse(event.data);

    // Fetch the object name of the event
    const kind = Object.keys(data)[0];

    switch (kind) {
        case "UserJoined":

            // Add the user to the list, if not already existing
            const existingEntry = document.getElementById(data.UserJoined.user.id);
            if (!existingEntry) {
                const userEntry = document.importNode(document.getElementById("user-entry").content, true);
                userEntry.children[0].textContent = data.UserJoined.user.name;
                userEntry.children[0].id = data.UserJoined.user.id;
                document.getElementById("users").appendChild(userEntry);
            } else {
                existingEntry.textContent = data.UserJoined.user.name;
            }

            break;

        case "UserUpdated":
            document.getElementById(data.UserUpdated.user.id).textContent = data.UserUpdated.user.name;
            break;

        case "PlayerSelection":
            console.log("Player selection: " + data.PlayerSelection.user.name);
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