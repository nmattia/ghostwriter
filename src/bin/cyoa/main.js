// Restart the story by clearing session storage and reloading page
function restartStory() {

    try {
        const key = "Saved Session"; // found in browser storage
        sessionStorage.removeItem(key);
    } catch (err) {
        console.warn("Could not clear Harlowe session storage:", err);
    }
    location.reload();

}
//
// allow iterating through passage links with space and selecting with enter
document.body.onkeydown = function(e) {
    if (e.key == " " ||
        e.code == "Space" ||
        e.keyCode == 32
    ) {
        e.preventDefault();

        const links = Array.from(document.querySelectorAll("tw-link"));

        // Reached the end, so restart
        if(links.length === 0) {
            restartStory();
            return;
        }

        const selected = document.querySelector(":focus") || links[0];

        const ix = links.indexOf(selected);
        const ixNxt = (ix + 1) % links.length;
        const nxt = links[ixNxt];
        nxt.focus();
    }

    // handle "r" for restart
    if (e.key === "r" || e.key === "R") {
        restartStory();
    }
}
