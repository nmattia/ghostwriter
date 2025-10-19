//! Const functions to process the story's HTML.
//! The raw HTML file is too big to fit and contains a lot of unnecessary data
//! like browser stylesheets & scripts.

const fn bytes_equal(haystack: &[u8], needle: &[u8], offset: usize) -> bool {
    let mut i = 0;
    while i < needle.len() {
        if haystack[offset + i] != needle[i] {
            return false;
        }
        i += 1;
    }

    true
}

const fn find_offset(haystack: &[u8], needle: &[u8]) -> usize {
    let mut i = 0;
    while i < haystack.len() {
        if bytes_equal(haystack, needle, i) {
            return i;
        }

        i += 1;
    }
    panic!("Not found");
}

const fn strip_header(bytes: &[u8]) -> &[u8] {
    let tw_storydata_start = b"<tw-storydata"; // don't close bc there are attrs
    let tw_storydata_end = b"</tw-storydata>";
    let start_offset = find_offset(bytes, tw_storydata_start);
    let end_offset = find_offset(bytes, tw_storydata_end);

    &bytes[start_offset..end_offset]
}

const RAW: &[u8] = include_bytes!("ghostwriter.html");

pub const DATA: &str = match core::str::from_utf8(strip_header(RAW)) {
    Err(_) => panic!("Nope"),
    Ok(s) => s,
};
