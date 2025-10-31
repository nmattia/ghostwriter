// allow iterating through passage links with space and selecting with enter
document.body.onkeydown = function(e) {
    if (e.key == " " ||
        e.code == "Space" ||
        e.keyCode == 32
    ) {
        e.preventDefault();

        const links = Array.from(document.querySelectorAll("tw-link"));
        const selected = document.querySelector(":focus") || links[0];

        const ix = links.indexOf(selected);
        const ixNxt = (ix + 1) % links.length;
        const nxt = links[ixNxt];
        nxt.focus();
    }
}

// Focus the first link every time a new passage is rendered (Harlowe)
const target = document.querySelector("tw-story");

const focus = () => {
    const firstLink = document.querySelector("tw-link");
    if (firstLink) {
        firstLink.focus();
    }
}

const observer = new MutationObserver(() => {
    // Defer a tick so Harlowe has time to render fully
    setTimeout(() => focus(), 0);
});

observer.observe(target, { childList: true });
