/// Text (typing) related

/// Convert an ASCII char code to a keyboard keycode
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
Lorem ipsum odor amet, consectetuer adipiscing elit. Vitae sapien adipiscing sem taciti bibendum aenean platea montes bibendum. Pharetra sem ultrices vitae quis bibendum augue ligula. Nibh fermentum lacinia purus molestie sociosqu felis est. Hendrerit sollicitudin eu sodales ullamcorper eros bibendum morbi. Integer class cursus suscipit commodo tempor nec. Nostra suscipit ipsum semper elementum habitant ultrices posuere. Auctor maximus venenatis turpis vitae enim; dis potenti dictumst. Libero consequat iaculis magna, sollicitudin feugiat praesent tempus.

Nulla sem erat sagittis taciti maximus fusce, at in dis. Fringilla venenatis vestibulum eu nostra inceptos morbi. Rhoncus cubilia adipiscing mauris ex mus montes felis primis. Sed bibendum eleifend senectus gravida; ex pulvinar magnis aptent. Arcu cras sagittis libero penatibus nunc rutrum vestibulum. Habitant class ante semper class dignissim; ante a aenean eu. Arcu suspendisse pellentesque netus bibendum egestas.

Convallis aenean lectus maecenas mollis at aenean. Litora risus malesuada nullam eu cursus mus mauris mauris vulputate. Erat tincidunt arcu justo interdum proin praesent tempus. Mollis sapien vestibulum ante suspendisse cras primis ligula lectus nam. Dapibus natoque sit gravida dui pellentesque nostra. Mi pulvinar sit; turpis mollis justo leo habitant vestibulum sed.

Elit congue scelerisque lectus phasellus consequat lacinia nulla. Hendrerit platea nostra quisque lorem laoreet. Orci nibh felis blandit lacus; vulputate nascetur morbi. Platea metus phasellus habitant, dignissim felis aenean imperdiet. Nascetur nisl mauris elementum auctor mus non ut arcu neque. Morbi egestas curae ultricies eu morbi accumsan lacus vitae. Efficitur lorem sodales pellentesque quisque commodo lacinia augue. Elementum massa hendrerit imperdiet imperdiet varius maecenas.

Libero quis sociosqu fringilla mauris; praesent fringilla praesent molestie. Consequat tellus cras ullamcorper maecenas inceptos diam pellentesque a cursus. Quam tincidunt conubia primis praesent maximus torquent. Integer dictum senectus tellus porta lacinia maecenas quis potenti. Pretium varius ornare lacus semper nullam ligula, luctus amet! Porttitor auctor eros aliquam ex nibh; dolor semper dapibus. Tincidunt ligula montes vehicula tellus ut habitasse massa.

";
