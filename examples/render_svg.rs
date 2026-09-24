// Renders an SVG as a white silhouette in a square PNG, for icons the game
// tints when it draws them (assets/github-logo.png, assets/hockey-puck.png).
// The drawing itself - not the SVG's viewBox, which can have empty space
// around it - is fitted and centered, keeping its aspect ratio. Only its
// shape is kept (with anti-aliased edges); its colors are dropped, since the
// game supplies the color. Not part of the game.
//
//   cargo run --example render_svg -- <in.svg> <out.png> <size in px> [<padding in px>]
//
// The padding is a transparent margin on every side (the drawing fits in
// size - 2 * padding). Without one, a drawing that touches the edges - like
// a circle - gets its outermost pixels clipped when the texture is drawn.

use resvg::{tiny_skia, usvg};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (svg, png, size, padding) = match args.as_slice() {
        [svg, png, size] => (svg, png, size, "0"),
        [svg, png, size, padding] => (svg, png, size, padding.as_str()),
        _ => {
            eprintln!("usage: render_svg <in.svg> <out.png> <size in px> [<padding in px>]");
            std::process::exit(2);
        }
    };
    let size: u32 = size.parse().expect("size in px");
    let padding: f32 = padding.parse().expect("padding in px");
    let inner = size as f32 - 2.0 * padding;
    let data = std::fs::read(svg).expect("read svg");
    let tree = usvg::Tree::from_data(&data, &usvg::Options::default()).expect("parse svg");

    let bounds = tree.root().abs_stroke_bounding_box();
    let scale = inner / bounds.width().max(bounds.height());
    let transform = tiny_skia::Transform::from_row(
        scale,
        0.0,
        0.0,
        scale,
        (size as f32 - bounds.width() * scale) / 2.0 - bounds.x() * scale,
        (size as f32 - bounds.height() * scale) / 2.0 - bounds.y() * scale,
    );
    let mut pixmap = tiny_skia::Pixmap::new(size, size).expect("pixmap");
    resvg::render(&tree, transform, &mut pixmap.as_mut());

    // White, keeping each pixel's coverage (pixels are premultiplied).
    for pixel in pixmap.pixels_mut() {
        let a = pixel.alpha();
        *pixel = tiny_skia::PremultipliedColorU8::from_rgba(a, a, a, a).expect("valid premultiplied color");
    }
    pixmap.save_png(png).expect("write png");
    println!(
        "{png}: drawing {:.1} x {:.1} px (aspect {:.4}) centered in {size} x {size}",
        bounds.width() * scale,
        bounds.height() * scale,
        bounds.width() / bounds.height()
    );
}
