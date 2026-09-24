// Renders an SVG into a square PNG, fitted and centered, for art the game
// draws as a texture (e.g. assets/github-logo.png from
// assets/GitHub_Invertocat_White.svg). Not part of the game.
//
//   cargo run --example render_svg -- <in.svg> <out.png> <size in px>

use resvg::{tiny_skia, usvg};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [svg, png, size] = args.as_slice() else {
        eprintln!("usage: render_svg <in.svg> <out.png> <size in px>");
        std::process::exit(2);
    };
    let size: u32 = size.parse().expect("size in px");
    let data = std::fs::read(svg).expect("read svg");
    let tree = usvg::Tree::from_data(&data, &usvg::Options::default()).expect("parse svg");
    let (w, h) = (tree.size().width(), tree.size().height());
    let scale = size as f32 / w.max(h);
    let transform = tiny_skia::Transform::from_scale(scale, scale)
        .post_translate((size as f32 - w * scale) / 2.0, (size as f32 - h * scale) / 2.0);
    let mut pixmap = tiny_skia::Pixmap::new(size, size).expect("pixmap");
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    pixmap.save_png(png).expect("write png");
}
