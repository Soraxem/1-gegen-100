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

let es;

function connect() {
    es = new EventSource("/api/room/" + room + "/player");
    es.addEventListener("message", (event) => {
        const data = JSON.parse(event.data);
        console.log(data);
    });
}

connect();