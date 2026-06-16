fn main() {
    let mut o = comrak::Options::default();
    o.extension.table = true;
    o.extension.tasklist = true;
    o.extension.math_dollars = true;
    o.extension.math_code = true;
    o.render.unsafe_ = true;
    for src in ["Inline $a^2+b^2$ here.", "$$\\int_0^1 x\\,dx$$", "- [x] done\n- [ ] todo", "| a | b |\n|:--|--:|\n| 1 | 2 |"] {
        println!("SRC: {src:?}\nOUT: {}\n---", comrak::markdown_to_html(src, &o));
    }
}
