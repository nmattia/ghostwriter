//! Text (typing) related

/// Convert an ASCII char code to a keyboard keycode
#[allow(clippy::if_same_then_else)]
#[allow(clippy::manual_range_contains)]
pub fn char_to_keycode(chr: u8) -> u8 {
    if chr >= 97 && chr <= 122 {
        chr - 97 + 4
    } else if chr == 44 {
        54
    } else if chr == 46 {
        55
    } else if chr == 32 {
        44
    } else if chr == 10 {
        40
    } else if chr == 39 {
        52
    } else if chr == 33 {
        51 // fake exlamation mark
    } else if chr == 58 {
        51 // fake colon (:)
    } else {
        0
    }
}

pub const TEXT: &str = "
lorem ipsum odor amet, consectetuer adipiscing elit. vitae sapien adipiscing sem taciti bibendum aenean platea montes bibendum. pharetra sem ultrices vitae quis bibendum augue ligula. nibh fermentum lacinia purus molestie sociosqu felis est. hendrerit sollicitudin eu sodales ullamcorper eros bibendum morbi. integer class cursus suscipit commodo tempor nec. nostra suscipit ipsum semper elementum habitant ultrices posuere. auctor maximus venenatis turpis vitae enim; dis potenti dictumst. libero consequat iaculis magna, sollicitudin feugiat praesent tempus.

nulla sem erat sagittis taciti maximus fusce, at in dis. fringilla venenatis vestibulum eu nostra inceptos morbi. rhoncus cubilia adipiscing mauris ex mus montes felis primis. sed bibendum eleifend senectus gravida; ex pulvinar magnis aptent. arcu cras sagittis libero penatibus nunc rutrum vestibulum. habitant class ante semper class dignissim; ante a aenean eu. arcu suspendisse pellentesque netus bibendum egestas.

convallis aenean lectus maecenas mollis at aenean. litora risus malesuada nullam eu cursus mus mauris mauris vulputate. erat tincidunt arcu justo interdum proin praesent tempus. mollis sapien vestibulum ante suspendisse cras primis ligula lectus nam. dapibus natoque sit gravida dui pellentesque nostra. mi pulvinar sit; turpis mollis justo leo habitant vestibulum sed.

elit congue scelerisque lectus phasellus consequat lacinia nulla. hendrerit platea nostra quisque lorem laoreet. orci nibh felis blandit lacus; vulputate nascetur morbi. platea metus phasellus habitant, dignissim felis aenean imperdiet. nascetur nisl mauris elementum auctor mus non ut arcu neque. morbi egestas curae ultricies eu morbi accumsan lacus vitae. efficitur lorem sodales pellentesque quisque commodo lacinia augue. elementum massa hendrerit imperdiet imperdiet varius maecenas.

libero quis sociosqu fringilla mauris; praesent fringilla praesent molestie. consequat tellus cras ullamcorper maecenas inceptos diam pellentesque a cursus. quam tincidunt conubia primis praesent maximus torquent. integer dictum senectus tellus porta lacinia maecenas quis potenti. pretium varius ornare lacus semper nullam ligula, luctus amet! porttitor auctor eros aliquam ex nibh; dolor semper dapibus. tincidunt ligula montes vehicula tellus ut habitasse massa.

";
