use console::Style;

pub(crate) static GREEN: Style = Style::new().green();
pub(crate) static CANY: Style = Style::new().cyan();
pub(crate) static RED: Style = Style::new().red();
pub(crate) static YELLOW: Style = Style::new().yellow();

fn now() -> String {
    let now = chrono::Local::now();
    now.format("%Y-%m-%d %H:%M:%S").to_string()
}

pub(crate) fn success(msg: &str) {
    println!("[{}] {} {}", now(), GREEN.apply_to("[INFO]"), msg,);
}

pub(crate) fn conf() -> String {
    format!("{} : ", YELLOW.apply_to("Question"))
}

pub(crate) fn info(msg: &str) {
    println!("[{}] {} {}", now(), CANY.apply_to("[INFO]"), msg,);
}

pub(crate) fn error(msg: &str) {
    eprintln!("[{}] {} {}", now(), RED.apply_to("[ ERR]"), msg,);
}

pub(crate) fn warn(msg: &str) {
    eprintln!("[{}] {} {}", now(), YELLOW.apply_to("[WARN]"), msg,);
}
